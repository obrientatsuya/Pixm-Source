/// Predição de input de peers ausentes (dead reckoning).
///
/// Quando um peer não mandou input pra um tick, predizemos com base
/// no último input confirmado. Se a predição estava errada, o rollback corrige.

use std::collections::HashMap;

/// Input genérico — bytes brutos serializados via bitcode.
/// O módulo de rollback não conhece a estrutura interna do input.
#[derive(Debug, Clone)]
pub struct RawInput {
    pub player_id: u8,
    pub tick:      u64,
    pub data:      Vec<u8>,
    pub confirmed: bool, // true = recebido do peer, false = predito
}

/// Rastreia último input confirmado de cada peer para predição.
pub struct InputPredictor {
    /// Último input confirmado por peer.
    last_confirmed: HashMap<u8, RawInput>,
    /// Número total de predições feitas (métrica).
    pub predictions_made: u64,
    /// Número de predições que estavam erradas (correções via rollback).
    pub mispredictions: u64,
}

impl InputPredictor {
    pub fn new() -> Self {
        Self {
            last_confirmed: HashMap::new(),
            predictions_made: 0,
            mispredictions: 0,
        }
    }

    /// Registra input confirmado recebido de um peer.
    pub fn confirm(&mut self, input: RawInput) {
        let pid = input.player_id;
        self.last_confirmed.insert(pid, input);
    }

    /// Gera predição para um peer que não mandou input pra este tick.
    /// Estratégia: repete último input confirmado.
    /// Se nunca recebemos input deste peer: retorna input vazio.
    pub fn predict(&mut self, player_id: u8, tick: u64) -> RawInput {
        self.predictions_made += 1;

        match self.last_confirmed.get(&player_id) {
            Some(last) => RawInput {
                player_id,
                tick,
                data: last.data.clone(),
                confirmed: false,
            },
            None => RawInput {
                player_id,
                tick,
                data: Vec::new(), // input vazio = "parado"
                confirmed: false,
            },
        }
    }

    /// Verifica se o input real difere da predição para este tick.
    /// Se sim, incrementa mispredictions e retorna true (rollback necessário).
    pub fn check_prediction(&mut self, predicted: &RawInput, actual: &RawInput) -> bool {
        if predicted.data != actual.data {
            self.mispredictions += 1;
            true
        } else {
            false
        }
    }

    /// Taxa de predição correta (0.0 .. 1.0).
    pub fn accuracy(&self) -> f64 {
        if self.predictions_made == 0 { return 1.0; }
        1.0 - (self.mispredictions as f64 / self.predictions_made as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(pid: u8, tick: u64, data: &[u8]) -> RawInput {
        RawInput {
            player_id: pid,
            tick,
            data: data.to_vec(),
            confirmed: true,
        }
    }

    #[test]
    fn predict_repeats_last_confirmed() {
        let mut pred = InputPredictor::new();
        pred.confirm(make_input(1, 10, &[0x01, 0x02]));

        let predicted = pred.predict(1, 11);
        assert_eq!(predicted.data, &[0x01, 0x02]);
        assert!(!predicted.confirmed);
        assert_eq!(predicted.tick, 11);
    }

    #[test]
    fn predict_unknown_peer_returns_empty() {
        let mut pred = InputPredictor::new();
        let predicted = pred.predict(99, 0);
        assert!(predicted.data.is_empty());
    }

    #[test]
    fn misprediction_detected() {
        let mut pred = InputPredictor::new();
        pred.confirm(make_input(1, 10, &[0x01]));

        let predicted = pred.predict(1, 11);
        let actual = make_input(1, 11, &[0xFF]); // diferente!
        assert!(pred.check_prediction(&predicted, &actual));
        assert_eq!(pred.mispredictions, 1);
    }

    #[test]
    fn correct_prediction_no_rollback() {
        let mut pred = InputPredictor::new();
        pred.confirm(make_input(1, 10, &[0x01]));

        let predicted = pred.predict(1, 11);
        let actual = make_input(1, 11, &[0x01]); // igual
        assert!(!pred.check_prediction(&predicted, &actual));
        assert_eq!(pred.mispredictions, 0);
    }

    #[test]
    fn accuracy_tracks_correctly() {
        let mut pred = InputPredictor::new();
        pred.predictions_made = 100;
        pred.mispredictions = 5;
        assert!((pred.accuracy() - 0.95).abs() < 0.001);
    }
}
