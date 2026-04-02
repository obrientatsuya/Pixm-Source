use std::time::{Duration, Instant};

const MAX_PENDING: usize = 64;
const RETRY_INTERVAL: Duration = Duration::from_millis(50);

/// Estado de ACK por peer — rastreia sequências enviadas e recebidas.
pub struct PeerAckState {
    /// Próxima sequência a enviar para este peer
    local_seq: u8,
    /// Última sequência recebida deste peer
    remote_ack: u8,
    /// Bitmask dos últimos 32 pacotes recebidos (bit 0 = remote_ack - 1, etc.)
    recv_history: u32,
    /// Pacotes reliable aguardando confirmação
    pending: [Option<ReliablePending>; MAX_PENDING],
}

#[derive(Clone)]
struct ReliablePending {
    sequence: u8,
    data:     Vec<u8>,
    sent_at:  Instant,
}

impl PeerAckState {
    pub fn new() -> Self {
        Self {
            local_seq:    0,
            remote_ack:   0,
            recv_history: 0,
            pending:      std::array::from_fn(|_| None),
        }
    }

    /// Chama ao enviar um pacote. Retorna o número de sequência alocado.
    pub fn on_send(&mut self, reliable: bool, data: &[u8]) -> u8 {
        let seq = self.local_seq;
        self.local_seq = self.local_seq.wrapping_add(1);

        if reliable {
            self.insert_pending(ReliablePending {
                sequence: seq,
                data: data.to_vec(),
                sent_at: Instant::now(),
            });
        }

        seq
    }

    /// Chama ao receber pacote de um peer. Atualiza histórico de recv.
    pub fn on_recv(&mut self, sequence: u8) {
        let delta = sequence.wrapping_sub(self.remote_ack) as i8;

        if delta > 0 {
            // Pacote mais novo: desloca histórico
            let shift = delta as u32;
            self.recv_history = self.recv_history.checked_shl(shift).unwrap_or(0);
            self.recv_history |= 1 << (shift - 1);
            self.remote_ack = sequence;
        } else if delta < 0 {
            // Pacote antigo chegou fora de ordem
            let bit = (-delta - 1) as u32;
            if bit < 32 {
                self.recv_history |= 1 << bit;
            }
        }
        // delta == 0: duplicata, ignora
    }

    /// Processa ACK recebido de um peer — remove pendentes confirmados.
    pub fn process_remote_ack(&mut self, ack: u8, ack_bits: u32) {
        self.remove_pending(ack);
        for bit in 0..32u8 {
            if ack_bits & (1 << bit) != 0 {
                let seq = ack.wrapping_sub(bit + 1);
                self.remove_pending(seq);
            }
        }
    }

    /// Campos a incluir no header de todo pacote enviado para este peer.
    pub fn header_ack_fields(&self) -> (u8, u32) {
        (self.remote_ack, self.recv_history)
    }

    /// Retorna dados de pacotes reliable que precisam ser reenviados.
    pub fn take_expired(&mut self) -> Vec<(u8, Vec<u8>)> {
        let mut expired = Vec::new();
        for slot in self.pending.iter_mut() {
            if let Some(p) = slot {
                if p.sent_at.elapsed() >= RETRY_INTERVAL {
                    expired.push((p.sequence, p.data.clone()));
                    p.sent_at = Instant::now();
                }
            }
        }
        expired
    }

    fn insert_pending(&mut self, pending: ReliablePending) {
        for slot in self.pending.iter_mut() {
            if slot.is_none() {
                *slot = Some(pending);
                return;
            }
        }
        // Buffer cheio: descarta o mais antigo
        let oldest = self.pending.iter_mut()
            .filter_map(|s| s.as_mut())
            .min_by_key(|p| p.sent_at);
        if let Some(p) = oldest {
            *p = pending;
        }
    }

    fn remove_pending(&mut self, sequence: u8) {
        for slot in self.pending.iter_mut() {
            if slot.as_ref().map(|p| p.sequence) == Some(sequence) {
                *slot = None;
                return;
            }
        }
    }
}
