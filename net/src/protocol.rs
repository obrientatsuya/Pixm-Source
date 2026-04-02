use bitcode::{Encode, Decode};

/// ID de sala — 32 bytes gerados aleatoriamente pelo host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Encode, Decode)]
pub struct RoomId(pub [u8; 32]);

impl RoomId {
    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        for b in bytes.iter_mut() {
            // TODO: trocar por gerador criptográfico (fase 3)
            *b = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() & 0xFF) as u8;
        }
        Self(bytes)
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        if s.len() != 64 { return None; }
        let mut bytes = [0u8; 32];
        for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
            let hi = hex_nibble(chunk[0])?;
            let lo = hex_nibble(chunk[1])?;
            bytes[i] = (hi << 4) | lo;
        }
        Some(Self(bytes))
    }
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Tipo de input — espelho do que o jogador pode fazer num MOBA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum InputKind {
    MoveGround,          // right-click no chão
    MoveAttack,          // right-click em entidade
    Stop,
    AttackMove,
    Ability(u8),         // 0=Q 1=W 2=E 3=R 4=D 5=F (summoners)
    Item(u8),            // slot 0-5
}

/// Todas as mensagens que trafegam entre peers durante uma partida.
#[derive(Debug, Clone, Encode, Decode)]
pub enum NetMessage {
    // ── Handshake ─────────────────────────────────────────
    /// Primeiro pacote ao conectar. Identifica o peer e a sala.
    Hello {
        room_id:   RoomId,
        player_id: u8,
        version:   u32,        // protocolo — rejeita se diferente
    },

    /// Confirmação de handshake. Distribui seed do RNG da partida.
    Welcome {
        player_id:   u8,       // ID atribuído pelo coordenador
        rng_seed:    u64,
        tick_start:  u32,
    },

    // ── Gameplay — reliable ────────────────────────────────
    /// Input de um jogador para um tick específico.
    Input {
        tick:      u32,
        player_id: u8,
        kind:      InputKind,
        target_x:  i32,        // fixed-point raw (I32F32 bits)
        target_y:  i32,
        target_id: u16,        // EntityId comprimido (0 = sem alvo)
    },

    // ── Sincronização ──────────────────────────────────────
    /// Hash do estado a cada 60 ticks — detecta divergência.
    StateChecksum {
        tick: u32,
        hash: u64,
    },

    /// Snapshot completo do estado — usado em resync após divergência.
    StateSnapshot {
        tick: u32,
        data: Vec<u8>,         // SimState serializado via bitcode
    },

    // ── Clock sync ─────────────────────────────────────────
    PingTime { t1_us: u64 },
    PongTime { t1_us: u64, t2_us: u64, t3_us: u64 },

    // ── Keepalive / controle ───────────────────────────────
    /// Heartbeat a 20Hz — detecção de desconexão.
    Heartbeat { tick: u32 },

    /// Coordenador anuncia eleição resolvida.
    CoordinatorElected {
        peer_id:   u8,
        shadow_id: u8,
        token:     u32,
    },

    /// Shadow assume após falha do coordenador.
    Takeover {
        from_tick:  u32,
        state_hash: u64,
        token:      u32,
    },
}

impl NetMessage {
    /// Serializa para bytes usando bitcode.
    pub fn encode(&self) -> Vec<u8> {
        bitcode::encode(self)
    }

    /// Desserializa. Retorna None se malformado.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        bitcode::decode(bytes).ok()
    }

    /// True = deve ser enviado como reliable no UdpTransport.
    pub fn is_reliable(&self) -> bool {
        matches!(self,
            Self::Hello { .. }     |
            Self::Welcome { .. }   |
            Self::Input { .. }     |
            Self::StateSnapshot { .. } |
            Self::CoordinatorElected { .. } |
            Self::Takeover { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_id_hex_roundtrip() {
        let id = RoomId([42u8; 32]);
        let hex = id.to_hex();
        assert_eq!(hex.len(), 64);
        let decoded = RoomId::from_hex(&hex).unwrap();
        assert_eq!(id, decoded);
    }

    #[test]
    fn message_encode_decode() {
        let msgs: &[NetMessage] = &[
            NetMessage::Heartbeat { tick: 1234 },
            NetMessage::Input {
                tick: 99, player_id: 3,
                kind: InputKind::Ability(0),
                target_x: 1000, target_y: -500, target_id: 7,
            },
            NetMessage::StateChecksum { tick: 60, hash: 0xDEADBEEF },
            NetMessage::PingTime { t1_us: 987654321 },
        ];

        for msg in msgs {
            let bytes = msg.encode();
            let decoded = NetMessage::decode(&bytes)
                .expect("decode falhou");
            // verifica que re-codifica igual (consistência)
            assert_eq!(bytes, decoded.encode());
        }
    }

    #[test]
    fn reliable_classification() {
        assert!(NetMessage::Hello {
            room_id: RoomId([0u8; 32]), player_id: 0, version: 1
        }.is_reliable());
        assert!(!NetMessage::Heartbeat { tick: 0 }.is_reliable());
        assert!(!NetMessage::StateChecksum { tick: 0, hash: 0 }.is_reliable());
    }
}
