/// Eleição do peer coordenador — menor RTT médio + menor jitter.
///
/// O coordenador NÃO está no caminho crítico de inputs.
/// Só arbitra checksums de divergência e coordena failover.

use crate::peer::{Peer, PeerStats};

/// Resultado da eleição.
#[derive(Debug, Clone, Copy)]
pub struct ElectionResult {
    /// ID do coordenador eleito.
    pub coordinator_id: u8,
    /// ID do shadow (backup — assume se coordenador desconecta).
    pub shadow_id:      u8,
    /// Token de validação (distribuído a todos os peers).
    pub token:          u32,
}

/// Score de elegibilidade — quanto maior, melhor candidato.
fn election_score(stats: &PeerStats) -> f32 {
    let rtt_score    = 1.0 / (stats.rtt_ms + 1.0);
    let stable_score = 1.0 / (stats.jitter_ms + 1.0);
    rtt_score * 0.6 + stable_score * 0.4
}

/// Verifica se um peer é elegível para coordenador.
fn is_eligible(peer: &Peer) -> bool {
    peer.is_connected()
        && peer.stats.jitter_ms    <= 15.0
        && peer.stats.packet_loss  <= 0.015
        && peer.stats.rtt_ms       <= 180.0
}

/// Executa eleição entre todos os peers da partida.
/// Retorna None se nenhum peer é elegível.
pub fn elect(peers: &[Peer], local_id: u8) -> Option<ElectionResult> {
    // Filtra elegíveis e ordena por score (maior = melhor)
    let mut candidates: Vec<(u8, f32)> = peers.iter()
        .filter(|p| is_eligible(p))
        .map(|p| (p.id, election_score(&p.stats)))
        .collect();

    // Peer local também é candidato
    // (stats não disponíveis para si mesmo — assume score máximo se elegível)
    candidates.push((local_id, f32::MAX));

    // Ordena por score (desc), tie-break por menor ID
    candidates.sort_by(|(id_a, score_a), (id_b, score_b)| {
        score_b.partial_cmp(score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(id_a.cmp(id_b))
    });

    if candidates.is_empty() { return None; }

    let coordinator_id = candidates[0].0;
    let shadow_id = candidates.get(1)
        .map(|(id, _)| *id)
        .unwrap_or(coordinator_id);

    // Token simples — XOR de todos os peer IDs
    let token = peers.iter()
        .map(|p| p.id as u32)
        .fold(local_id as u32, |acc, id| acc ^ (id.wrapping_mul(2654435761)));

    Some(ElectionResult {
        coordinator_id,
        shadow_id,
        token,
    })
}

/// Valida que um Takeover veio do shadow legítimo.
pub fn validate_takeover(result: &ElectionResult, sender_id: u8, token: u32) -> bool {
    sender_id == result.shadow_id && token == result.token
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn make_peer(id: u8, rtt: f32, jitter: f32) -> Peer {
        let addr: SocketAddr = format!("127.0.0.1:{}", 1000 + id as u16).parse().unwrap();
        let mut p = Peer::new(id, addr, 0);
        p.on_packet_received(); // set to Connected
        p.stats.rtt_ms = rtt;
        p.stats.jitter_ms = jitter;
        p.stats.packet_loss = 0.0;
        p
    }

    #[test]
    fn elects_best_rtt() {
        let peers = vec![
            make_peer(1, 50.0, 2.0),
            make_peer(2, 20.0, 1.0), // melhor
            make_peer(3, 80.0, 5.0),
        ];
        // local_id = 0, score = MAX → local é coordenador, shadow = peer 2
        let result = elect(&peers, 0).unwrap();
        assert_eq!(result.coordinator_id, 0);
        assert_eq!(result.shadow_id, 2);
    }

    #[test]
    fn ineligible_peers_excluded() {
        let peers = vec![
            make_peer(1, 200.0, 20.0), // RTT e jitter acima do limite
        ];
        let result = elect(&peers, 0).unwrap();
        // Peer 1 inelegível — só o local é candidato
        assert_eq!(result.coordinator_id, 0);
        assert_eq!(result.shadow_id, 0);
    }

    #[test]
    fn takeover_validation() {
        let result = ElectionResult {
            coordinator_id: 0,
            shadow_id: 2,
            token: 12345,
        };
        assert!(validate_takeover(&result, 2, 12345));
        assert!(!validate_takeover(&result, 1, 12345)); // impostor
        assert!(!validate_takeover(&result, 2, 99999)); // token errado
    }
}
