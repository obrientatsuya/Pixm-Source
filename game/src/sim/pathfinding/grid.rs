/// NavigationGrid — grade de obstáculos para o A*.
///
/// Células são inteiras; mundo usa Fixed. Conversão na borda.
/// Representação interna: bitset de u64 (1 bit = 1 célula).

use crate::core::types::{Fixed, Vec2Fixed};

pub struct NavigationGrid {
    width:     i32,
    height:    i32,
    cell_size: Fixed,   // tamanho de cada célula em unidades Fixed
    origin:    Vec2Fixed, // canto superior-esquerdo da grade
    blocked:   Vec<u64>,  // bitset: 1 = bloqueado
}

impl NavigationGrid {
    pub fn new(width: i32, height: i32, cell_size: Fixed, origin: Vec2Fixed) -> Self {
        let n_bits = (width * height) as usize;
        let n_u64  = (n_bits + 63) / 64;
        Self { width, height, cell_size, origin, blocked: vec![0u64; n_u64] }
    }

    /// Grade vazia 128×128 com célula de 1 unidade.
    pub fn default_128() -> Self {
        use fixed::types::I32F32;
        Self::new(128, 128, I32F32::from_num(1), Vec2Fixed::ZERO)
    }

    pub fn width(&self)  -> i32 { self.width }
    pub fn height(&self) -> i32 { self.height }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    pub fn is_blocked(&self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) { return true; }
        let idx = (y * self.width + x) as usize;
        (self.blocked[idx / 64] >> (idx % 64)) & 1 == 1
    }

    pub fn set_blocked(&mut self, x: i32, y: i32, blocked: bool) {
        if !self.in_bounds(x, y) { return; }
        let idx = (y * self.width + x) as usize;
        if blocked {
            self.blocked[idx / 64] |= 1 << (idx % 64);
        } else {
            self.blocked[idx / 64] &= !(1 << (idx % 64));
        }
    }

    /// Converte posição world → célula (clampado na grade).
    pub fn world_to_cell(&self, pos: Vec2Fixed) -> (i32, i32) {
        let dx = pos.x - self.origin.x;
        let dy = pos.y - self.origin.y;
        let cx = (dx / self.cell_size).to_num::<i32>().clamp(0, self.width - 1);
        let cy = (dy / self.cell_size).to_num::<i32>().clamp(0, self.height - 1);
        (cx, cy)
    }

    /// Centro da célula em coordenadas world.
    pub fn cell_to_world(&self, x: i32, y: i32) -> Vec2Fixed {
        let half = self.cell_size / fixed::types::I32F32::from_num(2);
        Vec2Fixed {
            x: self.origin.x + Fixed::from_num(x) * self.cell_size + half,
            y: self.origin.y + Fixed::from_num(y) * self.cell_size + half,
        }
    }
}

impl Clone for NavigationGrid {
    fn clone(&self) -> Self {
        Self {
            width:     self.width,
            height:    self.height,
            cell_size: self.cell_size,
            origin:    self.origin,
            blocked:   self.blocked.clone(),
        }
    }
}
