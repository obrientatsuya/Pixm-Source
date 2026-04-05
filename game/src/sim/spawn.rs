/// Helpers de spawn — constroem entidades compostas para testes e jogo.
/// Sem lógica de jogo: apenas composição de componentes.

use hecs::World;
use crate::core::types::{Fixed, Vec2Fixed};
use crate::sim::components::*;

/// Boneco alvo estático: vida, time, sem movimento.
pub fn spawn_dummy(world: &mut World, pos: Vec2Fixed, team: u8) -> hecs::Entity {
    world.spawn((
        Position(pos),
        PrevPosition(pos),
        Health::new(500),
        Team(team),
    ))
}

/// Minion com IA de lane: movimento, combate, waypoints.
pub fn spawn_minion(world: &mut World, pos: Vec2Fixed, team: u8, lane_id: u8) -> hecs::Entity {
    world.spawn((
        Position(pos),
        PrevPosition(pos),
        Velocity::default(),
        MoveSpeed(Fixed::from_num(1.5)),
        Health::new(200),
        Team(team),
        AttackRange(Fixed::from_num(5)),
        AttackDamage(15),
        AttackSpeed(90),
        AttackCooldown(0),
        WaypointIndex(0),
        LaneId(lane_id),
    ))
}
