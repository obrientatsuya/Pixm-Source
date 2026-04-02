# Roadmap — Camada de Rede (net/)

Ordem de implementação: cada fase entrega algo testável antes de avançar.

---

## Fase 1 — Transporte UDP (`net/src/transport.rs`)

**Entrega:** dois processos trocando bytes via UDP na mesma máquina.

```
[ ] UdpSocket non-blocking (tokio::net::UdpSocket)
[ ] send(peer_addr, &[u8]) → Result
[ ] recv() → Vec<(SocketAddr, Vec<u8>)>
[ ] Header de pacote: sequence(u8) + ack(u8) + ack_bits(u32) + type(u8) = 7 bytes
[ ] ACK seletivo: marcar pacote como "confirmado" via ack_bits
[ ] Re-envio de pacotes reliable não confirmados (retry após 50ms)
[ ] Teste: dois binários trocam 1000 pacotes, mede loss/latência
```

---

## Fase 2 — Protocolo (`net/src/protocol.rs`)

**Entrega:** NetMessage serializado e deserializado corretamente.

```
[ ] Enum NetMessage com todas as variantes (ver netcode.md)
[ ] Serialização com bitcode (binário compacto)
[ ] Deserialização com validação (pacote malformado → descarta)
[ ] Bitpacking de InputEvent (13 bytes conforme spec)
[ ] Teste unitário: serialize → deserialize → assertEqual para cada variante
```

---

## Fase 3 — DHT e Descoberta (`net/src/dht.rs`)

**Entrega:** dois peers se encontram via DHT sem servidor central.

```
[ ] Nó Kademlia via libp2p::kad
[ ] Bootstrap: conectar a pelo menos 1 peer conhecido (hardcoded p/ teste)
[ ] Criar sala: PUT(room_id, peer_info) no DHT
[ ] Entrar em sala: GET(room_id) → lista de peers
[ ] Aguardar N peers (N = tamanho da sala, ex: 2 p/ teste)
[ ] Retornar lista de (PeerId, SocketAddr) para fase de conexão
[ ] Teste: dois terminais diferentes, encontram-se via DHT local
```

---

## Fase 4 — NAT Traversal (`net/src/transport.rs`)

**Entrega:** dois peers em redes diferentes conectam diretamente.

```
[ ] STUN: descobrir IP público via servidor STUN público (stun.l.google.com)
[ ] Trocar (IP público, porta) entre peers via DHT
[ ] UDP hole punching simultâneo (ambos enviam ao mesmo tempo)
[ ] Fallback: TURN relay se hole punch falhar após 3s
[ ] Teste: dois PCs em redes diferentes (ex: um no celular como hotspot)
```

---

## Fase 5 — Clock Sync (`net/src/clock.rs`)

**Entrega:** todos os peers concordam em qual tick estão (±0.5ms).

```
[ ] Cristian's Algorithm: 16 amostras de ping/pong
[ ] Descarta outliers (jitter momentâneo): usa mediana
[ ] offset_us: diferença entre relógio local e coordenador
[ ] current_tick() = (monotonic_us() + offset_us) / TICK_DUS
[ ] Re-sync automático a cada 30s
[ ] Teste: mede drift entre dois peers por 5 minutos
```

---

## Fase 6 — Rollback Netcode (`net/src/rollback/`)

**Entrega:** dois peers jogando com rollback — input atrasado corrigido sem travar.

```
[ ] buffer.rs: SnapshotBuffer circular (16 frames)
    [ ] push(tick, state)
    [ ] get(tick) → &SimSnapshot
    [ ] overwrite automático do frame mais antigo

[ ] prediction.rs: predição de input de peers ausentes
    [ ] predict(peer_id) → InputEvent (repete último confirmado)
    [ ] marcar predições como "não confirmadas"

[ ] session.rs: RollbackSession
    [ ] advance(inputs) → avança 1 tick com inputs locais/preditos
    [ ] receive_remote(tick, input) → detecta divergência
    [ ] rollback(target_tick) → restaura snapshot + re-simula
    [ ] checksum por tick → detecta divergência de estado

[ ] Teste: simula 2 peers com delay artificial de 100ms
    → rollbacks são transparentes, estado final é idêntico
```

---

## Fase 7 — Integração e Mesh Completo

**Entrega:** sessão P2P funcional com 2+ peers jogando.

```
[ ] NetSession: coordena transport + dht + clock + rollback
[ ] Eleição do coordenador (menor RTT médio)
[ ] Shadow peer (backup do coordenador)
[ ] Heartbeat (20Hz) + detecção de desconexão (3 perdidos = 150ms)
[ ] Failover: shadow assume em < 160ms
[ ] Checksum broadcast (1Hz) + detecção de divergência
[ ] Teste de integração: 10 peers em mesh completo por 10 minutos
```

---

## Dependências entre fases

```
Fase 1 ──→ Fase 2 ──→ Fase 4 ──→ Fase 7
                 └──→ Fase 3 ──→ Fase 7
Fase 5 ──────────────────────────→ Fase 7
Fase 6 (independente, mock de sim) → Fase 7
```

Fase 6 (rollback) pode ser desenvolvida em paralelo com 3, 4, 5
usando um simulador simples de estado para testar.

---

## Critérios de sucesso antes de integrar com game/

```
RTT medido (localhost)    < 1ms
RTT medido (LAN)          < 5ms
Packet loss simulado 5%   → zero divergência de estado
Rollback transparente     → jogador não percebe correção
Clock sync drift          < 1ms após 5 minutos
Failover                  < 160ms
```
