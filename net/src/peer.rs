/// Estado de cada peer na partida — RTT, jitter, status de conexão.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(50);
const DISCONNECT_TIMEOUT: Duration = Duration::from_millis(150); // 3 heartbeats

/// Status de conexão de um peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerStatus {
    Connecting,
    Connected,
    Disconnected,
}

/// Estatísticas de rede medidas continuamente.
#[derive(Debug, Clone, Copy)]
pub struct PeerStats {
    pub rtt_ms:       f32,
    pub jitter_ms:    f32,
    pub packet_loss:  f32, // 0.0 .. 1.0
    pub input_delay:  u32, // frames de delay adaptativo para este peer
}

impl Default for PeerStats {
    fn default() -> Self {
        Self { rtt_ms: 0.0, jitter_ms: 0.0, packet_loss: 0.0, input_delay: 1 }
    }
}

/// Um peer na partida.
pub struct Peer {
    pub id:          u8,
    pub addr:        SocketAddr,
    pub status:      PeerStatus,
    pub stats:       PeerStats,
    pub team:        u8,
    last_seen:       Instant,
    last_heartbeat:  Instant,
    rtt_samples:     [f32; 16],
    rtt_idx:         usize,
    packets_sent:    u64,
    packets_lost:    u64,
}

impl Peer {
    pub fn new(id: u8, addr: SocketAddr, team: u8) -> Self {
        Self {
            id, addr, team,
            status: PeerStatus::Connecting,
            stats: PeerStats::default(),
            last_seen: Instant::now(),
            last_heartbeat: Instant::now(),
            rtt_samples: [0.0; 16],
            rtt_idx: 0,
            packets_sent: 0,
            packets_lost: 0,
        }
    }

    /// Atualiza ao receber qualquer pacote deste peer.
    pub fn on_packet_received(&mut self) {
        self.last_seen = Instant::now();
        if self.status == PeerStatus::Connecting {
            self.status = PeerStatus::Connected;
        }
    }

    /// Registra amostra de RTT (medida pelo ACK).
    pub fn record_rtt(&mut self, rtt_ms: f32) {
        let prev = self.stats.rtt_ms;
        self.rtt_samples[self.rtt_idx % 16] = rtt_ms;
        self.rtt_idx += 1;

        // Média das últimas 16 amostras
        let count = self.rtt_idx.min(16);
        let sum: f32 = self.rtt_samples[..count].iter().sum();
        self.stats.rtt_ms = sum / count as f32;

        // Jitter = variação média entre amostras consecutivas
        self.stats.jitter_ms = (rtt_ms - prev).abs() * 0.1 + self.stats.jitter_ms * 0.9;

        self.update_input_delay();
    }

    /// Registra pacote enviado/perdido para cálculo de loss.
    pub fn record_send(&mut self) { self.packets_sent += 1; }
    pub fn record_loss(&mut self) {
        self.packets_lost += 1;
        if self.packets_sent > 0 {
            self.stats.packet_loss = self.packets_lost as f32 / self.packets_sent as f32;
        }
    }

    /// Verifica se precisa enviar heartbeat.
    pub fn needs_heartbeat(&self) -> bool {
        self.last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL
    }

    pub fn mark_heartbeat_sent(&mut self) {
        self.last_heartbeat = Instant::now();
    }

    /// Verifica desconexão: 3 heartbeats sem resposta = 150ms.
    pub fn check_disconnect(&mut self) -> bool {
        if self.last_seen.elapsed() >= DISCONNECT_TIMEOUT {
            self.status = PeerStatus::Disconnected;
            true
        } else {
            false
        }
    }

    /// Calcula input delay adaptativo baseado em RTT.
    fn update_input_delay(&mut self) {
        let tick_ms = 1000.0 / 60.0; // ~16.67ms
        let base = (self.stats.rtt_ms / tick_ms).ceil() as u32;
        let delay = (base + 1).clamp(1, 8);
        self.stats.input_delay = delay;
    }

    pub fn is_connected(&self) -> bool { self.status == PeerStatus::Connected }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_connects_on_first_packet() {
        let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
        let mut peer = Peer::new(0, addr, 0);
        assert_eq!(peer.status, PeerStatus::Connecting);
        peer.on_packet_received();
        assert_eq!(peer.status, PeerStatus::Connected);
    }

    #[test]
    fn rtt_updates_input_delay() {
        let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
        let mut peer = Peer::new(0, addr, 0);
        // RTT 80ms → base = ceil(80/16.67) = 5, +1 = 6
        peer.record_rtt(80.0);
        assert!(peer.stats.input_delay >= 5);
    }

    #[test]
    fn disconnect_after_timeout() {
        let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
        let mut peer = Peer::new(0, addr, 0);
        peer.on_packet_received();
        peer.last_seen = Instant::now() - Duration::from_millis(200);
        assert!(peer.check_disconnect());
        assert_eq!(peer.status, PeerStatus::Disconnected);
    }
}
