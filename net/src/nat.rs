/// NAT Traversal — STUN + UDP Hole Punching
///
/// ⚠️  NÃO TESTADO — requer dois PCs em redes diferentes para validar.
///
/// Ordem de tentativas:
///   1. Mesmo IP local (LAN)           → direto
///   2. UDP hole punching simultâneo   → direto atravessando NAT
///   3. STUN/ICE                       → NAT moderado
///   4. TURN relay                     → último recurso (não implementado)

use std::net::{SocketAddr, UdpSocket, IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};

const STUN_MAGIC_COOKIE: u32 = 0x2112_A442;
const STUN_TIMEOUT:      Duration = Duration::from_secs(3);
const PUNCH_ATTEMPTS:    usize = 10;
const PUNCH_INTERVAL:    Duration = Duration::from_millis(50);

/// Servidores STUN públicos para descobrir IP:porta externos.
pub const STUN_SERVERS: &[&str] = &[
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun.cloudflare.com:3478",
];

/// IP e porta visíveis externamente (após NAT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicAddr {
    pub ip:   Ipv4Addr,
    pub port: u16,
}

impl From<PublicAddr> for SocketAddr {
    fn from(a: PublicAddr) -> Self {
        SocketAddr::new(IpAddr::V4(a.ip), a.port)
    }
}

// ─── STUN ────────────────────────────────────────────────────────────────────

/// Constrói um STUN Binding Request (RFC 5389).
fn build_stun_request(transaction_id: &[u8; 12]) -> [u8; 20] {
    let mut msg = [0u8; 20];
    msg[0..2].copy_from_slice(&0x0001u16.to_be_bytes()); // Binding Request
    msg[2..4].copy_from_slice(&0x0000u16.to_be_bytes()); // length = 0
    msg[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
    msg[8..20].copy_from_slice(transaction_id);
    msg
}

/// Parseia XOR-MAPPED-ADDRESS de uma resposta STUN.
fn parse_stun_response(buf: &[u8], transaction_id: &[u8; 12]) -> Option<PublicAddr> {
    if buf.len() < 20 { return None; }

    let msg_type = u16::from_be_bytes(buf[0..2].try_into().ok()?);
    if msg_type != 0x0101 { return None; } // não é Binding Response

    let cookie = u32::from_be_bytes(buf[4..8].try_into().ok()?);
    if cookie != STUN_MAGIC_COOKIE { return None; }

    if &buf[8..20] != transaction_id { return None; }

    let msg_len = u16::from_be_bytes(buf[2..4].try_into().ok()?) as usize;
    if buf.len() < 20 + msg_len { return None; }

    // Itera atributos
    let mut pos = 20usize;
    while pos + 4 <= 20 + msg_len {
        let attr_type = u16::from_be_bytes(buf[pos..pos+2].try_into().ok()?);
        let attr_len  = u16::from_be_bytes(buf[pos+2..pos+4].try_into().ok()?) as usize;
        pos += 4;

        if attr_type == 0x0020 && attr_len >= 8 {
            // XOR-MAPPED-ADDRESS
            let family = buf[pos + 1];
            if family != 0x01 { break; } // só IPv4

            let port_xor = u16::from_be_bytes(buf[pos+2..pos+4].try_into().ok()?);
            let port = port_xor ^ (STUN_MAGIC_COOKIE >> 16) as u16;

            let ip_xor = u32::from_be_bytes(buf[pos+4..pos+8].try_into().ok()?);
            let ip_raw = ip_xor ^ STUN_MAGIC_COOKIE;
            let ip = Ipv4Addr::from(ip_raw);

            return Some(PublicAddr { ip, port });
        }

        pos += (attr_len + 3) & !3; // alinha a 4 bytes
    }
    None
}

/// Descobre IP e porta públicos via STUN.
/// Tenta cada servidor da lista até um responder.
pub fn discover_public_addr(local_socket: &UdpSocket) -> Option<PublicAddr> {
    local_socket.set_read_timeout(Some(STUN_TIMEOUT)).ok()?;

    let transaction_id: [u8; 12] = {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut id = [0u8; 12];
        id[..8].copy_from_slice(&t.to_le_bytes()[..8]);
        id
    };

    let request = build_stun_request(&transaction_id);
    let mut buf = [0u8; 512];

    for server in STUN_SERVERS {
        let Ok(server_addr) = server.parse::<SocketAddr>() else { continue };

        if local_socket.send_to(&request, server_addr).is_err() { continue }

        match local_socket.recv_from(&mut buf) {
            Ok((len, _)) => {
                if let Some(addr) = parse_stun_response(&buf[..len], &transaction_id) {
                    tracing::info!("STUN: IP público = {}:{}", addr.ip, addr.port);
                    return Some(addr);
                }
            }
            Err(_) => continue,
        }
    }

    tracing::warn!("STUN: nenhum servidor respondeu");
    None
}

// ─── Hole Punching ───────────────────────────────────────────────────────────

/// Resultado da tentativa de conexão direta.
#[derive(Debug)]
pub enum PunchResult {
    /// Conexão direta estabelecida.
    Connected(SocketAddr),
    /// Falhou — tentar TURN relay.
    Failed,
}

/// Tenta estabelecer conexão direta via UDP hole punching simultâneo.
///
/// Ambos os peers devem chamar isso ao mesmo tempo (coordenar via DHT + clock sync).
/// `peer_public` = IP:porta público do peer remoto (obtido via STUN dele, trocado pelo DHT).
pub fn punch(local_socket: &UdpSocket, peer_public: SocketAddr) -> PunchResult {
    local_socket.set_nonblocking(true)
        .unwrap_or_else(|e| tracing::warn!("set_nonblocking: {e}"));

    let punch_payload = b"PIXM_PUNCH";
    let pong_payload  = b"PIXM_PONG";
    let mut recv_buf  = [0u8; 64];

    let deadline = Instant::now() + Duration::from_secs(5);

    tracing::info!("hole punch → {peer_public}");

    for attempt in 0..PUNCH_ATTEMPTS {
        // Envia punch
        let _ = local_socket.send_to(punch_payload, peer_public);

        // Tenta receber resposta (não-bloqueante)
        match local_socket.recv_from(&mut recv_buf) {
            Ok((len, from)) if from == peer_public => {
                let payload = &recv_buf[..len];
                if payload == punch_payload {
                    // Recebeu punch do peer — responde com pong
                    let _ = local_socket.send_to(pong_payload, peer_public);
                    tracing::info!("hole punch ok (attempt {attempt}): {peer_public}");
                    return PunchResult::Connected(peer_public);
                }
                if payload == pong_payload {
                    tracing::info!("hole punch ok via pong (attempt {attempt}): {peer_public}");
                    return PunchResult::Connected(peer_public);
                }
            }
            _ => {}
        }

        if Instant::now() >= deadline { break; }
        std::thread::sleep(PUNCH_INTERVAL);
    }

    tracing::warn!("hole punch falhou para {peer_public}");
    PunchResult::Failed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stun_request_format() {
        let id = [1u8; 12];
        let req = build_stun_request(&id);
        assert_eq!(&req[0..2], &0x0001u16.to_be_bytes());
        assert_eq!(&req[4..8], &STUN_MAGIC_COOKIE.to_be_bytes());
        assert_eq!(&req[8..20], &id);
    }

    #[test]
    fn stun_response_wrong_type_returns_none() {
        // Não é Binding Response (0x0101)
        let mut buf = [0u8; 20];
        buf[0..2].copy_from_slice(&0x0001u16.to_be_bytes());
        buf[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        let id = [0u8; 12];
        assert!(parse_stun_response(&buf, &id).is_none());
    }
}
