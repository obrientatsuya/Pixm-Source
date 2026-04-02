/// Converte InputEvent do Godot → InputEvent da simulação.
///
/// Responsabilidade única: tradução de fronteira.
/// Zero lógica de jogo aqui.

use godot::prelude::*;
use crate::input::events::InputEvent;
use crate::core::types::{Fixed, PlayerId};

/// Converte coordenadas Godot (f32 Vector2) → fixed-point da sim.
/// Chamado apenas no adaptador — f32 só existe aqui, nunca na sim.
pub fn godot_vec_to_fixed(v: Vector2) -> (Fixed, Fixed) {
    (Fixed::from_num(v.x), Fixed::from_num(v.y))
}

/// Converte clique no chão (Godot) → InputEvent::MoveGround.
pub fn on_ground_click(player_id: u8, world_pos: Vector2) -> InputEvent {
    let (x, y) = godot_vec_to_fixed(world_pos);
    InputEvent::move_ground(PlayerId(player_id), x, y)
}

/// Converte clique em entidade inimiga → InputEvent::MoveAttack.
pub fn on_entity_click(player_id: u8, entity_id: u64) -> InputEvent {
    InputEvent::MoveAttack { player_id: PlayerId(player_id), target_entity: entity_id }
}

/// Converte tecla de habilidade → InputEvent::Ability.
pub fn on_ability_key(
    player_id: u8,
    slot:      u8,
    world_pos: Vector2,
    target_id: Option<u64>,
) -> InputEvent {
    let (x, y) = godot_vec_to_fixed(world_pos);
    InputEvent::ability(PlayerId(player_id), slot, x, y, target_id)
}

/// Converte tecla S (stop) → InputEvent::Stop.
pub fn on_stop_key(player_id: u8) -> InputEvent {
    InputEvent::Stop { player_id: PlayerId(player_id) }
}
