/// Buffer circular de snapshots para rollback.
///
/// Armazena os últimos N frames do estado da simulação.
/// Quando um input atrasado chega, restaura o snapshot do tick correspondente
/// e re-simula até o tick atual.

/// Máximo de frames que podemos voltar no tempo (~267ms a 60Hz).
pub const MAX_ROLLBACK_FRAMES: usize = 16;

/// Snapshot genérico de um frame — estado serializável da simulação.
#[derive(Clone, Debug)]
pub struct FrameSnapshot {
    pub tick: u64,
    pub state: Vec<u8>, // SimState serializado via bitcode
    pub rng_state: u64, // estado do RNG determinístico neste tick
    pub checksum: u64,  // hash para detecção de divergência
}

/// Buffer circular indexado por tick % MAX_ROLLBACK_FRAMES.
pub struct SnapshotBuffer {
    slots: [Option<FrameSnapshot>; MAX_ROLLBACK_FRAMES],
    oldest_tick: u64,
    newest_tick: u64,
}

impl SnapshotBuffer {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
            oldest_tick: 0,
            newest_tick: 0,
        }
    }

    /// Salva snapshot de um tick. Sobrescreve o mais antigo se buffer cheio.
    pub fn push(&mut self, snapshot: FrameSnapshot) {
        let tick = snapshot.tick;
        let idx = (tick as usize) % MAX_ROLLBACK_FRAMES;
        self.slots[idx] = Some(snapshot);

        if tick > self.newest_tick {
            self.newest_tick = tick;
        }
        if self.newest_tick >= MAX_ROLLBACK_FRAMES as u64 {
            self.oldest_tick = self.newest_tick - MAX_ROLLBACK_FRAMES as u64 + 1;
        }
    }

    /// Recupera snapshot de um tick específico (para rollback).
    pub fn get(&self, tick: u64) -> Option<&FrameSnapshot> {
        if tick < self.oldest_tick || tick > self.newest_tick {
            return None;
        }
        let idx = (tick as usize) % MAX_ROLLBACK_FRAMES;
        self.slots[idx].as_ref().filter(|s| s.tick == tick)
    }

    /// Tick mais antigo disponível no buffer.
    pub fn oldest(&self) -> u64 { self.oldest_tick }

    /// Tick mais recente salvo.
    pub fn newest(&self) -> u64 { self.newest_tick }

    /// Quantos snapshots válidos existem no buffer.
    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    pub fn is_empty(&self) -> bool { self.len() == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snap(tick: u64) -> FrameSnapshot {
        FrameSnapshot {
            tick,
            state: vec![tick as u8],
            rng_state: tick * 7,
            checksum: tick * 13,
        }
    }

    #[test]
    fn push_and_get() {
        let mut buf = SnapshotBuffer::new();
        buf.push(make_snap(0));
        buf.push(make_snap(1));
        buf.push(make_snap(2));

        assert_eq!(buf.get(0).unwrap().tick, 0);
        assert_eq!(buf.get(2).unwrap().tick, 2);
        assert!(buf.get(99).is_none());
    }

    #[test]
    fn overwrites_oldest_when_full() {
        let mut buf = SnapshotBuffer::new();
        for i in 0..MAX_ROLLBACK_FRAMES as u64 + 5 {
            buf.push(make_snap(i));
        }
        // Os primeiros 5 foram sobrescritos
        assert!(buf.get(0).is_none());
        assert!(buf.get(4).is_none());
        assert!(buf.get(5).is_some());
        assert!(buf.get(MAX_ROLLBACK_FRAMES as u64 + 4).is_some());
    }

    #[test]
    fn len_tracks_filled_slots() {
        let mut buf = SnapshotBuffer::new();
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());

        for i in 0..5 {
            buf.push(make_snap(i));
        }
        assert_eq!(buf.len(), 5);
    }
}
