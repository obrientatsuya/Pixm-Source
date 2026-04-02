/// NetSession — orquestra toda a camada de rede numa API única.
///
/// Cola: transport + protocol + clock + rollback + peer + election.
/// O jogo interage apenas com esta struct.

use std::net::SocketAddr;
use crate::transport::UdpTransport;
use crate::protocol::{NetMessage, RoomId};
use crate::clock::{ClockSync, GameClock};
use crate::peer::{Peer, PeerStatus};
use crate::election::{self, ElectionResult};
use crate::rollback::prediction::RawInput;
use crate::rollback::session::{RollbackSession, Simulation};

/// Fase da sessão de rede.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    /// Aguardando peers conectarem.
    WaitingPeers,
    /// Ping mesh + eleição + clock sync.
    Setup,
    /// Jogo em andamento.
    Playing,
    /// Sessão encerrada (desconexão ou fim de partida).
    Ended,
}

/// Configuração de uma partida.
pub struct SessionConfig {
    pub room_id:      RoomId,
    pub local_id:     u8,
    pub player_count: u8,
    pub local_addr:   SocketAddr,
}

/// API principal da camada de rede.
pub struct NetSession<S: Simulation> {
    config:     SessionConfig,
    phase:      SessionPhase,
    transport:  UdpTransport,
    peers:      Vec<Peer>,
    clock:      GameClock,
    election:   Option<ElectionResult>,
    rollback:   Option<RollbackSession<S>>,
    /// Checksum broadcast — a cada 60 ticks.
    last_checksum_tick: u64,
}

impl<S: Simulation> NetSession<S> {
    pub fn new(config: SessionConfig) -> std::io::Result<Self> {
        let transport = UdpTransport::bind(config.local_addr)?;
        let clock = GameClock::new(ClockSync::zero());

        Ok(Self {
            config,
            phase: SessionPhase::WaitingPeers,
            transport,
            peers: Vec::new(),
            clock,
            election: None,
            rollback: None,
            last_checksum_tick: 0,
        })
    }

    /// Registra um peer descoberto via DHT.
    pub fn add_peer(&mut self, id: u8, addr: SocketAddr, team: u8) {
        if !self.peers.iter().any(|p| p.id == id) {
            self.peers.push(Peer::new(id, addr, team));
        }
    }

    /// Inicia setup: envia Hello para todos os peers.
    pub fn begin_setup(&mut self) {
        self.phase = SessionPhase::Setup;
        let hello = NetMessage::Hello {
            room_id:   self.config.room_id,
            player_id: self.config.local_id,
            version:   1,
        };
        let bytes = hello.encode();
        for peer in &self.peers {
            let _ = self.transport.send(peer.addr, &bytes, true);
        }
    }

    /// Completa setup: eleição + clock sync. Inicia o jogo.
    pub fn finalize_setup(&mut self, initial_sim: S, rng_seed: u64) {
        self.election = election::elect(&self.peers, self.config.local_id);
        self.rollback = Some(RollbackSession::new(initial_sim, self.config.player_count));
        self.phase = SessionPhase::Playing;

        // Broadcast Welcome se somos o coordenador
        if let Some(ref el) = self.election {
            if el.coordinator_id == self.config.local_id {
                let welcome = NetMessage::Welcome {
                    player_id: self.config.local_id,
                    rng_seed,
                    tick_start: self.clock.current_tick() as u32 + 60, // ~1s de buffer
                };
                let bytes = welcome.encode();
                for peer in &self.peers {
                    let _ = self.transport.send(peer.addr, &bytes, true);
                }
            }
        }
    }

    /// Tick principal — chamar uma vez por frame do game loop.
    /// Retorna inputs confirmados recebidos neste frame.
    pub fn update(&mut self) -> Vec<RawInput> {
        let mut received_inputs = Vec::new();

        // 1. Drena socket
        let packets = self.transport.poll();
        for pkt in packets {
            // Atualiza peer stats
            if let Some(peer) = self.peers.iter_mut().find(|p| p.addr == pkt.from) {
                peer.on_packet_received();
            }

            // Decodifica e processa mensagem
            if let Some(msg) = NetMessage::decode(&pkt.payload) {
                match msg {
                    NetMessage::Input { tick, player_id, .. } => {
                        let input = RawInput {
                            player_id,
                            tick: tick as u64,
                            data: pkt.payload.clone(),
                            confirmed: true,
                        };
                        received_inputs.push(input.clone());

                        if let Some(ref mut rb) = self.rollback {
                            rb.receive_remote_input(input);
                        }
                    }
                    NetMessage::Heartbeat { tick: _ } => {
                        // Já atualizou last_seen acima
                    }
                    NetMessage::StateChecksum { tick, hash } => {
                        self.handle_checksum(tick, hash, pkt.from);
                    }
                    NetMessage::PingTime { t1_us } => {
                        let pong = crate::clock::handle_ping(t1_us);
                        let _ = self.transport.send(pkt.from, &pong.encode(), false);
                    }
                    NetMessage::Takeover { from_tick: _, state_hash: _, token } => {
                        if let Some(ref el) = self.election {
                            if !election::validate_takeover(el, 0, token) {
                                tracing::warn!("takeover inválido de {}", pkt.from);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // 2. Processa rollbacks pendentes
        if let Some(ref mut rb) = self.rollback {
            rb.process_rollbacks();
        }

        // 3. Re-envia reliable não confirmados
        let _ = self.transport.flush_reliable();

        // 4. Heartbeat + detecção de desconexão
        self.heartbeat_and_disconnect();

        // 5. Checksum periódico (a cada 60 ticks)
        self.periodic_checksum();

        received_inputs
    }

    /// Envia input local para todos os peers.
    pub fn send_input(&mut self, input: &RawInput) {
        let msg = NetMessage::Input {
            tick:      input.tick as u32,
            player_id: input.player_id,
            kind:      crate::protocol::InputKind::Stop, // decodificado de input.data pelo jogo
            target_x:  0,
            target_y:  0,
            target_id: 0,
        };
        let bytes = msg.encode();
        for peer in &self.peers {
            if peer.is_connected() {
                let _ = self.transport.send(peer.addr, &bytes, true);
            }
        }
    }

    /// Avança a rollback session com inputs locais.
    pub fn advance_tick(&mut self, local_inputs: Vec<RawInput>) {
        if let Some(ref mut rb) = self.rollback {
            rb.advance(local_inputs);
        }
    }

    // ─── Internos ────────────────────────────────────────────────────────

    fn heartbeat_and_disconnect(&mut self) {
        let tick = self.clock.current_tick() as u32;

        for peer in &mut self.peers {
            // Detecção de desconexão
            if peer.check_disconnect() {
                tracing::warn!("peer {} desconectou", peer.id);
                // TODO: failover se era o coordenador
            }

            // Envia heartbeat se necessário
            if peer.needs_heartbeat() && peer.is_connected() {
                let hb = NetMessage::Heartbeat { tick }.encode();
                let _ = self.transport.send(peer.addr, &hb, false);
                peer.mark_heartbeat_sent();
            }
        }
    }

    fn periodic_checksum(&mut self) {
        let tick = self.clock.current_tick();
        if tick >= self.last_checksum_tick + 60 {
            if let Some(ref rb) = self.rollback {
                let checksum = NetMessage::StateChecksum {
                    tick: tick as u32,
                    hash: rb.checksum(),
                };
                let bytes = checksum.encode();
                for peer in &self.peers {
                    if peer.is_connected() {
                        let _ = self.transport.send(peer.addr, &bytes, false);
                    }
                }
            }
            self.last_checksum_tick = tick;
        }
    }

    fn handle_checksum(&self, tick: u32, hash: u64, from: SocketAddr) {
        if let Some(ref rb) = self.rollback {
            if rb.current_tick() >= tick as u64 {
                let local_hash = rb.checksum();
                if local_hash != hash {
                    tracing::error!(
                        "DIVERGÊNCIA tick={tick} peer={from} local={local_hash:#x} remote={hash:#x}"
                    );
                    // TODO: resync via StateSnapshot
                }
            }
        }
    }

    // ─── Accessors ───────────────────────────────────────────────────────

    pub fn phase(&self) -> SessionPhase { self.phase }
    pub fn clock(&self) -> &GameClock { &self.clock }
    pub fn peers(&self) -> &[Peer] { &self.peers }
    pub fn rollback_session(&self) -> Option<&RollbackSession<S>> { self.rollback.as_ref() }
    pub fn election(&self) -> Option<&ElectionResult> { self.election.as_ref() }
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> { self.transport.local_addr() }

    pub fn connected_peer_count(&self) -> usize {
        self.peers.iter().filter(|p| p.status == PeerStatus::Connected).count()
    }
}
