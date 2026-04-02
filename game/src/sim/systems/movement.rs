/// Sistemas de movimento — move_target e integração de posição.
///
/// Sem lógica de jogo — opera sobre componentes genéricos.
/// Funciona para heróis, minions, projéteis — qualquer entidade com os componentes certos.

use hecs::World;
use crate::core::types::Vec2Fixed;
use crate::sim::components::{Position, Velocity, MoveTarget, MoveSpeed, CrowdControl, CcKind};

/// Resolve MoveTarget → Velocity.
/// Entidades com Root/Stun não se movem.
pub fn move_target_system(world: &mut World) {
    let rooted: Vec<hecs::Entity> = world
        .query::<&CrowdControl>()
        .iter()
        .filter(|(_, cc)| matches!(cc.kind, CcKind::Root | CcKind::Stun | CcKind::Knockup))
        .map(|(e, _)| e)
        .collect();

    for (entity, (pos, vel, target, speed)) in world
        .query_mut::<(&Position, &mut Velocity, &MoveTarget, &MoveSpeed)>()
    {
        if rooted.contains(&entity) {
            vel.0 = Vec2Fixed::ZERO;
            continue;
        }

        let dir = target.0 - pos.0;
        let dist_sq = dir.length_sq();
        let arrival_sq = speed.0 * speed.0; // chegou se dist < speed (1 tick)

        if dist_sq <= arrival_sq {
            // Chegou ao destino
            vel.0 = Vec2Fixed::ZERO;
        } else {
            vel.0 = dir.normalize() * speed.0;
        }
    }
}

/// Integra velocidade → posição (1 tick).
pub fn movement_system(world: &mut World) {
    for (_, (pos, vel)) in world.query_mut::<(&mut Position, &Velocity)>() {
        pos.0 = pos.0 + vel.0;
    }
}

/// Remove MoveTarget quando entidade chegou (velocidade zerada).
pub fn clear_arrived_targets(world: &mut World) {
    let arrived: Vec<hecs::Entity> = world
        .query::<(&Velocity, &MoveTarget)>()
        .iter()
        .filter(|(_, (vel, _))| vel.0 == Vec2Fixed::ZERO)
        .map(|(e, _)| e)
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
}
