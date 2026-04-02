/// Sistemas de combate — auto-attack, dano, morte.

use hecs::World;
use crate::core::events::{EventBus, DamageEvent, DeathEvent};
use crate::core::types::Fixed;
use crate::sim::components::{
    Position, Health, AttackRange, AttackDamage, AttackSpeed,
    AttackCooldown, CritChance, Team, Dying,
};
use crate::sim::rng::DeterministicRng;

/// Decrementa cooldowns de auto-attack por 1 tick.
pub fn cooldown_system(world: &mut World) {
    for (_, cd) in world.query_mut::<&mut AttackCooldown>() {
        if cd.0 > 0 { cd.0 -= 1; }
    }
}

/// Auto-attack: entidades com AttackRange buscam alvo em range e atacam.
/// Tie-break: menor HP → menor EntityId (determinístico).
pub fn auto_attack_system(world: &mut World, rng: &mut DeterministicRng, events: &mut EventBus) {
    // Coleta posições e times de todos para lookup
    let targets: Vec<(hecs::Entity, Fixed, Fixed, u8, i32)> = world
        .query::<(&Position, &Team, &Health)>()
        .iter()
        .map(|(e, (pos, team, hp))| (e, pos.0.x, pos.0.y, team.0, hp.current))
        .collect();

    for (attacker, (pos, team, range, dmg, speed, cd, crit)) in world
        .query_mut::<(
            &Position, &Team, &AttackRange, &AttackDamage,
            &AttackSpeed, &mut AttackCooldown, &CritChance,
        )>()
    {
        if cd.0 > 0 { continue; }

        let range_sq = range.0 * range.0;

        // Seleciona alvo: menor HP no range, tie-break menor entity bits
        let target = targets.iter()
            .filter(|(e, tx, ty, t, hp)| {
                *t != team.0 && *hp > 0 && *e != attacker && {
                    let dx = pos.0.x - Fixed::from_num(*tx);
                    let dy = pos.0.y - Fixed::from_num(*ty);
                    dx * dx + dy * dy <= range_sq
                }
            })
            .min_by_key(|(e, _, _, _, hp)| (*hp, e.id()));

        if let Some((target_entity, ..)) = target {
            let is_crit = rng.next_bool_pct(crit.0);
            let amount = if is_crit { dmg.0 * 2 } else { dmg.0 };

            events.damage.emit(DamageEvent {
                source: attacker,
                target: *target_entity,
                amount,
            });

            cd.0 = speed.0;
        }
    }
}

/// Aplica DamageEvents → atualiza Health, emite DeathEvent.
pub fn health_system(world: &mut World, events: &mut EventBus) {
    let damage_events: Vec<DamageEvent> = events.damage.drain().collect();

    for evt in damage_events {
        if let Ok(mut hp) = world.get::<&mut Health>(evt.target) {
            hp.apply_damage(evt.amount);
            if hp.is_dead() {
                events.deaths.emit(DeathEvent {
                    entity: evt.target,
                    killer: evt.source,
                });
            }
        }
    }
}

/// Marca entidades mortas com Dying (removidas no cleanup).
pub fn death_system(world: &mut World, events: &mut EventBus) {
    let death_events: Vec<DeathEvent> = events.deaths.drain().collect();
    for evt in death_events {
        tracing::debug!("entity {:?} morreu para {:?}", evt.entity, evt.killer);
        let _ = world.insert_one(evt.entity, Dying);
    }
}

/// Remove entidades marcadas com Dying.
pub fn cleanup_system(world: &mut World) {
    let dying: Vec<hecs::Entity> = world
        .query::<&Dying>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    for e in dying {
        let _ = world.despawn(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::Vec2Fixed;
    use crate::sim::components::*;

    fn make_world_with_two_entities() -> (World, hecs::Entity, hecs::Entity) {
        let mut world = World::new();
        let a = world.spawn((
            Position(Vec2Fixed::ZERO),
            Team(0),
            AttackRange(Fixed::from_num(5)),
            AttackDamage(100),
            AttackSpeed(60),
            AttackCooldown(0),
            CritChance(0),
            Health::new(500),
        ));
        let b = world.spawn((
            Position(Vec2Fixed::ZERO), // mesmo ponto = em range
            Team(1),
            Health::new(200),
        ));
        (world, a, b)
    }

    #[test]
    fn auto_attack_deals_damage() {
        let (mut world, _a, b) = make_world_with_two_entities();
        let mut rng = DeterministicRng::new(42);
        let mut events = EventBus::new();

        auto_attack_system(&mut world, &mut rng, &mut events);
        health_system(&mut world, &mut events);

        let hp = world.get::<&Health>(b).unwrap();
        assert!(hp.current < 200, "alvo deve ter tomado dano");
    }

    #[test]
    fn dead_entity_cleaned_up() {
        let mut world = World::new();
        let e = world.spawn((Health::new(1), Team(0)));
        let mut events = EventBus::new();

        events.deaths.emit(DeathEvent { entity: e, killer: e });
        death_system(&mut world, &mut events);
        cleanup_system(&mut world);

        assert!(world.contains(e) == false);
    }
}
