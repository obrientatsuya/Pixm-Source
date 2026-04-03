/// Sistema de projéteis — colisão e movimento homing.
///
/// Linear (skillshot): velocidade constante definida no spawn.
///   Colide com primeiro inimigo no raio. Despawna ao atingir endpoint.
/// Homing (auto-attack missile): atualiza velocidade em direção ao alvo
///   a cada tick até colidir ou o alvo morrer.
///
/// Roda APÓS movement_system, ANTES de health_system.

use hecs::{Entity, World};
use crate::core::events::{EventBus, DamageEvent};
use crate::core::types::{Fixed, Vec2Fixed};
use crate::sim::components::{
    Position, Velocity, Projectile, ProjectileTarget, Health, Team, Dying,
};

/// Raio de colisão (unidades de mundo).  20² = 400
const HIT_R_SQ: i64 = 20 * 20;

pub fn projectile_system(world: &mut World, events: &mut EventBus) {
    let projs: Vec<(Entity, Vec2Fixed, Projectile)> = world
        .query::<(&Position, &Projectile)>()
        .iter()
        .map(|(e, (pos, proj))| (e, pos.0, *proj))
        .collect();

    if projs.is_empty() { return; }

    // Alvos vivos para detecção de colisão
    let live: Vec<(Entity, Vec2Fixed, u8)> = world
        .query::<(&Position, &Team, &Health)>()
        .iter()
        .filter(|(_, (_, _, hp))| !hp.is_dead())
        .map(|(e, (pos, team, _))| (e, pos.0, team.0))
        .collect();

    let mut to_die:     Vec<Entity>               = vec![];
    let mut vel_update: Vec<(Entity, Vec2Fixed)>  = vec![];

    for (proj_e, proj_pos, proj) in projs {
        let owner_team = world.get::<&Team>(proj.owner).map(|t| t.0).unwrap_or(255);
        let hr_sq = Fixed::from_num(HIT_R_SQ);

        match proj.target {
            ProjectileTarget::Point(endpoint) => {
                // Primeiro inimigo dentro do raio da posição atual
                let hit = live.iter()
                    .filter(|(e, _, t)| *e != proj.owner && *t != owner_team)
                    .filter(|(_, ep, _)| proj_pos.dist_sq(*ep) <= hr_sq)
                    .min_by_key(|(e, ep, _)| (proj_pos.dist_sq(*ep).to_bits(), e.id()));

                if let Some((tgt, _, _)) = hit {
                    events.damage.emit(DamageEvent {
                        source: proj.owner, target: *tgt, amount: proj.damage,
                    });
                    to_die.push(proj_e);
                } else if proj_pos.dist_sq(endpoint) <= hr_sq {
                    to_die.push(proj_e); // atingiu endpoint sem acertar ninguém
                }
            }

            ProjectileTarget::Entity(target_e) => {
                // Alvo sumiu ou está morrendo
                let target_pos = match world.get::<&Position>(target_e) {
                    Ok(p) => p.0,
                    Err(_) => { to_die.push(proj_e); continue; }
                };
                if world.get::<&Dying>(target_e).is_ok() {
                    to_die.push(proj_e);
                    continue;
                }

                if proj_pos.dist_sq(target_pos) <= hr_sq {
                    events.damage.emit(DamageEvent {
                        source: proj.owner, target: target_e, amount: proj.damage,
                    });
                    to_die.push(proj_e);
                } else {
                    // Homing: atualiza velocidade em direção ao alvo
                    let dir = (target_pos - proj_pos).normalize();
                    vel_update.push((proj_e, dir * proj.speed));
                }
            }
        }
    }

    for (e, v) in vel_update {
        if let Ok(mut vel) = world.get::<&mut Velocity>(e) { vel.0 = v; }
    }
    for e in to_die {
        let _ = world.insert_one(e, Dying);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixed::types::I32F32;
    use crate::core::events::EventBus;

    fn fixed(n: f64) -> I32F32 { I32F32::from_num(n) }
    fn pos(x: f64, y: f64) -> Position { Position(Vec2Fixed::new(fixed(x), fixed(y))) }

    #[test]
    fn linear_hits_nearby_enemy() {
        let mut world = World::new();
        let mut events = EventBus::new();

        let shooter = world.spawn((pos(0.0, 0.0), Team(0)));
        let target  = world.spawn((pos(15.0, 0.0), Team(1), Health::new(100)));

        // Projétil a 10 unidades do alvo (< HIT_R=20) → colide imediatamente
        world.spawn((
            pos(5.0, 0.0),
            Velocity::default(),
            Projectile {
                owner:  shooter,
                damage: 60,
                speed:  fixed(10.0),
                target: ProjectileTarget::Point(Vec2Fixed::new(fixed(30.0), fixed(0.0))),
            },
        ));

        projectile_system(&mut world, &mut events);

        let dmgs: Vec<_> = events.damage.drain().collect();
        assert_eq!(dmgs.len(), 1);
        assert_eq!(dmgs[0].target, target);
        assert_eq!(dmgs[0].amount, 60);
    }

    #[test]
    fn linear_despawns_at_endpoint() {
        let mut world = World::new();
        let mut events = EventBus::new();

        let shooter = world.spawn((pos(0.0, 0.0), Team(0)));
        // Projétil já no endpoint, sem inimigos
        let proj = world.spawn((
            pos(100.0, 0.0),
            Velocity::default(),
            Projectile {
                owner:  shooter,
                damage: 50,
                speed:  fixed(10.0),
                target: ProjectileTarget::Point(Vec2Fixed::new(fixed(100.0), fixed(0.0))),
            },
        ));

        projectile_system(&mut world, &mut events);

        assert!(world.get::<&Dying>(proj).is_ok(), "deve marcar Dying ao atingir endpoint");
    }

    #[test]
    fn homing_tracks_and_hits() {
        let mut world = World::new();
        let mut events = EventBus::new();

        let shooter = world.spawn((pos(0.0, 0.0), Team(0)));
        let target  = world.spawn((pos(15.0, 0.0), Team(1), Health::new(100)));

        // Homing direto no alvo (dentro do HIT_R)
        world.spawn((
            pos(5.0, 0.0),
            Velocity::default(),
            Projectile {
                owner:  shooter,
                damage: 80,
                speed:  fixed(10.0),
                target: ProjectileTarget::Entity(target),
            },
        ));

        projectile_system(&mut world, &mut events);

        let dmgs: Vec<_> = events.damage.drain().collect();
        assert_eq!(dmgs.len(), 1);
        assert_eq!(dmgs[0].target, target);
    }

    #[test]
    fn projectile_not_owned_team_safe() {
        let mut world = World::new();
        let mut events = EventBus::new();

        let shooter  = world.spawn((pos(0.0, 0.0), Team(0)));
        let friendly = world.spawn((pos(5.0, 0.0), Team(0), Health::new(100))); // mesmo time
        let _ = friendly;

        world.spawn((
            pos(0.0, 0.0),
            Velocity::default(),
            Projectile {
                owner:  shooter,
                damage: 50,
                speed:  fixed(5.0),
                target: ProjectileTarget::Point(Vec2Fixed::new(fixed(20.0), fixed(0.0))),
            },
        ));

        projectile_system(&mut world, &mut events);
        assert_eq!(events.damage.len(), 0, "não deve acertar aliados");
    }
}
