use std::net::SocketAddr;
use crate::protocol::RoomId;

/// Bootstrap nodes do BitTorrent DHT mainnet.
/// Rede pública, sem hospedagem necessária.
pub const BOOTSTRAP_NODES: &[&str] = &[
    "router.bittorrent.com:6881",
    "router.utorrent.com:6881",
    "dht.transmissionbt.com:6881",
];

/// Anúncio de sala no DHT.
#[derive(Debug, Clone)]
pub struct RoomAnnouncement {
    pub room_id:  RoomId,
    pub addr:     SocketAddr,
    pub player_id: u8,
}

/// Handle para operações DHT.
/// Implementação real via libp2p::kad — stub por enquanto.
pub struct DhtNode {
    local_addr: SocketAddr,
    // TODO fase 3: libp2p Swarm com Kademlia behaviour
}

impl DhtNode {
    /// Inicializa o nó DHT e conecta ao bootstrap.
    pub async fn start(local_addr: SocketAddr) -> std::io::Result<Self> {
        // TODO: inicializar libp2p Swarm
        // 1. gerar keypair Ed25519
        // 2. criar Kademlia behaviour com BOOTSTRAP_NODES
        // 3. spawnar task do swarm
        tracing::info!("DHT iniciando em {local_addr}");
        Ok(Self { local_addr })
    }

    /// Anuncia sala no DHT (PUT).
    /// Host chama isso ao criar partida.
    pub async fn announce(&self, announcement: RoomAnnouncement) -> std::io::Result<()> {
        // TODO: libp2p kad.put_record(room_id → addr serializado)
        tracing::info!(
            "DHT announce: room={} addr={}",
            announcement.room_id.to_hex(),
            announcement.addr
        );
        Ok(())
    }

    /// Resolve sala no DHT (GET).
    /// Guest chama isso ao clicar no link pixm://join/<room_id>.
    pub async fn resolve(&self, room_id: RoomId) -> Option<Vec<SocketAddr>> {
        // TODO: libp2p kad.get_record(room_id) → Vec<SocketAddr>
        tracing::info!("DHT resolve: room={}", room_id.to_hex());
        None
    }

    /// Remove anúncio da sala (ao encerrar partida ou ao desistir do host).
    pub async fn revoke(&self, room_id: RoomId) {
        // TODO: libp2p kad.remove_record(room_id)
        tracing::info!("DHT revoke: room={}", room_id.to_hex());
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

/// Gera o link `pixm://join/<room_id>` para compartilhar.
pub fn make_join_link(room_id: &RoomId) -> String {
    format!("pixm://join/{}", room_id.to_hex())
}

/// Extrai RoomId de um link `pixm://join/<hex>`.
pub fn parse_join_link(link: &str) -> Option<RoomId> {
    let hex = link
        .strip_prefix("pixm://join/")?
        .trim();
    RoomId::from_hex(hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_roundtrip() {
        let id = RoomId([0xABu8; 32]);
        let link = make_join_link(&id);
        assert!(link.starts_with("pixm://join/"));
        let parsed = parse_join_link(&link).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn invalid_link_returns_none() {
        assert!(parse_join_link("https://example.com").is_none());
        assert!(parse_join_link("pixm://join/zzz").is_none());
        assert!(parse_join_link("pixm://join/tooshort").is_none());
    }
}
