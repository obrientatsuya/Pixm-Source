/// Sistema de IA para minions — waypoints de lane + aggro.
///
/// Minions seguem waypoints lineares da sua lane.
/// Quando inimigo entra em AttackRange, trava AiTarget nele.
/// Quando o alvo morre ou sai do range, retoma movimento de lane.
/// O engine não conhece MOBA — lanes são dados externos passados via LanePaths.

use hecs::World;
use crate::core::types::{Fixed, Vec2Fixed};
use crate::sim::components::{
    Position, MoveTarget, AttackRange, Team, Health,
    AiTarget, WaypointIndex, LaneId,
};

// ─── Recurso: caminhos de lane ────────────────────────────────────────────────

/// Caminhos de lane — dados puros configurados pelo jogo, não pelo engine.
/// Índice = lane_id. Cada Vec é uma sequência linear de waypoints.
#[derive(Debug, Clone, Default)]
pub struct LanePaths(pub Vec<Vec<Vec2Fixed>>);

impl LanePaths {
    pub fn get(&self, lane_id: u8) -> Option<&[Vec2Fixed]> {
        self.0.get(lane_id as usize).map(Vec::as_slice)
    }
}

// ─── Sistemas ─────────────────────────────────────────────────────────────────

/// Aggro: para cada minion, busca inimigo mais próximo em AttackRange.
/// - Se encontrado: insere/mantém AiTarget.
/// - Se nenhum: remove AiTarget (retorna à lane).
/// Tie-break: menor EntityId (determinístico).
pub fn ai_aggro_system(world: &mut World) {
    // Snapshot de alvos potenciais: posição, time, vivo/morto
    let candidates: Vec<(hecs::Entity, Vec2Fixed, u8, bool)> = world
        .query::<(&Position, &Team, &Health)>()
        .iter()
        .map(|(e, (pos, team, hp))| (e, pos.0, team.0, hp.is_dead()))
        .collect();

    // Snapshot de minions: entidades com WaypointIndex (marcador de IA)
    let ai_snapshot: Vec<(hecs::Entity, Vec2Fixed, u8, Fixed)> = world
        .query::<(&Position, &Team, &AttackRange, &WaypointIndex)>()
        .iter()
        .map(|(e, (pos, team, range, _))| (e, pos.0, team.0, range.0))
        .collect();

    // Snapshot de targets atuais para verificação de validade
    let current_targets: Vec<(hecs::Entity, hecs::Entity)> = world
        .query::<&AiTarget>()
        .iter()
        .map(|(e, at)| (e, at.0))
        .collect();

    let mut to_insert: Vec<(hecs::Entity, AiTarget)> = vec![];
    let mut to_remove: Vec<hecs::Entity>              = vec![];

    for (entity, pos, team, range) in ai_snapshot {
        let range_sq = range * range;

        // Verifica se o AiTarget atual ainda é válido (vivo + em range)
        let current_valid = current_targets.iter()
            .find(|(e, _)| *e == entity)
            .and_then(|(_, t)| candidates.iter().find(|(e, ..)| *e == *t))
            .map(|(_, tpos, _, dead)| {
                let dx = pos.x - tpos.x;
                let dy = pos.y - tpos.y;
                !dead && (dx * dx + dy * dy) <= range_sq
            })
            .unwrap_or(false);

        if current_valid { continue; }

        // Busca novo alvo: inimigo vivo mais próximo no range
        let new_target = candidates.iter()
            .filter(|(e, tpos, t, dead)| {
                *t != team && !dead && *e != entity && {
                    let dx = pos.x - tpos.x;
                    let dy = pos.y - tpos.y;
                    dx * dx + dy * dy <= range_sq
                }
            })
            .min_by(|(e1, tpos1, _, _), (e2, tpos2, _, _)| {
                let d1 = {let dx = pos.x - tpos1.x; let dy = pos.y - tpos1.y; dx*dx + dy*dy};
                let d2 = {let dx = pos.x - tpos2.x; let dy = pos.y - tpos2.y; dx*dx + dy*dy};
                d1.cmp(&d2).then_with(|| e1.id().cmp(&e2.id()))
            });

        if let Some((target_entity, ..)) = new_target {
            to_insert.push((entity, AiTarget(*target_entity)));
        } else {
            to_remove.push(entity);
        }
    }

    for (e, at) in to_insert { let _ = world.insert_one(e, at); }
    for e in to_remove        { let _ = world.remove_one::<AiTarget>(e); }
}

/// Waypoints: minions sem AiTarget definem MoveTarget para o próximo waypoint.
/// Quando chegam perto do waypoint atual, avançam o índice.
pub fn ai_waypoint_system(world: &mut World, lanes: &LanePaths) {
    const ARRIVE_THRESHOLD_SQ: f64 = 4.0; // dist² < 4 → chegou (~2 unidades)
    let thresh = Fixed::from_num(ARRIVE_THRESHOLD_SQ);

    // Minions sem AiTarget ativo → definem MoveTarget para o waypoint da lane
    let movers: Vec<(hecs::Entity, u8, u8)> = world
        .query::<(&WaypointIndex, &LaneId)>()
        .iter()
        .map(|(e, (wp, lane))| (e, wp.0, lane.0))
        .collect();

    let has_target: Vec<hecs::Entity> = world
        .query::<&AiTarget>()
        .iter()
        .map(|(e, _)| e)
        .collect();

    for (entity, wp_index, lane_id) in movers {
        if has_target.contains(&entity) { continue; }
        let Some(waypoints) = lanes.get(lane_id) else { continue };
        let Some(&target)   = waypoints.get(wp_index as usize) else { continue };
        let _ = world.insert_one(entity, MoveTarget(target));
    }

    // Avança WaypointIndex quando entidade chegou perto do waypoint atual
    let positions: Vec<(hecs::Entity, Vec2Fixed, u8, u8)> = world
        .query::<(&Position, &WaypointIndex, &LaneId)>()
        .iter()
        .map(|(e, (pos, wp, lane))| (e, pos.0, wp.0, lane.0))
        .collect();

    for (entity, pos, wp_index, lane_id) in positions {
        let Some(waypoints) = lanes.get(lane_id) else { continue };
        let Some(&target)   = waypoints.get(wp_index as usize) else { continue };
        let dx = pos.x - target.x;
        let dy = pos.y - target.y;
        if dx * dx + dy * dy < thresh {
            let max = waypoints.len().saturating_sub(1) as u8;
            if let Ok(mut wi) = world.get::<&mut WaypointIndex>(entity) {
                wi.0 = wi.0.saturating_add(1).min(max);
            }
        }
    }
}

// ─── Testes ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::components::*;

    fn fixed(n: f64) -> Fixed { Fixed::from_num(n) }
    fn at(x: f64, y: f64) -> Position { Position(Vec2Fixed::new(fixed(x), fixed(y))) }

    fn spawn_minion(world: &mut World, pos: (f64, f64), team: u8, lane: u8) -> hecs::Entity {
        world.spawn((
            at(pos.0, pos.1),
            Team(team),
            AttackRange(fixed(5.0)),
            AttackDamage(10),
            AttackSpeed(60),
            AttackCooldown(0),
            Health::new(200),
            WaypointIndex(0),
            LaneId(lane),
        ))
    }

    fn spawn_enemy(world: &mut World, pos: (f64, f64), team: u8) -> hecs::Entity {
        world.spawn((
            at(pos.0, pos.1),
            Team(team),
            Health::new(100),
        ))
    }

    #[test]
    fn aggro_locks_nearest_enemy_in_range() {
        let mut world = World::new();
        let minion = spawn_minion(&mut world, (0.0, 0.0), 0, 0);
        let enemy  = spawn_enemy(&mut world, (3.0, 0.0), 1); // dentro do range=5

        ai_aggro_system(&mut world);

        let target = world.get::<&AiTarget>(minion).unwrap();
        assert_eq!(target.0, enemy);
    }

    #[test]
    fn aggro_ignores_out_of_range_enemy() {
        let mut world = World::new();
        let minion = spawn_minion(&mut world, (0.0, 0.0), 0, 0);
        spawn_enemy(&mut world, (10.0, 0.0), 1); // fora do range=5

        ai_aggro_system(&mut world);

        assert!(world.get::<&AiTarget>(minion).is_err(), "sem alvo fora do range");
    }

    #[test]
    fn aggro_clears_when_enemy_dies() {
        let mut world = World::new();
        let minion = spawn_minion(&mut world, (0.0, 0.0), 0, 0);
        let enemy  = spawn_enemy(&mut world, (3.0, 0.0), 1);

        // Estabelece aggro
        ai_aggro_system(&mut world);
        assert!(world.get::<&AiTarget>(minion).is_ok());

        // Mata o inimigo
        world.get::<&mut Health>(enemy).unwrap().current = 0;

        ai_aggro_system(&mut world);
        assert!(world.get::<&AiTarget>(minion).is_err(), "aggro deve limpar com inimigo morto");
    }

    #[test]
    fn waypoint_sets_move_target() {
        let mut world = World::new();
        let minion = spawn_minion(&mut world, (0.0, 0.0), 0, 0);

        let lanes = LanePaths(vec![
            vec![Vec2Fixed::new(fixed(10.0), fixed(0.0))],
        ]);

        ai_waypoint_system(&mut world, &lanes);

        let mt = world.get::<&MoveTarget>(minion).unwrap();
        assert_eq!(mt.0.x, fixed(10.0));
    }

    #[test]
    fn waypoint_does_not_move_when_in_combat() {
        let mut world = World::new();
        let minion = spawn_minion(&mut world, (0.0, 0.0), 0, 0);
        let enemy  = spawn_enemy(&mut world, (3.0, 0.0), 1);
        let _ = world.insert_one(minion, AiTarget(enemy));

        let lanes = LanePaths(vec![
            vec![Vec2Fixed::new(fixed(10.0), fixed(0.0))],
        ]);

        ai_waypoint_system(&mut world, &lanes);

        // MoveTarget não deve ter sido inserido (minion em combate)
        assert!(world.get::<&MoveTarget>(minion).is_err());
    }

    #[test]
    fn waypoint_index_advances_on_arrival() {
        let mut world = World::new();
        let minion = spawn_minion(&mut world, (10.0, 0.0), 0, 0); // já no wp0

        let lanes = LanePaths(vec![
            vec![
                Vec2Fixed::new(fixed(10.0), fixed(0.0)), // wp0 — minion está aqui
                Vec2Fixed::new(fixed(20.0), fixed(0.0)), // wp1
            ],
        ]);

        ai_waypoint_system(&mut world, &lanes);

        let wi = world.get::<&WaypointIndex>(minion).unwrap();
        assert_eq!(wi.0, 1, "deve avançar para wp1");
    }
}
