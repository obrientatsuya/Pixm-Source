/// RNG determinístico — Linear Congruential Generator.
///
/// Seed compartilhada entre todos os peers via DHT na fase de loading.
/// Resultado idêntico em qualquer CPU/OS dado a mesma seed e sequência.
/// NUNCA usar rand::thread_rng() na simulação.

pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Próximo u32 pseudo-aleatório.
    pub fn next_u32(&mut self) -> u32 {
        // LCG com constantes de Knuth (período máximo para u64)
        self.state = self.state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.state >> 33) as u32
    }

    /// Valor em [0, max).
    pub fn next_range(&mut self, max: u32) -> u32 {
        if max == 0 { return 0; }
        self.next_u32() % max
    }

    /// True com probabilidade pct/100 (pct: 0..100).
    pub fn next_bool_pct(&mut self, pct: u8) -> bool {
        self.next_range(100) < pct as u32
    }

    /// Dano variável em [min, max].
    pub fn next_damage(&mut self, min: i32, max: i32) -> i32 {
        if min >= max { return min; }
        let range = (max - min + 1) as u32;
        min + self.next_range(range) as i32
    }

    /// Estado atual do RNG — salvo no snapshot para rollback.
    pub fn state(&self) -> u64 { self.state }

    /// Restaura estado de um snapshot.
    pub fn restore(&mut self, state: u64) { self.state = state; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = DeterministicRng::new(42);
        let mut b = DeterministicRng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn range_stays_in_bounds() {
        let mut rng = DeterministicRng::new(999);
        for _ in 0..1000 {
            let v = rng.next_range(10);
            assert!(v < 10);
        }
    }

    #[test]
    fn bool_pct_zero_always_false() {
        let mut rng = DeterministicRng::new(1);
        for _ in 0..100 { assert!(!rng.next_bool_pct(0)); }
    }

    #[test]
    fn bool_pct_hundred_always_true() {
        let mut rng = DeterministicRng::new(1);
        for _ in 0..100 { assert!(rng.next_bool_pct(100)); }
    }

    #[test]
    fn state_restore() {
        let mut rng = DeterministicRng::new(7);
        let _ = rng.next_u32();
        let saved = rng.state();
        let v1 = rng.next_u32();

        rng.restore(saved);
        let v2 = rng.next_u32();
        assert_eq!(v1, v2);
    }
}
