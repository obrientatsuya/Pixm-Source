/// InputCollector — coleta e faz coalescing de inputs por tick.
///
/// Em MOBA, inputs são eventos esparsos. Se o jogador clicou 5x em move
/// no mesmo tick, só o último importa. Abilities não coalescem.

use crate::input::events::InputEvent;

/// Buffer de inputs de um tick, com coalescing.
pub struct InputCollector {
    events: Vec<InputEvent>,
}

impl InputCollector {
    pub fn new() -> Self {
        Self { events: Vec::with_capacity(8) }
    }

    /// Adiciona um input. Move events sobrescrevem o anterior do mesmo jogador.
    pub fn push(&mut self, event: InputEvent) {
        match &event {
            // Move events coalescem — só o último importa
            InputEvent::MoveGround { player_id, .. } |
            InputEvent::AttackMove { player_id, .. } => {
                let pid = *player_id;
                // Remove move anterior do mesmo jogador
                self.events.retain(|e| !is_move_event(e) || e.player_id() != pid);
                self.events.push(event);
            }
            InputEvent::Stop { player_id } => {
                let pid = *player_id;
                self.events.retain(|e| !is_move_event(e) || e.player_id() != pid);
                self.events.push(event);
            }
            // Abilities e items NÃO coalescem — todos são enviados
            _ => {
                self.events.push(event);
            }
        }
    }

    /// Drena todos os inputs do tick e limpa o buffer.
    pub fn drain(&mut self) -> Vec<InputEvent> {
        std::mem::take(&mut self.events)
    }

    /// Sem inputs neste tick — não enviar pacote de rede.
    pub fn is_empty(&self) -> bool { self.events.is_empty() }

    pub fn len(&self) -> usize { self.events.len() }
}

fn is_move_event(event: &InputEvent) -> bool {
    matches!(event,
        InputEvent::MoveGround { .. } |
        InputEvent::AttackMove { .. } |
        InputEvent::Stop       { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixed::types::I32F32;

    fn move_ev(pid: u8, x: f64) -> InputEvent {
        use crate::core::types::PlayerId;
        InputEvent::move_ground(PlayerId(pid), I32F32::from_num(x), I32F32::ZERO)
    }

    fn ability_ev(pid: u8, slot: u8) -> InputEvent {
        use crate::core::types::PlayerId;
        InputEvent::ability(PlayerId(pid), slot, I32F32::ZERO, I32F32::ZERO, None)
    }

    #[test]
    fn move_events_coalesce() {
        let mut col = InputCollector::new();
        col.push(move_ev(0, 10.0));
        col.push(move_ev(0, 20.0));
        col.push(move_ev(0, 30.0));

        let drained = col.drain();
        assert_eq!(drained.len(), 1);
        let (x, _) = drained[0].target_pos().unwrap();
        assert_eq!(x, I32F32::from_num(30.0));
    }

    #[test]
    fn abilities_do_not_coalesce() {
        let mut col = InputCollector::new();
        col.push(ability_ev(0, 0)); // Q
        col.push(ability_ev(0, 1)); // W
        assert_eq!(col.drain().len(), 2);
    }

    #[test]
    fn different_players_independent() {
        let mut col = InputCollector::new();
        col.push(move_ev(0, 10.0));
        col.push(move_ev(1, 20.0)); // jogador diferente
        col.push(move_ev(0, 30.0)); // substitui move do p0, não p1

        let drained = col.drain();
        assert_eq!(drained.len(), 2);
    }

    #[test]
    fn drain_clears_buffer() {
        let mut col = InputCollector::new();
        col.push(move_ev(0, 5.0));
        let _ = col.drain();
        assert!(col.is_empty());
    }
}
