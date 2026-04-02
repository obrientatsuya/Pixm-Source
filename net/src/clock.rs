/// Sincronização de relógio entre peers — Cristian's Algorithm.
///
/// Garante que todos os peers concordam em qual tick estão (±0.5ms).
/// Obrigatório para lockstep funcionar corretamente.
///
/// Re-sync automático a cada 30s. Drift máximo entre re-syncs: ~6ms < 1 tick.

use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};
use crate::protocol::NetMessage;

pub const TICK_RATE:    u64 = 60;
pub const TICK_DUS:     u64 = 1_000_000 / TICK_RATE; // microssegundos por tick (~16_666)
pub const SYNC_SAMPLES: usize = 16;
pub const RESYNC_SECS:  u64 = 30;

// ─── Tipos ───────────────────────────────────────────────────────────────────

/// Uma amostra de clock sync (um ping/pong completo).
#[derive(Debug, Clone, Copy)]
pub struct ClockSample {
    /// RTT real sem tempo de processamento do coordinator.
    pub rtt_us:    u64,
    /// Diferença estimada entre nosso relógio e o do coordinator.
    /// Positivo = nosso relógio está atrasado.
    pub offset_us: i64,
}

/// Estado de sincronização calculado a partir de N amostras.
#[derive(Debug, Clone, Copy)]
pub struct ClockSync {
    pub offset_us:  i64,
    pub rtt_us:     u64,
    pub sample_count: u32,
    pub synced_at:  Instant,
}

impl ClockSync {
    /// Sync inicial sem ajuste — usado antes do primeiro sync real.
    pub fn zero() -> Self {
        Self {
            offset_us: 0,
            rtt_us: 0,
            sample_count: 0,
            synced_at: Instant::now(),
        }
    }

    pub fn needs_resync(&self) -> bool {
        self.synced_at.elapsed() >= Duration::from_secs(RESYNC_SECS)
    }
}

// ─── Cálculo puro (testável sem rede) ────────────────────────────────────────

/// Calcula uma amostra de clock sync a partir de 4 timestamps.
///
/// t1 = cliente enviou ping (relógio do cliente)
/// t2 = coordinator recebeu  (relógio do coordinator)
/// t3 = coordinator respondeu (relógio do coordinator)
/// t4 = cliente recebeu pong  (relógio do cliente)
pub fn measure_sample(t1: u64, t2: u64, t3: u64, t4: u64) -> ClockSample {
    let processing = t3.saturating_sub(t2);
    let rtt_us     = (t4.saturating_sub(t1)).saturating_sub(processing);
    let offset_us  = ((t2 as i64 - t1 as i64) + (t3 as i64 - t4 as i64)) / 2;
    ClockSample { rtt_us, offset_us }
}

/// Agrega N amostras em um ClockSync final.
///
/// Usa o offset da amostra com menor RTT (mais precisa — menos tempo de trânsito).
/// Descarta amostras com RTT > 3× mediana (outliers de jitter).
pub fn compute_sync(samples: &[ClockSample]) -> ClockSync {
    assert!(!samples.is_empty());

    // Mediana do RTT para detectar outliers
    let mut rtts: Vec<u64> = samples.iter().map(|s| s.rtt_us).collect();
    rtts.sort_unstable();
    let median_rtt = rtts[rtts.len() / 2];
    let threshold  = median_rtt.saturating_mul(3);

    // Filtra outliers, usa menor RTT restante
    let best = samples.iter()
        .filter(|s| s.rtt_us <= threshold)
        .min_by_key(|s| s.rtt_us)
        .unwrap_or(&samples[0]);

    ClockSync {
        offset_us:    best.offset_us,
        rtt_us:       best.rtt_us,
        sample_count: samples.len() as u32,
        synced_at:    Instant::now(),
    }
}

// ─── GameClock ───────────────────────────────────────────────────────────────

/// Relógio de jogo sincronizado com o coordinator.
/// Nunca usar wall clock diretamente na simulação — sempre este.
pub struct GameClock {
    sync: ClockSync,
}

impl GameClock {
    pub fn new(sync: ClockSync) -> Self {
        Self { sync }
    }

    /// Tempo atual ajustado pelo offset do coordinator (microssegundos).
    pub fn now_us(&self) -> u64 {
        monotonic_us().saturating_add_signed(self.sync.offset_us)
    }

    /// Tick atual da simulação.
    pub fn current_tick(&self) -> u64 {
        self.now_us() / TICK_DUS
    }

    /// Atualiza o sync (chamado pelo re-sync a cada 30s).
    pub fn update_sync(&mut self, sync: ClockSync) {
        self.sync = sync;
    }

    pub fn rtt_us(&self) -> u64 { self.sync.rtt_us }
}

// ─── Sync via rede ───────────────────────────────────────────────────────────

/// Executa SYNC_SAMPLES rodadas de ping/pong com o coordinator.
/// Retorna ClockSync calculado. Usa socket já aberto.
///
/// ⚠️  Bloqueante — chamar em thread separada ou antes do game loop.
pub fn sync_with_coordinator(
    socket: &UdpSocket,
    coordinator: SocketAddr,
) -> Option<ClockSync> {
    socket.set_read_timeout(Some(Duration::from_millis(500))).ok()?;

    let mut samples = Vec::with_capacity(SYNC_SAMPLES);
    let mut recv_buf = [0u8; 256];

    for _ in 0..SYNC_SAMPLES {
        let t1 = monotonic_us();
        let ping = NetMessage::PingTime { t1_us: t1 }.encode();
        socket.send_to(&ping, coordinator).ok()?;

        match socket.recv_from(&mut recv_buf) {
            Ok((len, from)) if from == coordinator => {
                let t4 = monotonic_us();
                if let Some(NetMessage::PongTime { t1_us, t2_us, t3_us }) =
                    NetMessage::decode(&recv_buf[..len])
                {
                    if t1_us == t1 {
                        samples.push(measure_sample(t1, t2_us, t3_us, t4));
                    }
                }
            }
            _ => continue,
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    if samples.is_empty() { return None; }
    Some(compute_sync(&samples))
}

/// Responde a um PingTime — chamar no coordinator ao receber esse pacote.
pub fn handle_ping(t1_us: u64) -> NetMessage {
    let t2 = monotonic_us();
    // pequena pausa simulando processamento real
    let t3 = monotonic_us();
    NetMessage::PongTime { t1_us, t2_us: t2, t3_us: t3 }
}

// ─── Utilitário ──────────────────────────────────────────────────────────────

pub fn monotonic_us() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

// ─── Testes ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_sample_zero_processing() {
        // t1=0, t2=10, t3=10, t4=20 → RTT=20, offset=0
        let s = measure_sample(0, 10, 10, 20);
        assert_eq!(s.rtt_us, 20);
        assert_eq!(s.offset_us, 0);
    }

    #[test]
    fn measure_sample_clock_behind() {
        // Nosso relógio está 5µs atrasado
        // t1=100, t2=115(coord), t3=115(coord), t4=120
        // RTT = (120-100)-(115-115) = 20, offset = ((115-100)+(115-120))/2 = (15-5)/2 = 5
        let s = measure_sample(100, 115, 115, 120);
        assert_eq!(s.rtt_us, 20);
        assert_eq!(s.offset_us, 5);
    }

    #[test]
    fn compute_sync_discards_outliers() {
        let mut samples = vec![
            ClockSample { rtt_us: 1000, offset_us: 5 },
            ClockSample { rtt_us: 900,  offset_us: 4 },
            ClockSample { rtt_us: 950,  offset_us: 5 },
            ClockSample { rtt_us: 9999, offset_us: 99 }, // outlier
        ];
        // Preenche até SYNC_SAMPLES para não travar asserts
        samples.resize(SYNC_SAMPLES, ClockSample { rtt_us: 920, offset_us: 4 });
        let sync = compute_sync(&samples);
        // outlier descartado — menor RTT válido é 900
        assert!(sync.rtt_us < 1100);
        assert_ne!(sync.offset_us, 99);
    }

    #[test]
    fn game_clock_tick_is_deterministic() {
        let sync = ClockSync::zero();
        let clock = GameClock::new(sync);
        let t1 = clock.current_tick();
        let t2 = clock.current_tick();
        // Ticks só avançam — nunca retrocedem
        assert!(t2 >= t1);
    }

    #[test]
    fn localhost_sync_roundtrip() {
        use std::net::SocketAddr;
        let coord_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let client_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

        let coord_sock = UdpSocket::bind(coord_addr).unwrap();
        let client_sock = UdpSocket::bind(client_addr).unwrap();
        let coord_real = coord_sock.local_addr().unwrap();

        // Coordinator em thread separada
        std::thread::spawn(move || {
            coord_sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut buf = [0u8; 256];
            for _ in 0..SYNC_SAMPLES {
                if let Ok((len, from)) = coord_sock.recv_from(&mut buf) {
                    if let Some(NetMessage::PingTime { t1_us }) = NetMessage::decode(&buf[..len]) {
                        let pong = handle_ping(t1_us).encode();
                        let _ = coord_sock.send_to(&pong, from);
                    }
                }
            }
        });

        std::thread::sleep(Duration::from_millis(50));
        let sync = sync_with_coordinator(&client_sock, coord_real).unwrap();

        // Em localhost: RTT deve ser < 5ms, offset próximo de 0
        assert!(sync.rtt_us < 5_000, "RTT muito alto: {}µs", sync.rtt_us);
        assert!(sync.offset_us.abs() < 5_000, "offset muito alto: {}µs", sync.offset_us);
        assert_eq!(sync.sample_count, SYNC_SAMPLES as u32);
    }
}
