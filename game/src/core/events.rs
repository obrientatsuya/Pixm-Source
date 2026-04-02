/// Event bus síncrono — drenado ao fim de cada tick.
///
/// Módulos não se chamam diretamente — comunicam via eventos.
/// Nenhum evento sobrevive além do tick em que foi emitido.

/// Fila de eventos tipada. Drenada ao fim de cada tick.
pub struct EventQueue<T> {
    events: Vec<T>,
}

impl<T> EventQueue<T> {
    pub fn new() -> Self { Self { events: Vec::new() } }

    pub fn emit(&mut self, event: T) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        self.events.drain(..)
    }

    pub fn is_empty(&self) -> bool { self.events.is_empty() }
    pub fn len(&self) -> usize { self.events.len() }
}

// ─── Eventos de simulação ────────────────────────────────────────────────────

use hecs::Entity;

/// Dano aplicado a uma entidade.
#[derive(Debug, Clone)]
pub struct DamageEvent {
    pub source: Entity,
    pub target: Entity,
    pub amount: i32,
}

/// Entidade morreu.
#[derive(Debug, Clone)]
pub struct DeathEvent {
    pub entity:  Entity,
    pub killer:  Entity,
}

/// Buff aplicado ou removido.
#[derive(Debug, Clone)]
pub struct BuffEvent {
    pub target:   Entity,
    pub buff_id:  u16,
    pub applied:  bool, // true = aplicado, false = removido
}

/// Habilidade usada — para efeitos visuais e lógica de jogo.
#[derive(Debug, Clone)]
pub struct AbilityEvent {
    pub caster:    Entity,
    pub ability_slot: u8,
    pub target_x:  crate::core::types::Fixed,
    pub target_y:  crate::core::types::Fixed,
    pub target_id: Option<Entity>,
}

/// Coletânea de todas as filas de eventos por tick.
pub struct EventBus {
    pub damage:  EventQueue<DamageEvent>,
    pub deaths:  EventQueue<DeathEvent>,
    pub buffs:   EventQueue<BuffEvent>,
    pub abilities: EventQueue<AbilityEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            damage:    EventQueue::new(),
            deaths:    EventQueue::new(),
            buffs:     EventQueue::new(),
            abilities: EventQueue::new(),
        }
    }

    /// Drena todas as filas — chamar ao fim de cada tick.
    pub fn clear(&mut self) {
        let _ = self.damage.drain().count();
        let _ = self.deaths.drain().count();
        let _ = self.buffs.drain().count();
        let _ = self.abilities.drain().count();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_queue_drain() {
        let mut q: EventQueue<i32> = EventQueue::new();
        q.emit(1);
        q.emit(2);
        q.emit(3);
        assert_eq!(q.len(), 3);

        let drained: Vec<i32> = q.drain().collect();
        assert_eq!(drained, vec![1, 2, 3]);
        assert!(q.is_empty());
    }
}
