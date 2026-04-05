/// Sistemas de movimento — move_target e integração de posição.
///
/// AccelProfile: rampa suave + decel_zone (escala vel pela distância ao destino)
/// + momentum em virada brusca. movement_system faz wall-slide automático.

use hecs::World;
use crate::core::types::{Fixed, Vec2Fixed};
use crate::sim::components::{Position, Velocity, MoveTarget, MoveSpeed,
                              Path, CrowdControl, CcKind, AccelProfile};
use crate::sim::pathfinding::NavigationGrid;

/// Snap do destino final (sim-units).
const SNAP_FINAL: f64 = 0.22;
/// Snap de waypoints intermediários — deve ser > max_speed para evitar overshoot.
const SNAP_INTER: f64 = 0.65;

pub fn move_target_system(world: &mut World) {
    let rooted: Vec<hecs::Entity> = world
        .query::<&CrowdControl>()
        .iter()
        .filter(|(_, cc)| matches!(cc.kind, CcKind::Root | CcKind::Stun | CcKind::Knockup))
        .map(|(e, _)| e)
        .collect();

    let mut exhausted: Vec<hecs::Entity> = vec![];

    for (entity, (pos, vel, target, speed, mut maybe_path, maybe_accel)) in world
        .query_mut::<(
            &mut Position, &mut Velocity, &MoveTarget, &MoveSpeed,
            Option<&mut Path>, Option<&AccelProfile>,
        )>()
    {
        let old_vel = vel.0;
        vel.0       = Vec2Fixed::ZERO;
        if rooted.contains(&entity) { continue; }

        if let Some(accel) = maybe_accel {
            // ── AccelProfile: movimento suave com decel zone ──────────────────
            // Se path exausto, marca e sai
            if maybe_path.as_ref().map_or(false, |p| p.exhausted()) {
                exhausted.push(entity); continue;
            }
            // Alvo: waypoint atual ou MoveTarget direto
            let wp = maybe_path.as_ref()
                .and_then(|p| p.current_wp())
                .unwrap_or(target.0);
            let is_final = maybe_path.as_ref()
                .map_or(true, |p| p.current + 1 >= p.waypoints.len());

            let dir       = wp - pos.0;
            let dist      = dir.length();
            let snap_dist = if is_final {
                Fixed::from_num(SNAP_FINAL)
            } else {
                Fixed::from_num(SNAP_INTER)
            };

            // Snap ao waypoint/destino
            if dist <= snap_dist {
                pos.0 = wp;
                if let Some(ref mut p) = maybe_path {
                    p.advance();
                    if p.exhausted() { exhausted.push(entity); }
                }
                continue; // vel = ZERO
            }

            // Escala velocidade na decel zone — apenas no segmento final
            let desired_spd = if is_final
                && accel.decel_zone > Fixed::ZERO
                && dist < accel.decel_zone
            {
                // Escala linear: 0 no destino, max na borda da zona.
                // Clampado a 8% do max para não parar completamente antes de snap.
                let s = dist / accel.decel_zone;
                let min = Fixed::from_num(0.08);
                if s < min { min * speed.0 } else { s * speed.0 }
            } else {
                speed.0
            };

            vel.0 = blend(old_vel, dir.normalize() * desired_spd, speed.0, *accel);

        } else {
            // ── Sem AccelProfile: budget loop instantâneo ─────────────────────
            if let Some(path) = maybe_path {
                let mut budget = speed.0;
                loop {
                    let Some(wp) = path.current_wp() else {
                        exhausted.push(entity); break;
                    };
                    let dir  = wp - pos.0;
                    let dist = dir.length();
                    if dist == Fixed::ZERO { path.advance(); continue; }
                    if dist <= budget {
                        pos.0 = wp; budget -= dist; path.advance();
                        if budget <= Fixed::ZERO { break; }
                    } else {
                        vel.0 = dir.normalize() * budget; break;
                    }
                }
            } else {
                let dir     = target.0 - pos.0;
                let dist_sq = dir.length_sq();
                if dist_sq <= speed.0 * speed.0 {
                    pos.0 = target.0;
                } else {
                    vel.0 = dir.normalize() * speed.0;
                }
            }
        }
    }

    for e in exhausted { let _ = world.remove_one::<Path>(e); }
}

/// Blenda vel antiga → desejada com momentum proporcional à velocidade atual.
///
/// Momentum escala pelo quadrado de (|vel| / max_speed):
///   - pouco deslocamento (vel baixa) → quase sem slide
///   - velocidade máxima acumulada   → slide completo
/// Isso garante que só sente o deslize após acelerar por tempo suficiente.
fn blend(old: Vec2Fixed, desired: Vec2Fixed, max_speed: Fixed, p: AccelProfile) -> Vec2Fixed {
    if desired == Vec2Fixed::ZERO { return Vec2Fixed::ZERO; }
    let dot = old.dot(desired);
    let max_sq = max_speed * max_speed;
    // Slide quando acima de ~20% da vel máxima e direção inverteu.
    // Fórmula: lerp(old→desired) com peso momentum no old.
    // Resultado: hero continua na direção antiga (overshoot visível).
    let threshold = max_sq * Fixed::from_num(0.04);
    if dot < Fixed::ZERO && old.length_sq() > threshold {
        let blended = old * p.momentum + desired * (Fixed::ONE - p.momentum);
        let len = blended.length();
        let cap = max_speed * Fixed::from_num(1.3);
        if len > cap { blended * (cap / len) } else { blended }
    } else {
        old + (desired - old) * p.accel
    }
}

/// Drena velocidade de entidades com AccelProfile que não têm MoveTarget ativo.
/// Chamado após stop_hero() — o personagem desacelera suavemente em vez de parar.
pub fn coast_system(world: &mut World) {
    let coasting: Vec<(hecs::Entity, Vec2Fixed)> = world
        .query::<(&Velocity, &AccelProfile, Option<&MoveTarget>)>()
        .iter()
        .filter(|(_, (_, _, mt))| mt.is_none())
        .map(|(e, (v, _, _))| (e, v.0))
        .collect();
    for (e, old_vel) in coasting {
        if let Ok(mut v) = world.get::<&mut Velocity>(e) {
            v.0 = old_vel * Fixed::from_num(0.88);
            if v.0.length_sq() < Fixed::from_num(0.0004) { v.0 = Vec2Fixed::ZERO; }
        }
    }
}

/// Integra vel → pos com wall-slide.
/// Tenta mover completo; se bloqueado, desliza em X ou Y; senão para.
pub fn movement_system(world: &mut World, nav: &NavigationGrid) {
    for (_, (pos, vel)) in world.query_mut::<(&mut Position, &mut Velocity)>() {
        if vel.0 == Vec2Fixed::ZERO { continue; }
        let new = pos.0 + vel.0;
        let nc  = nav.world_to_cell(new);
        if !nav.is_blocked(nc.0, nc.1) { pos.0 = new; continue; }
        // Slide X
        let xc = nav.world_to_cell(Vec2Fixed::new(new.x, pos.0.y));
        if !nav.is_blocked(xc.0, xc.1) {
            pos.0.x = new.x; vel.0.y = Fixed::ZERO; continue;
        }
        // Slide Y
        let yc = nav.world_to_cell(Vec2Fixed::new(pos.0.x, new.y));
        if !nav.is_blocked(yc.0, yc.1) {
            pos.0.y = new.y; vel.0.x = Fixed::ZERO; continue;
        }
        vel.0 = Vec2Fixed::ZERO; // totalmente bloqueado
    }
}

/// Remove MoveTarget quando entidade parou e não tem Path ativo.
pub fn clear_arrived_targets(world: &mut World) {
    let stopped: Vec<hecs::Entity> = world
        .query::<(&Velocity, &MoveTarget)>()
        .iter()
        .filter(|(_, (v, _))| v.0 == Vec2Fixed::ZERO)
        .map(|(e, _)| e)
        .collect();
    let arrived: Vec<hecs::Entity> = stopped.into_iter()
        .filter(|e| world.get::<&Path>(*e).is_err())
        .collect();
    for e in arrived { let _ = world.remove_one::<MoveTarget>(e); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::Fixed;
    use crate::sim::components::*;
    use crate::sim::pathfinding::NavigationGrid;

    fn f(n: f64) -> Fixed { Fixed::from_num(n) }
    fn p(x: f64, y: f64) -> Position { Position(Vec2Fixed::new(f(x), f(y))) }

    #[test]
    fn entity_moves_toward_target() {
        let mut w = World::new();
        let e = w.spawn((p(0.,0.), Velocity::default(),
            MoveTarget(Vec2Fixed::new(f(10.), f(0.))), MoveSpeed(f(2.))));
        let nav = NavigationGrid::default_128();
        move_target_system(&mut w);
        movement_system(&mut w, &nav);
        assert!(w.get::<&Position>(e).unwrap().0.x > Fixed::ZERO);
    }

    #[test]
    fn rooted_entity_does_not_move() {
        let mut w = World::new();
        let e = w.spawn((p(0.,0.), Velocity::default(),
            MoveTarget(Vec2Fixed::new(f(10.), f(0.))), MoveSpeed(f(2.)),
            CrowdControl { kind: CcKind::Root, ticks_remaining: 5 }));
        let nav = NavigationGrid::default_128();
        move_target_system(&mut w);
        movement_system(&mut w, &nav);
        assert_eq!(w.get::<&Position>(e).unwrap().0.x, Fixed::ZERO);
    }

    #[test]
    fn accel_profile_ramps_up() {
        let mut w = World::new();
        let prof = AccelProfile { accel: f(0.10), decel_zone: f(8.), momentum: f(0.70) };
        let e = w.spawn((p(0.,0.), Velocity::default(),
            MoveTarget(Vec2Fixed::new(f(50.), f(0.))), MoveSpeed(f(0.375)), prof));
        let nav = NavigationGrid::default_128();
        move_target_system(&mut w);
        let v1 = w.get::<&Velocity>(e).unwrap().0.x;
        movement_system(&mut w, &nav);
        move_target_system(&mut w);
        let v2 = w.get::<&Velocity>(e).unwrap().0.x;
        assert!(v2 > v1, "velocidade deve crescer com aceleração");
    }

    #[test]
    fn accel_profile_decelerates_near_target() {
        let mut w = World::new();
        let spd = f(0.375);
        let prof = AccelProfile { accel: f(1.0), decel_zone: f(8.), momentum: f(0.) };
        // Hero longe → velocidade máxima
        let e_far = w.spawn((p(0.,0.), Velocity::default(),
            MoveTarget(Vec2Fixed::new(f(50.), f(0.))), MoveSpeed(spd), prof));
        // Hero dentro da decel zone (dist=4, zone=8 → scale=0.5)
        let e_near = w.spawn((p(46.,0.), Velocity::default(),
            MoveTarget(Vec2Fixed::new(f(50.), f(0.))), MoveSpeed(spd), prof));
        move_target_system(&mut w);
        let v_far  = w.get::<&Velocity>(e_far).unwrap().0.x;
        let v_near = w.get::<&Velocity>(e_near).unwrap().0.x;
        assert!(v_near < v_far, "mais perto = velocidade menor na decel zone");
    }

    #[test]
    fn wall_slide_prevents_clipping() {
        let mut nav = NavigationGrid::default_128();
        nav.set_blocked(5, 0, true);  // parede na célula (5,0)
        let mut w = World::new();
        // Entidade em x=4 com vel=1.5 → new_pos=5.5 → cai na célula (5,0) bloqueada
        let e = w.spawn((p(4.,0.), Velocity(Vec2Fixed::new(f(1.5), Fixed::ZERO))));
        movement_system(&mut w, &nav);
        let px = w.get::<&Position>(e).unwrap().0.x;
        assert!(px < f(5.), "não deve atravessar a parede");
    }
}
