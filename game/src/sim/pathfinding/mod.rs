/// Pathfinding — sistema A* determinístico.
///
/// pathfinding_system() converte MoveTarget → Path de waypoints.
/// Só recomputa quando o destino muda (nova ordem de movimento).
/// Movement system segue os waypoints via Path component.

mod grid;
mod astar;

pub use grid::NavigationGrid;

use hecs::World;
use crate::core::types::Vec2Fixed;
use crate::sim::components::{Position, MoveTarget, Path};

/// Computa Path para cada entidade com MoveTarget novo ou modificado.
/// Também recomputa se o herói perdeu LOS para o waypoint atual (saiu da rota por wall-slide).
pub fn pathfinding_system(world: &mut World, nav: &NavigationGrid) {
    let to_compute: Vec<(hecs::Entity, Vec2Fixed, Vec2Fixed)> = world
        .query::<(&Position, &MoveTarget, Option<&Path>)>()
        .iter()
        .filter_map(|(e, (pos, target, maybe_path))| {
            let stale = match maybe_path {
                None    => true,
                Some(p) => {
                    p.destination != target.0
                    // Recomputa se waypoint atual ficou inacessível (wall-slide desviou herói)
                    || p.current_wp().map_or(false, |wp| !line_of_sight(nav, pos.0, wp))
                }
            };
            if stale { Some((e, pos.0, target.0)) } else { None }
        })
        .collect();

    for (entity, start_pos, goal_pos) in to_compute {
        let path = compute_path(nav, start_pos, goal_pos);
        let _ = world.insert_one(entity, path);
    }
}

fn compute_path(nav: &NavigationGrid, start: Vec2Fixed, goal: Vec2Fixed) -> Path {
    let gc = nav.world_to_cell(goal);

    // Destino bloqueado → fica parado (sem waypoints).
    if nav.is_blocked(gc.0, gc.1) {
        return Path { waypoints: vec![], current: 0, destination: goal };
    }

    // Linha de visão livre → caminho direto, sem passar por centros de célula.
    if line_of_sight(nav, start, goal) {
        return Path { waypoints: vec![goal], current: 0, destination: goal };
    }

    let sc = nav.world_to_cell(start);

    let cells = astar::find_path(
        |x, y| nav.is_blocked(x, y),
        nav.width(),
        nav.height(),
        sc,
        gc,
    );

    if cells.is_empty() {
        return Path { waypoints: vec![], current: 0, destination: goal };
    }

    // Converte células → posições world; substitui último pelo goal exato
    let mut raw: Vec<Vec2Fixed> = cells.iter()
        .map(|&(x, y)| nav.cell_to_world(x, y))
        .collect();
    if let Some(last) = raw.last_mut() { *last = goal; }

    // String-pulling: mantém só waypoints onde a LOS muda de direção.
    // Reduz de ~N centros de célula para poucos pontos de virada.
    let waypoints = pull_string(nav, start, raw);

    Path { waypoints, current: 0, destination: goal }
}

/// Reduz waypoints ao mínimo: só mantém pontos onde a LOS muda.
/// Greedy: do ponto atual, avança para o mais distante com LOS direta.
fn pull_string(nav: &NavigationGrid, start: Vec2Fixed, wps: Vec<Vec2Fixed>) -> Vec<Vec2Fixed> {
    if wps.len() <= 1 { return wps; }
    let mut result = Vec::new();
    let mut from   = start;
    let mut i      = 0;
    while i < wps.len() {
        // Encontra o waypoint mais distante com LOS a partir de `from`
        let mut j = wps.len() - 1;
        while j > i && !line_of_sight(nav, from, wps[j]) { j -= 1; }
        result.push(wps[j]);
        from = wps[j];
        i = j + 1;
    }
    result
}

/// Verifica linha de visão via Bresenham — true se nenhuma célula bloqueada.
fn line_of_sight(nav: &NavigationGrid, from: Vec2Fixed, to: Vec2Fixed) -> bool {
    let (mut x, mut y) = nav.world_to_cell(from);
    let (x1,   y1)    = nav.world_to_cell(to);

    let dx =  (x1 - x).abs();
    let dy =  (y1 - y).abs();
    let sx = if x1 > x { 1 } else { -1 };
    let sy = if y1 > y { 1 } else { -1 };
    let mut err = dx - dy;

    loop {
        if nav.is_blocked(x, y) { return false; }
        if x == x1 && y == y1  { break; }
        let e2 = 2 * err;
        if e2 > -dy { err -= dy; x += sx; }
        if e2 <  dx { err += dx; y += sy; }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use hecs::World;
    use fixed::types::I32F32;
    use crate::sim::components::{Velocity, MoveSpeed};

    fn fixed(n: f64) -> I32F32 { I32F32::from_num(n) }

    #[test]
    fn path_created_on_move_target() {
        let mut world = World::new();
        let nav = NavigationGrid::default_128();

        let e = world.spawn((
            Position(Vec2Fixed::new(fixed(0.0), fixed(0.0))),
            Velocity::default(),
            MoveTarget(Vec2Fixed::new(fixed(10.0), fixed(0.0))),
            MoveSpeed(fixed(2.0)),
        ));

        pathfinding_system(&mut world, &nav);

        assert!(world.get::<&Path>(e).is_ok(), "Path deve existir após system");
        let path = world.get::<&Path>(e).unwrap();
        assert_eq!(path.destination, Vec2Fixed::new(fixed(10.0), fixed(0.0)));
        assert!(!path.waypoints.is_empty());
    }

    #[test]
    fn path_not_recomputed_when_same_destination() {
        let mut world = World::new();
        let nav = NavigationGrid::default_128();

        let e = world.spawn((
            Position(Vec2Fixed::new(fixed(0.0), fixed(0.0))),
            Velocity::default(),
            MoveTarget(Vec2Fixed::new(fixed(5.0), fixed(0.0))),
            MoveSpeed(fixed(2.0)),
        ));

        pathfinding_system(&mut world, &nav);
        let len_before = world.get::<&Path>(e).unwrap().waypoints.len();
        pathfinding_system(&mut world, &nav); // segundo tick
        let len_after = world.get::<&Path>(e).unwrap().waypoints.len();
        assert_eq!(len_before, len_after);
    }

    #[test]
    fn path_recomputed_on_new_click() {
        let mut world = World::new();
        let nav = NavigationGrid::default_128();

        let e = world.spawn((
            Position(Vec2Fixed::new(fixed(0.0), fixed(0.0))),
            Velocity::default(),
            MoveTarget(Vec2Fixed::new(fixed(5.0), fixed(0.0))),
            MoveSpeed(fixed(2.0)),
        ));

        pathfinding_system(&mut world, &nav);
        // Simula novo click — troca MoveTarget
        world.get::<&mut MoveTarget>(e).unwrap().0 = Vec2Fixed::new(fixed(20.0), fixed(0.0));
        pathfinding_system(&mut world, &nav);
        let path = world.get::<&Path>(e).unwrap();
        assert_eq!(path.destination, Vec2Fixed::new(fixed(20.0), fixed(0.0)));
    }
}
