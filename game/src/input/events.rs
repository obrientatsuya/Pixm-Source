/// InputEvent — representação interna de um input de jogador.
///
/// Serializado via bitcode para RawInput antes de ir para o rollback/rede.
/// Fixed-point armazenado como raw bits (i64) — bitcode não suporta I32F32 diretamente.

use bitcode::{Encode, Decode};
use crate::core::types::{Fixed, PlayerId};

/// Converte Fixed → bits para serialização.
pub fn fixed_to_bits(f: Fixed) -> i64 { f.to_bits() }
/// Converte bits → Fixed para desserialização.
pub fn bits_to_fixed(b: i64) -> Fixed { Fixed::from_bits(b) }

/// Um input discreto de um jogador num tick.
#[derive(Debug, Clone, Encode, Decode)]
pub enum InputEvent {
    /// Click no chão — mover para posição (x, y em bits de Fixed).
    MoveGround { player_id: PlayerId, x_bits: i64, y_bits: i64 },

    /// Click em entidade inimiga — mover + atacar.
    MoveAttack { player_id: PlayerId, target_entity: u64 },

    /// Parar movimento.
    Stop { player_id: PlayerId },

    /// Attack-move (A+click).
    AttackMove { player_id: PlayerId, x_bits: i64, y_bits: i64 },

    /// Habilidade ativada (slot: 0=Q 1=W 2=E 3=R 4=D 5=F).
    Ability {
        player_id: PlayerId,
        slot:      u8,
        x_bits:    i64,
        y_bits:    i64,
        target_id: Option<u64>,
    },

    /// Item ativado (slot 0-5).
    Item { player_id: PlayerId, slot: u8 },
}

impl InputEvent {
    pub fn player_id(&self) -> PlayerId {
        match self {
            Self::MoveGround  { player_id, .. } => *player_id,
            Self::MoveAttack  { player_id, .. } => *player_id,
            Self::Stop        { player_id }     => *player_id,
            Self::AttackMove  { player_id, .. } => *player_id,
            Self::Ability     { player_id, .. } => *player_id,
            Self::Item        { player_id, .. } => *player_id,
        }
    }

    /// Helpers para construir com Fixed diretamente.
    pub fn move_ground(player_id: PlayerId, x: Fixed, y: Fixed) -> Self {
        Self::MoveGround { player_id, x_bits: fixed_to_bits(x), y_bits: fixed_to_bits(y) }
    }

    pub fn attack_move(player_id: PlayerId, x: Fixed, y: Fixed) -> Self {
        Self::AttackMove { player_id, x_bits: fixed_to_bits(x), y_bits: fixed_to_bits(y) }
    }

    pub fn ability(player_id: PlayerId, slot: u8, x: Fixed, y: Fixed, target_id: Option<u64>) -> Self {
        Self::Ability { player_id, slot, x_bits: fixed_to_bits(x), y_bits: fixed_to_bits(y), target_id }
    }

    /// Extrai coordenadas Fixed de eventos de movimento.
    pub fn target_pos(&self) -> Option<(Fixed, Fixed)> {
        match self {
            Self::MoveGround { x_bits, y_bits, .. } |
            Self::AttackMove { x_bits, y_bits, .. } |
            Self::Ability    { x_bits, y_bits, .. } =>
                Some((bits_to_fixed(*x_bits), bits_to_fixed(*y_bits))),
            _ => None,
        }
    }

    pub fn to_raw_bytes(&self) -> Vec<u8> { bitcode::encode(self) }
    pub fn from_raw_bytes(bytes: &[u8]) -> Option<Self> { bitcode::decode(bytes).ok() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixed::types::I32F32;

    #[test]
    fn move_ground_roundtrip() {
        let ev = InputEvent::move_ground(
            PlayerId(3),
            I32F32::from_num(100),
            I32F32::from_num(-50),
        );
        let bytes = ev.to_raw_bytes();
        let decoded = InputEvent::from_raw_bytes(&bytes).unwrap();
        let (x, y) = decoded.target_pos().unwrap();
        assert_eq!(x, I32F32::from_num(100));
        assert_eq!(y, I32F32::from_num(-50));
    }

    #[test]
    fn player_id_preserved() {
        let ev = InputEvent::Stop { player_id: PlayerId(7) };
        let decoded = InputEvent::from_raw_bytes(&ev.to_raw_bytes()).unwrap();
        assert_eq!(decoded.player_id(), PlayerId(7));
    }
}
