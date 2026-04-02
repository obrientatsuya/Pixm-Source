/// RollbackSession — orquestra snapshot, predição e re-simulação.
///
/// Fluxo por tick:
///   1. Coleta inputs (confirmados + preditos)
///   2. Se input atrasado chegou → rollback + re-sim
///   3. Avança 1 tick
///   4. Salva snapshot

use std::collections::HashMap;
use crate::rollback::buffer::{SnapshotBuffer, FrameSnapshot};
use crate::rollback::prediction::{InputPredictor, RawInput};

/// Trait que o jogo implementa — o rollback não conhece a sim.
pub trait Simulation: Clone {
    /// Serializa o estado completo (para snapshot).
    fn serialize(&self) -> Vec<u8>;
    /// Restaura de um snapshot serializado.
    fn deserialize(data: &[u8]) -> Self;
    /// Avança 1 tick com os inputs fornecidos.
    fn step(&mut self, inputs: &[RawInput]);
    /// Hash rápido do estado atual (FNV-1a ou similar).
    fn checksum(&self) -> u64;
    /// Estado atual do RNG determinístico.
    fn rng_state(&self) -> u64;
}

pub struct RollbackSession<S: Simulation> {
    sim:            S,
    snapshots:      SnapshotBuffer,
    predictor:      InputPredictor,
    current_tick:   u64,
    confirmed_tick: u64,
    /// Inputs por tick — confirmados e preditos misturados.
    input_log:      HashMap<u64, Vec<RawInput>>,
    /// Inputs atrasados aguardando processamento.
    pending_late:   Vec<RawInput>,
    /// Quantos rollbacks aconteceram (métrica).
    pub rollback_count: u64,
    pub player_count:   u8,
}

impl<S: Simulation> RollbackSession<S> {
    pub fn new(initial_sim: S, player_count: u8) -> Self {
        Self {
            sim: initial_sim,
            snapshots: SnapshotBuffer::new(),
            predictor: InputPredictor::new(),
            current_tick: 0,
            confirmed_tick: 0,
            input_log: HashMap::new(),
            pending_late: Vec::new(),
            rollback_count: 0,
            player_count,
        }
    }

    /// Avança 1 tick com inputs disponíveis. Prediz inputs faltantes.
    pub fn advance(&mut self, confirmed_inputs: Vec<RawInput>) {
        let tick = self.current_tick;

        // Registra inputs confirmados
        for input in &confirmed_inputs {
            self.predictor.confirm(input.clone());
        }

        // Preenche inputs faltantes com predição
        let mut all_inputs = confirmed_inputs;
        let confirmed_ids: Vec<u8> = all_inputs.iter().map(|i| i.player_id).collect();

        for pid in 0..self.player_count {
            if !confirmed_ids.contains(&pid) {
                all_inputs.push(self.predictor.predict(pid, tick));
            }
        }

        // Salva snapshot ANTES de avançar (para rollback futuro)
        self.save_snapshot(tick);

        // Avança simulação
        self.sim.step(&all_inputs);
        self.input_log.insert(tick, all_inputs);
        self.current_tick += 1;
    }

    /// Recebe input remoto que pode estar atrasado.
    /// Se atrasado: agenda rollback para o próximo update().
    pub fn receive_remote_input(&mut self, input: RawInput) {
        let tick = input.tick;
        self.predictor.confirm(input.clone());

        if tick < self.current_tick {
            // Input atrasado — verifica se predição foi correta
            if let Some(existing) = self.input_log.get(&tick) {
                let predicted = existing.iter().find(|i| i.player_id == input.player_id);
                if let Some(p) = predicted {
                    if self.predictor.check_prediction(p, &input) {
                        self.pending_late.push(input);
                    }
                }
            }
        } else {
            // Input futuro ou do tick atual — armazena normalmente
            self.input_log.entry(tick).or_default().push(input);
        }
    }

    /// Processa rollbacks pendentes. Chamar uma vez por frame.
    pub fn process_rollbacks(&mut self) {
        if self.pending_late.is_empty() { return; }

        // Acha o tick mais antigo que precisa de rollback
        let oldest_late = self.pending_late.iter()
            .map(|i| i.tick)
            .min()
            .unwrap();

        // Substitui inputs preditos pelos confirmados no log
        for input in self.pending_late.drain(..) {
            if let Some(log) = self.input_log.get_mut(&input.tick) {
                if let Some(idx) = log.iter().position(|i| i.player_id == input.player_id) {
                    log[idx] = input;
                }
            }
        }

        // Rollback: restaura snapshot e re-simula
        self.rollback_to(oldest_late);
    }

    fn rollback_to(&mut self, target_tick: u64) {
        let Some(snapshot) = self.snapshots.get(target_tick) else {
            tracing::warn!("rollback para tick {target_tick} falhou: snapshot ausente");
            return;
        };

        self.sim = S::deserialize(&snapshot.state);
        self.rollback_count += 1;

        let end_tick = self.current_tick;
        tracing::debug!("rollback: tick {target_tick} → {end_tick} (re-sim {} frames)", end_tick - target_tick);

        // Re-simula do tick alvo até o tick atual
        for tick in target_tick..end_tick {
            let inputs = self.input_log.get(&tick)
                .cloned()
                .unwrap_or_default();
            self.sim.step(&inputs);
        }
    }

    fn save_snapshot(&mut self, tick: u64) {
        self.snapshots.push(FrameSnapshot {
            tick,
            state: self.sim.serialize(),
            rng_state: self.sim.rng_state(),
            checksum: self.sim.checksum(),
        });
    }

    // ─── Accessors ───────────────────────────────────────────────────────

    pub fn current_tick(&self) -> u64 { self.current_tick }
    pub fn confirmed_tick(&self) -> u64 { self.confirmed_tick }
    pub fn prediction_accuracy(&self) -> f64 { self.predictor.accuracy() }
    pub fn sim(&self) -> &S { &self.sim }

    /// Checksum do estado atual — para broadcast de verificação.
    pub fn checksum(&self) -> u64 { self.sim.checksum() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simulação dummy para testes: soma dos inputs = estado.
    #[derive(Clone, Debug)]
    struct DummySim { value: i64, rng: u64 }

    impl Simulation for DummySim {
        fn serialize(&self) -> Vec<u8> {
            let mut v = self.value.to_le_bytes().to_vec();
            v.extend(&self.rng.to_le_bytes());
            v
        }
        fn deserialize(data: &[u8]) -> Self {
            let value = i64::from_le_bytes(data[..8].try_into().unwrap());
            let rng = u64::from_le_bytes(data[8..16].try_into().unwrap());
            Self { value, rng }
        }
        fn step(&mut self, inputs: &[RawInput]) {
            for inp in inputs {
                if let Some(&b) = inp.data.first() {
                    self.value += b as i64;
                }
            }
            self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        }
        fn checksum(&self) -> u64 { self.value as u64 }
        fn rng_state(&self) -> u64 { self.rng }
    }

    fn input(pid: u8, tick: u64, val: u8) -> RawInput {
        RawInput { player_id: pid, tick, data: vec![val], confirmed: true }
    }

    #[test]
    fn advance_accumulates_state() {
        let sim = DummySim { value: 0, rng: 42 };
        let mut session = RollbackSession::new(sim, 2);

        session.advance(vec![input(0, 0, 10), input(1, 0, 5)]);
        assert_eq!(session.sim().value, 15);
        assert_eq!(session.current_tick(), 1);
    }

    #[test]
    fn rollback_corrects_misprediction() {
        let sim = DummySim { value: 0, rng: 42 };
        let mut session = RollbackSession::new(sim, 2);

        // Tick 0: player 0 manda 10, player 1 ausente → predição = empty
        session.advance(vec![input(0, 0, 10)]);
        assert_eq!(session.sim().value, 10); // 10 + 0 (predição vazia)

        // Tick 1: ambos mandam
        session.advance(vec![input(0, 1, 5), input(1, 1, 5)]);
        assert_eq!(session.sim().value, 20);

        // Input atrasado: player 1 mandou 20 no tick 0 (não 0)
        session.receive_remote_input(input(1, 0, 20));
        session.process_rollbacks();

        // Após rollback: tick 0 = 10+20=30, tick 1 = 30+5+5=40
        assert_eq!(session.sim().value, 40);
        assert_eq!(session.rollback_count, 1);
    }

    #[test]
    fn no_rollback_when_prediction_correct() {
        let sim = DummySim { value: 0, rng: 42 };
        let mut session = RollbackSession::new(sim, 1);

        // Manda input confirmado no tick 0
        session.advance(vec![input(0, 0, 7)]);

        // "Recebe" o mesmo input (confirmação idêntica à predição)
        session.receive_remote_input(input(0, 0, 7));
        session.process_rollbacks();

        assert_eq!(session.rollback_count, 0);
    }
}
