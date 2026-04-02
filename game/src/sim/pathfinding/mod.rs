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
pub fn pathfinding_system(world: &mut World, nav: &NavigationGrid) {
    // Entidades que precisam de novo caminho
    let to_compute: Vec<(hecs::Entity, Vec2Fixed, Vec2Fixed)> = world
        .query::<(&Position, &MoveTarget, Option<&Path>)>()
        .iter()
        .filter_map(|(e, (pos, target, maybe_path))| {
            let stale = match maybe_path {
                None    => true,
                Some(p) => p.destination != target.0,
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
    let sc = nav.world_to_cell(start);
    let gc = nav.world_to_cell(goal);

    let cells = astar::find_path(
        |x, y| nav.is_blocked(x, y),
        nav.width(),
        nav.height(),
        sc,
        gc,
    );

    let mut waypoints: Vec<Vec2Fixed> = cells.iter()
        .map(|&(x, y)| nav.cell_to_world(x, y))
        .collect();

    // Substitui último waypoint pelo destino exato (evita parar no centro da célula)
    match waypoints.last_mut() {
        Some(last) => *last = goal,
        None       => waypoints.push(goal), // start == goal célula ou sem caminho → direto
    }

    Path { waypoints, current: 0, destination: goal }
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
