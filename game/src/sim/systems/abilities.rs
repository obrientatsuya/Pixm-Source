/// Sistema de habilidades — processa PendingAbility → efeitos.
///
/// Fluxo: InputEvent::Ability → apply_inputs → PendingAbility component
///        → ability_system → DamageEvent / CC / Dash / Projétil

use hecs::{Entity, World};
use crate::core::events::{EventBus, DamageEvent, AbilityEvent};
use crate::core::types::{Fixed, Vec2Fixed};
use crate::sim::components::{
    AbilityEffect, AbilitySlots, AbilityCooldowns, PendingAbility,
    Position, Velocity, Health, Team, CrowdControl, CcKind, MoveTarget, Path,
    Projectile, ProjectileTarget,
};

/// Decrementa cooldowns de habilidades (1 tick por slot).
pub fn ability_cooldown_system(world: &mut World) {
    for (_, cds) in world.query_mut::<&mut AbilityCooldowns>() {
        for cd in cds.0.iter_mut() {
            if *cd > 0 { *cd -= 1; }
        }
    }
}

/// Processa todas as habilidades pendentes deste tick.
pub fn ability_system(world: &mut World, events: &mut EventBus) {
    let pending: Vec<(Entity, PendingAbility)> = world
        .query::<&PendingAbility>()
        .iter()
        .map(|(e, p)| (e, *p))
        .collect();

    for (caster, pend) in pending {
        process_ability(world, events, caster, pend);
        let _ = world.remove_one::<PendingAbility>(caster);
    }
}

fn process_ability(world: &mut World, events: &mut EventBus, caster: Entity, pend: PendingAbility) {
    let slot = pend.slot as usize;
    if slot >= 4 { return; }

    // Lê posição do lançador
    let caster_pos = match world.get::<&Position>(caster) {
        Ok(p) => p.0,
        Err(_) => return,
    };

    // Silence/Stun bloqueiam habilidades
    if let Ok(cc) = world.get::<&CrowdControl>(caster) {
        if matches!(cc.kind, CcKind::Stun | CcKind::Silence) { return; }
    }

    // Lê def e clona para liberar borrow
    let def = {
        let Ok(slots) = world.get::<&AbilitySlots>(caster) else { return };
        match slots.0[slot] {
            Some(d) => d,
            None    => return,
        }
    };

    // Verifica e consome cooldown
    {
        let Ok(mut cds) = world.get::<&mut AbilityCooldowns>(caster) else { return };
        if cds.0[slot] > 0 { return; }
        cds.0[slot] = def.cooldown;
    }

    let target = Vec2Fixed::new(pend.target_x, pend.target_y);

    // Verifica alcance (lançador → ponto alvo)
    if caster_pos.dist_sq(target) > def.range * def.range { return; }

    let my_team = world.get::<&Team>(caster).map(|t| t.0).unwrap_or(255);

    match def.effect {
        AbilityEffect::InstantDamage { amount, hit_radius, cc } => {
            if let Some(hit) = closest_enemy(world, target, hit_radius, my_team) {
                events.damage.emit(DamageEvent { source: caster, target: hit, amount });
                if let Some((kind, ticks)) = cc {
                    let _ = world.insert_one(hit, CrowdControl { kind, ticks_remaining: ticks });
                }
            }
        }
        AbilityEffect::AreaDamage { radius, amount } => {
            for hit in enemies_in_radius(world, target, radius, my_team) {
                events.damage.emit(DamageEvent { source: caster, target: hit, amount });
            }
        }
        AbilityEffect::Dash { distance } => {
            let dir     = (target - caster_pos).normalize();
            let new_pos = caster_pos + dir * distance.min(def.range);
            if let Ok(mut pos) = world.get::<&mut Position>(caster) { pos.0 = new_pos; }
            let _ = world.remove_one::<MoveTarget>(caster);
            let _ = world.remove_one::<Path>(caster);
        }
        AbilityEffect::Projectile { speed, damage } => {
            let dir = (target - caster_pos).normalize();
            world.spawn((
                Position(caster_pos),
                Velocity(dir * speed),
                Projectile {
                    owner:  caster,
                    damage,
                    speed,
                    target: ProjectileTarget::Point(target),
                },
            ));
        }
    }

    events.abilities.emit(AbilityEvent {
        caster,
        ability_slot: pend.slot,
        target_x:     pend.target_x,
        target_y:     pend.target_y,
        target_id:    None,
    });
}

/// Inimigo mais próximo de `pos` dentro de `radius`. Tie-break por entity id.
fn closest_enemy(world: &World, pos: Vec2Fixed, radius: Fixed, my_team: u8) -> Option<Entity> {
    let r2 = radius * radius;
    world.query::<(&Position, &Team, &Health)>()
        .iter()
        .filter(|(_, (p, t, hp))| t.0 != my_team && !hp.is_dead() && pos.dist_sq(p.0) <= r2)
        .min_by_key(|(e, (p, _, _))| (pos.dist_sq(p.0).to_bits(), e.id()))
        .map(|(e, _)| e)
}

/// Todos os inimigos dentro de `radius`, ordenados deterministicamente.
fn enemies_in_radius(world: &World, pos: Vec2Fixed, radius: Fixed, my_team: u8) -> Vec<Entity> {
    let r2 = radius * radius;
    let mut hits: Vec<(Entity, i64)> = world.query::<(&Position, &Team, &Health)>()
        .iter()
        .filter(|(_, (p, t, hp))| t.0 != my_team && !hp.is_dead() && pos.dist_sq(p.0) <= r2)
        .map(|(e, (p, _, _))| (e, pos.dist_sq(p.0).to_bits()))
        .collect();
    hits.sort_by_key(|(e, d)| (*d, e.id()));
    hits.into_iter().map(|(e, _)| e).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixed::types::I32F32;
    use crate::sim::components::{AbilityDef, AbilitySlots, AbilityCooldowns, Dying};

    fn fixed(n: f64) -> I32F32 { I32F32::from_num(n) }
    fn pos(x: f64, y: f64) -> Position { Position(Vec2Fixed::new(fixed(x), fixed(y))) }

    fn spawn_hero(world: &mut World, x: f64, y: f64, team: u8) -> Entity {
        world.spawn((
            pos(x, y), Health::new(100), Team(team),
            AbilitySlots::default(), AbilityCooldowns::default(),
        ))
    }

    fn q_slot() -> AbilityDef {
        AbilityDef {
            range:    fixed(200.0),
            cooldown: 60,
            effect:   AbilityEffect::InstantDamage {
                amount: 50, hit_radius: fixed(40.0), cc: None,
            },
        }
    }

    #[test]
    fn instant_damage_hits_enemy() {
        let mut world = World::new();
        let mut events = EventBus::new();

        let caster = spawn_hero(&mut world, 0.0, 0.0, 0);
        let target = spawn_hero(&mut world, 100.0, 0.0, 1);

        // Equipa Q
        world.get::<&mut AbilitySlots>(caster).unwrap().0[0] = Some(q_slot());
        let _ = world.insert_one(caster, PendingAbility {
            slot: 0, target_x: fixed(100.0), target_y: fixed(0.0),
        });

        ability_system(&mut world, &mut events);

        let dmgs: Vec<_> = events.damage.drain().collect();
        assert_eq!(dmgs.len(), 1);
        assert_eq!(dmgs[0].target, target);
        assert_eq!(dmgs[0].amount, 50);
    }

    #[test]
    fn ability_blocked_by_stun() {
        let mut world = World::new();
        let mut events = EventBus::new();

        let caster = spawn_hero(&mut world, 0.0, 0.0, 0);
        spawn_hero(&mut world, 50.0, 0.0, 1);

        world.get::<&mut AbilitySlots>(caster).unwrap().0[0] = Some(q_slot());
        let _ = world.insert_one(caster, CrowdControl { kind: CcKind::Stun, ticks_remaining: 5 });
        let _ = world.insert_one(caster, PendingAbility {
            slot: 0, target_x: fixed(50.0), target_y: fixed(0.0),
        });

        ability_system(&mut world, &mut events);

        assert_eq!(events.damage.len(), 0);
    }

    #[test]
    fn cooldown_prevents_double_cast() {
        let mut world = World::new();
        let mut events = EventBus::new();

        let caster = spawn_hero(&mut world, 0.0, 0.0, 0);
        spawn_hero(&mut world, 50.0, 0.0, 1);
        world.get::<&mut AbilitySlots>(caster).unwrap().0[0] = Some(q_slot());

        // Primeiro cast
        let _ = world.insert_one(caster, PendingAbility { slot: 0, target_x: fixed(50.0), target_y: fixed(0.0) });
        ability_system(&mut world, &mut events);
        let _ = events.damage.drain().count();

        // Segundo cast imediato (cooldown ativo)
        let _ = world.insert_one(caster, PendingAbility { slot: 0, target_x: fixed(50.0), target_y: fixed(0.0) });
        ability_system(&mut world, &mut events);
        assert_eq!(events.damage.len(), 0, "segundo cast deve ser bloqueado pelo cooldown");
    }
}
