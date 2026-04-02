use fixed::types::I32F32;

/// Tipo fixed-point usado em toda a simulação. NUNCA usar f32/f64 na sim.
pub type Fixed = I32F32;

/// ID único de entidade no ECS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(pub u64);

/// ID de jogador (0..9 numa partida 5v5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, bitcode::Encode, bitcode::Decode)]
pub struct PlayerId(pub u8);

impl PlayerId {
    pub fn team(&self) -> u8 { self.0 / 5 }
}

/// Número de tick da simulação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TickId(pub u64);

/// Vetor 2D em fixed-point — posição, velocidade, direção.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Vec2Fixed {
    pub x: Fixed,
    pub y: Fixed,
}

impl Vec2Fixed {
    pub const ZERO: Self = Self { x: Fixed::ZERO, y: Fixed::ZERO };

    pub fn new(x: Fixed, y: Fixed) -> Self { Self { x, y } }

    /// Distância ao quadrado — evita sqrt (usar para comparações).
    pub fn length_sq(&self) -> Fixed {
        self.x * self.x + self.y * self.y
    }

    /// Distância ao quadrado entre dois pontos.
    pub fn dist_sq(&self, other: Vec2Fixed) -> Fixed {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    /// Normaliza o vetor. Retorna ZERO se o vetor é zero.
    /// Usa sqrt inteira 128-bit para manter determinismo sem float.
    pub fn normalize(&self) -> Self {
        let sq = self.length_sq();
        if sq == Fixed::ZERO { return Self::ZERO; }

        // Para I32F32 (Q32.32): sq_raw = valor * 2^32
        // sqrt_raw correto = sqrt(valor) * 2^32 = sqrt(sq_raw * 2^32)
        let sq_raw = sq.to_bits();
        if sq_raw <= 0 { return Self::ZERO; }
        let extended = (sq_raw as u128) << 32;
        let sqrt_raw = isqrt_u128(extended) as i64;
        let sqrt = Fixed::from_bits(sqrt_raw);
        if sqrt == Fixed::ZERO { return Self::ZERO; }

        Self {
            x: self.x / sqrt,
            y: self.y / sqrt,
        }
    }

    /// Interpolação linear (alpha: 0..1 em Fixed).
    pub fn lerp(&self, other: Vec2Fixed, alpha: Fixed) -> Self {
        Self {
            x: self.x + (other.x - self.x) * alpha,
            y: self.y + (other.y - self.y) * alpha,
        }
    }
}

impl std::ops::Add for Vec2Fixed {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { x: self.x + rhs.x, y: self.y + rhs.y }
    }
}

impl std::ops::Sub for Vec2Fixed {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self { x: self.x - rhs.x, y: self.y - rhs.y }
    }
}

impl std::ops::Mul<Fixed> for Vec2Fixed {
    type Output = Self;
    fn mul(self, rhs: Fixed) -> Self {
        Self { x: self.x * rhs, y: self.y * rhs }
    }
}

/// Integer square root para u128 (Newton-Raphson).
fn isqrt_u128(n: u128) -> u128 {
    if n == 0 { return 0; }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec2_dist_sq() {
        let a = Vec2Fixed::new(Fixed::from_num(3), Fixed::ZERO);
        let b = Vec2Fixed::new(Fixed::from_num(6), Fixed::ZERO);
        assert_eq!(a.dist_sq(b), Fixed::from_num(9));
    }

    #[test]
    fn player_id_team() {
        assert_eq!(PlayerId(0).team(), 0);
        assert_eq!(PlayerId(4).team(), 0);
        assert_eq!(PlayerId(5).team(), 1);
        assert_eq!(PlayerId(9).team(), 1);
    }

    #[test]
    fn vec2_lerp_midpoint() {
        let a = Vec2Fixed::new(Fixed::from_num(0), Fixed::from_num(0));
        let b = Vec2Fixed::new(Fixed::from_num(10), Fixed::from_num(10));
        let mid = a.lerp(b, Fixed::from_num(0.5));
        assert_eq!(mid.x, Fixed::from_num(5));
        assert_eq!(mid.y, Fixed::from_num(5));
    }
}
