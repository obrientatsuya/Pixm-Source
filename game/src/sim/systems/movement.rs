/// Sistemas de movimento — move_target e integração de posição.
///
/// move_target_system: Path-aware. Se entidade tem Path, segue waypoints.
/// Quando waypoint alcançado, avança; quando Path esgotado, remove-o.
/// Sem Path: movimento direto ao MoveTarget (fallback ou entidades simples).

use hecs::World;
use crate::core::types::{Fixed, Vec2Fixed};
use crate::sim::components::{Position, Velocity, MoveTarget, MoveSpeed, Path,
                              CrowdControl, CcKind};

/// Resolve MoveTarget/Path → Velocity.
/// Entidades com Root/Stun/Knockup ficam paradas.
///
/// Loop de budget: consome waypoints até esgotar `speed` unidades por tick.
/// Evita o problema de "um waypoint por tick" quando waypoints são densos.
pub fn move_target_system(world: &mut World) {
    let rooted: Vec<hecs::Entity> = world
        .query::<&CrowdControl>()
        .iter()
        .filter(|(_, cc)| matches!(cc.kind, CcKind::Root | CcKind::Stun | CcKind::Knockup))
        .map(|(e, _)| e)
        .collect();

    let mut path_exhausted: Vec<hecs::Entity> = vec![];

    for (entity, (pos, vel, target, speed, maybe_path)) in world
        .query_mut::<(&mut Position, &mut Velocity, &MoveTarget, &MoveSpeed, Option<&mut Path>)>()
    {
        vel.0 = Vec2Fixed::ZERO; // padrão: sem movimento

        if rooted.contains(&entity) { continue; }

        if let Some(path) = maybe_path {
            // Consome waypoints usando budget de distância (speed unidades/tick).
            // Após snaps intermediários, o budget restante vira velocidade
            // que movement_system aplica → total = speed unidades no tick.
            let mut budget = speed.0;

            loop {
                let Some(wp) = path.current_wp() else {
                    path_exhausted.push(entity);
                    break;
                };
                let dir  = wp - pos.0;
                let dist = dir.length();

                if dist == Fixed::ZERO {
                    // waypoint sobre a entidade — avança sem consumir budget
                    path.advance();
                    continue;
                }

                if dist <= budget {
                    // Snap até este waypoint e continua com o budget restante
                    pos.0   = wp;
                    budget -= dist;
                    path.advance();
                    if budget <= Fixed::ZERO { break; }
                } else {
                    // Move com o budget restante em direção ao próximo waypoint
                    vel.0 = dir.normalize() * budget;
                    break;
                }
            }
        } else {
            // Sem Path: movimento direto ao MoveTarget
            let dir     = target.0 - pos.0;
            let dist_sq = dir.length_sq();
            let arr_sq  = speed.0 * speed.0;

            if dist_sq <= arr_sq {
                pos.0 = target.0;
            } else {
                vel.0 = dir.normalize() * speed.0;
            }
        }
    }

    for e in path_exhausted {
        let _ = world.remove_one::<Path>(e);
    }
}

/// Integra velocidade → posição (1 tick).
pub fn movement_system(world: &mut World) {
    for (_, (pos, vel)) in world.query_mut::<(&mut Position, &Velocity)>() {
        pos.0 = pos.0 + vel.0;
    }
}

/// Remove MoveTarget quando entidade chegou E não tem Path ativo.
/// Path esgotado já foi removido em move_target_system.
pub fn clear_arrived_targets(world: &mut World) {
    // Candidatos: vel == 0 e tem MoveTarget
    let candidates: Vec<hecs::Entity> = world
        .query::<(&Velocity, &MoveTarget)>()
        .iter()
        .filter(|(_, (vel, _))| vel.0 == Vec2Fixed::ZERO)
        .map(|(e, _)| e)
        .collect();

    // Só remove se não há Path ativo (Path ativo = ainda em trânsito)
    let arrived: Vec<hecs::Entity> = candidates.into_iter()
        .filter(|e| world.get::<&Path>(*e).is_err())
        .collect();

    for entity in arrived {
        let _ = world.remove_one::<MoveTarget>(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::Fixed;
    use crate::sim::components::*;

    fn fixed(n: f64) -> Fixed { Fixed::from_num(n) }
    fn pos(x: f64, y: f64) -> Position { Position(Vec2Fixed::new(fixed(x), fixed(y))) }
    fn speed(s: f64) -> MoveSpeed { MoveSpeed(fixed(s)) }

    #[test]
    fn entity_moves_toward_target() {
        let mut world = World::new();
        let e = world.spawn((
            pos(0.0, 0.0),
            Velocity::default(),
            MoveTarget(Vec2Fixed::new(fixed(10.0), fixed(0.0))),
            speed(2.0),
        ));

        move_target_system(&mut world);
        movement_system(&mut world);

        let p = world.get::<&Position>(e).unwrap();
        assert!(p.0.x > Fixed::ZERO, "entidade deve ter avançado em X");
    }

    #[test]
    fn rooted_entity_does_not_move() {
        let mut world = World::new();
        let e = world.spawn((
            pos(0.0, 0.0),
            Velocity::default(),
            MoveTarget(Vec2Fixed::new(fixed(10.0), fixed(0.0))),
            speed(2.0),
            CrowdControl { kind: CcKind::Root, ticks_remaining: 5 },
        ));

        move_target_system(&mut world);
        movement_system(&mut world);

        let p = world.get::<&Position>(e).unwrap();
        assert_eq!(p.0.x, Fixed::ZERO, "entidade com root não deve mover");
    }

    #[test]
    fn follows_path_waypoints() {
        let mut world = World::new();
        // Dois waypoints: (5,0) depois (10,0)
        let path = Path {
            waypoints:   vec![
                Vec2Fixed::new(fixed(5.0), fixed(0.0)),
                Vec2Fixed::new(fixed(10.0), fixed(0.0)),
            ],
            current:     0,
            destination: Vec2Fixed::new(fixed(10.0), fixed(0.0)),
        };
        let e = world.spawn((
            pos(0.0, 0.0),
            Velocity::default(),
            MoveTarget(Vec2Fixed::new(fixed(10.0), fixed(0.0))),
            speed(2.0),
            path,
        ));

        // Avança até chegar perto do primeiro waypoint
        for _ in 0..10 {
            move_target_system(&mut world);
            movement_system(&mut world);
        }

        // Deve ter avançado em direção a x=5
        let p = world.get::<&Position>(e).unwrap();
        assert!(p.0.x > fixed(0.0), "deve ter avançado");
    }

    #[test]
    fn path_removed_when_exhausted() {
        let mut world = World::new();
        // Waypoint muito próximo — chega em 1 tick
        let path = Path {
            waypoints:   vec![Vec2Fixed::new(fixed(1.0), fixed(0.0))],
            current:     0,
            destination: Vec2Fixed::new(fixed(1.0), fixed(0.0)),
        };
        let e = world.spawn((
            pos(0.0, 0.0),
            Velocity::default(),
            MoveTarget(Vec2Fixed::new(fixed(1.0), fixed(0.0))),
            speed(2.0), // speed > dist → chega imediatamente
            path,
        ));

        move_target_system(&mut world);

        assert!(world.get::<&Path>(e).is_err(), "Path deve ser removido ao esgotar");
    }
}
