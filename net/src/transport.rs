use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use crate::ack::PeerAckState;

pub const HEADER_SIZE: usize = 7;
pub const MAX_PACKET_SIZE: usize = 1200;

/// Tipo do pacote — ocupa os 4 bits mais significativos de type_flags.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketKind {
    Unreliable = 0x00,
    Reliable   = 0x10,
}

/// Header de 7 bytes prefixado em todo pacote UDP.
#[derive(Debug, Clone, Copy)]
pub struct PacketHeader {
    pub sequence:   u8,
    pub ack:        u8,
    pub ack_bits:   u32,
    pub kind:       PacketKind,
}

impl PacketHeader {
    pub fn encode(&self, buf: &mut [u8; HEADER_SIZE]) {
        buf[0] = self.sequence;
        buf[1] = self.ack;
        buf[2..6].copy_from_slice(&self.ack_bits.to_le_bytes());
        buf[6] = self.kind as u8;
    }

    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < HEADER_SIZE { return None; }
        let kind = match buf[6] & 0xF0 {
            0x00 => PacketKind::Unreliable,
            0x10 => PacketKind::Reliable,
            _    => return None,
        };
        Some(Self {
            sequence: buf[0],
            ack:      buf[1],
            ack_bits: u32::from_le_bytes(buf[2..6].try_into().ok()?),
            kind,
        })
    }
}

/// Pacote recebido e já decodificado.
#[derive(Debug)]
pub struct InboundPacket {
    pub from:   SocketAddr,
    pub header: PacketHeader,
    pub payload: Vec<u8>,
}

/// Transporte UDP não-bloqueante com ACK seletivo e reenvio reliable.
///
/// Responsabilidade única: enviar e receber bytes brutos entre peers.
/// Não conhece NetMessage nem lógica de jogo.
pub struct UdpTransport {
    socket: UdpSocket,
    peers:  HashMap<SocketAddr, PeerAckState>,
    recv_buf: Box<[u8; MAX_PACKET_SIZE]>,
}

impl UdpTransport {
    /// Liga o socket na porta fornecida.
    pub fn bind(addr: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            peers: HashMap::new(),
            recv_buf: Box::new([0u8; MAX_PACKET_SIZE]),
        })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Envia payload para peer. `reliable = true` → reenviado até ACK.
    pub fn send(&mut self, peer: SocketAddr, payload: &[u8], reliable: bool) -> std::io::Result<()> {
        let state = self.peers.entry(peer).or_insert_with(PeerAckState::new);
        let seq = state.on_send(reliable, payload);
        let (ack, ack_bits) = state.header_ack_fields();

        let kind = if reliable { PacketKind::Reliable } else { PacketKind::Unreliable };
        let header = PacketHeader { sequence: seq, ack, ack_bits, kind };

        let mut hdr_buf = [0u8; HEADER_SIZE];
        header.encode(&mut hdr_buf);

        let mut packet = Vec::with_capacity(HEADER_SIZE + payload.len());
        packet.extend_from_slice(&hdr_buf);
        packet.extend_from_slice(payload);

        self.socket.send_to(&packet, peer)?;
        Ok(())
    }

    /// Drena todos os pacotes disponíveis no socket (não-bloqueante).
    /// Também processa ACKs e remove pendentes confirmados.
    pub fn poll(&mut self) -> Vec<InboundPacket> {
        let mut inbound = Vec::new();

        loop {
            match self.socket.recv_from(self.recv_buf.as_mut()) {
                Ok((len, from)) => {
                    let buf = &self.recv_buf[..len];
                    let Some(header) = PacketHeader::decode(buf) else { continue };
                    let payload = buf[HEADER_SIZE..].to_vec();

                    let state = self.peers.entry(from).or_insert_with(PeerAckState::new);
                    state.on_recv(header.sequence);
                    state.process_remote_ack(header.ack, header.ack_bits);

                    inbound.push(InboundPacket { from, header, payload });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }

        inbound
    }

    /// Reenvia pacotes reliable que não foram confirmados após 50ms.
    /// Chamar uma vez por iteração do loop de rede.
    pub fn flush_reliable(&mut self) -> std::io::Result<()> {
        let peers: Vec<SocketAddr> = self.peers.keys().copied().collect();
        for peer in peers {
            let expired = self.peers.get_mut(&peer).unwrap().take_expired();
            for (seq, data) in expired {
                let (ack, ack_bits) = self.peers[&peer].header_ack_fields();
                let header = PacketHeader {
                    sequence: seq, ack, ack_bits,
                    kind: PacketKind::Reliable,
                };
                let mut hdr_buf = [0u8; HEADER_SIZE];
                header.encode(&mut hdr_buf);

                let mut packet = Vec::with_capacity(HEADER_SIZE + data.len());
                packet.extend_from_slice(&hdr_buf);
                packet.extend_from_slice(&data);
                self.socket.send_to(&packet, peer)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let h = PacketHeader {
            sequence: 42, ack: 7,
            ack_bits: 0b1010_1010,
            kind: PacketKind::Reliable,
        };
        let mut buf = [0u8; HEADER_SIZE];
        h.encode(&mut buf);
        let decoded = PacketHeader::decode(&buf).unwrap();
        assert_eq!(decoded.sequence, 42);
        assert_eq!(decoded.ack, 7);
        assert_eq!(decoded.ack_bits, 0b1010_1010);
        assert_eq!(decoded.kind, PacketKind::Reliable);
    }

    #[test]
    fn send_recv_local() {
        let addr_a: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let addr_b: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut a = UdpTransport::bind(addr_a).unwrap();
        let mut b = UdpTransport::bind(addr_b).unwrap();
        let b_addr = b.local_addr().unwrap();

        a.send(b_addr, b"hello", false).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));

        let packets = b.poll();
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].payload, b"hello");
    }
}
