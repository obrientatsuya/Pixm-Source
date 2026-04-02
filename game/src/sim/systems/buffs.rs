/// Sistema de buffs/debuffs e crowd control.
///
/// Opera sobre componentes Buff e CrowdControl — sem lógica específica de jogo.
/// Efeitos concretos (ex: "buff X aumenta dano") são aplicados por sistemas de jogo,
/// não aqui. Este sistema só gerencia duração e remoção.

use hecs::World;
use crate::core::events::{EventBus, BuffEvent};
use crate::sim::components::{Buff, CrowdControl};

/// Decrementa duração de todos os CCs. Remove quando expirar.
pub fn crowd_control_system(world: &mut World) {
    let expired: Vec<hecs::Entity> = world
        .query::<&mut CrowdControl>()
        .iter()
        .filter_map(|(e, cc)| {
            if cc.ticks_remaining > 0 {
                cc.ticks_remaining -= 1;
            }
            if cc.ticks_remaining == 0 { Some(e) } else { None }
        })
        .collect();

    for e in expired {
        let _ = world.remove_one::<CrowdControl>(e);
    }
}

/// Decrementa duração de todos os Buffs. Emite BuffEvent ao remover.
pub fn buff_system(world: &mut World, events: &mut EventBus) {
    let mut to_remove: Vec<(hecs::Entity, u16)> = Vec::new();

    for (entity, buff) in world.query_mut::<&mut Buff>() {
        if buff.ticks_remaining > 0 {
            buff.ticks_remaining -= 1;
        }
        if buff.ticks_remaining == 0 {
            to_remove.push((entity, buff.id));
        }
    }

    for (entity, buff_id) in to_remove {
        let _ = world.remove_one::<Buff>(entity);
        events.buffs.emit(BuffEvent { target: entity, buff_id, applied: false });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::components::{CcKind, CrowdControl, Buff};

    #[test]
    fn cc_decrements_and_removes() {
        let mut world = World::new();
        let e = world.spawn((CrowdControl { kind: CcKind::Stun, ticks_remaining: 2 },));

        crowd_control_system(&mut world);
        assert!(world.get::<&CrowdControl>(e).is_ok());
        assert_eq!(world.get::<&CrowdControl>(e).unwrap().ticks_remaining, 1);

        crowd_control_system(&mut world);
        assert!(world.get::<&CrowdControl>(e).is_err(), "CC deve ser removido ao expirar");
    }

    #[test]
    fn buff_emits_event_on_expiry() {
        let mut world = World::new();
        let mut events = EventBus::new();
        let e = world.spawn((Buff { id: 7, stacks: 1, ticks_remaining: 1, magnitude: 10 },));

        buff_system(&mut world, &mut events);

        assert!(world.get::<&Buff>(e).is_err(), "buff deve ser removido");
        let emitted: Vec<_> = events.buffs.drain().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].buff_id, 7);
        assert!(!emitted[0].applied);
    }
}
