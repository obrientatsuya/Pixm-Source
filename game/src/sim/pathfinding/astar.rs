/// A* determinístico em grade inteira com 8 direções.
///
/// Retorna caminho de células da origem (exclusivo) ao destino (inclusivo).
/// Tie-breaking por coordenada garante determinismo independente do peer.

use std::collections::BinaryHeap;
use std::cmp::Reverse;

/// `blocked(x, y)` → true se a célula está bloqueada.
/// Retorna Vec de células (x, y) excluindo `start`, incluindo `goal`.
/// Retorna vec![] se start == goal ou caminho inatingível.
pub fn find_path(
    blocked:  impl Fn(i32, i32) -> bool,
    width:    i32,
    height:   i32,
    start:    (i32, i32),
    goal:     (i32, i32),
) -> Vec<(i32, i32)> {
    if start == goal { return vec![]; }

    let idx  = |x: i32, y: i32| (y * width + x) as usize;
    let size = (width * height) as usize;

    let mut g_score:   Vec<i32>              = vec![i32::MAX; size];
    let mut came_from: Vec<Option<(i32,i32)>> = vec![None; size];

    // min-heap: (f, x, y) — tie-break por posição para determinismo
    let mut open: BinaryHeap<Reverse<(i32, i32, i32)>> = BinaryHeap::new();

    g_score[idx(start.0, start.1)] = 0;
    open.push(Reverse((heuristic(start, goal), start.0, start.1)));

    while let Some(Reverse((_, x, y))) = open.pop() {
        if (x, y) == goal {
            return reconstruct(&came_from, width, goal, start);
        }

        let g_cur = g_score[idx(x, y)];

        for (nx, ny, step) in neighbors(x, y) {
            if nx < 0 || ny < 0 || nx >= width || ny >= height { continue; }
            if blocked(nx, ny) { continue; }
            // Evita corte de quina em diagonal
            if nx != x && ny != y && (blocked(x, ny) || blocked(nx, y)) { continue; }

            let new_g = g_cur.saturating_add(step);
            let ni = idx(nx, ny);
            if new_g < g_score[ni] {
                g_score[ni] = new_g;
                came_from[ni] = Some((x, y));
                let f = new_g + heuristic((nx, ny), goal);
                open.push(Reverse((f, nx, ny)));
            }
        }
    }

    vec![] // sem caminho
}

/// Heurística: distância octile × 10 (inteiros, sem float).
fn heuristic(a: (i32, i32), b: (i32, i32)) -> i32 {
    let dx = (a.0 - b.0).abs();
    let dy = (a.1 - b.1).abs();
    10 * (dx + dy) - 6 * dx.min(dy)
}

/// 8 vizinhos com custo: cardinal = 10, diagonal = 14.
fn neighbors(x: i32, y: i32) -> [(i32, i32, i32); 8] {
    [
        (x-1, y-1, 14), (x, y-1, 10), (x+1, y-1, 14),
        (x-1, y,   10),                (x+1, y,   10),
        (x-1, y+1, 14), (x, y+1, 10), (x+1, y+1, 14),
    ]
}

fn reconstruct(
    came_from: &[Option<(i32, i32)>],
    width:     i32,
    goal:      (i32, i32),
    start:     (i32, i32),
) -> Vec<(i32, i32)> {
    let idx = |x: i32, y: i32| (y * width + x) as usize;
    let mut path = Vec::new();
    let mut cur = goal;
    loop {
        path.push(cur);
        match came_from[idx(cur.0, cur.1)] {
            Some(prev) if prev == start => break,
            Some(prev) => cur = prev,
            None => break,
        }
    }
    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_line_no_obstacles() {
        let path = find_path(|_, _| false, 16, 16, (0, 0), (5, 0));
        assert!(!path.is_empty());
        assert_eq!(*path.last().unwrap(), (5, 0));
    }

    #[test]
    fn same_cell_returns_empty() {
        let path = find_path(|_, _| false, 16, 16, (3, 3), (3, 3));
        assert!(path.is_empty());
    }

    #[test]
    fn wall_detour() {
        // Parede vertical x=2, y=0..3 — forçar desvio
        let blocked = |x: i32, y: i32| x == 2 && y < 4;
        let path = find_path(blocked, 16, 16, (0, 2), (4, 2));
        assert!(!path.is_empty());
        assert_eq!(*path.last().unwrap(), (4, 2));
        // Nenhum passo passa pela parede
        assert!(path.iter().all(|(x, y)| !blocked(*x, *y)));
    }

    #[test]
    fn deterministic_same_output() {
        let a = find_path(|_, _| false, 32, 32, (0, 0), (10, 10));
        let b = find_path(|_, _| false, 32, 32, (0, 0), (10, 10));
        assert_eq!(a, b);
    }
}
