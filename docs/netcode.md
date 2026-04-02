# Netcode Spec — Pixm Engine

## Stack de Rede

```
Camada          Tecnologia              Propósito
────────────────────────────────────────────────────────
Descoberta      DHT Kademlia (libp2p)   peer discovery via BitTorrent mainnet
NAT Traversal   ICE/STUN → hole punch   conexão direta entre peers
Transporte      UDP raw (non-blocking)  gameplay — sem overhead de protocolo
Serialização    bitcode                 binário compacto, zero-copy
Confiabilidade  seletiva (ACK bits)     inputs: reliable | posições: unreliable
```

---

## Matchmaking — Solução Temporária

**Sem servidor próprio.** Bootstrap via DHT do BitTorrent mainnet (nodes públicos).

```
Bootstrap nodes (hardcoded no cliente):
  router.bittorrent.com:6881
  router.utorrent.com:6881
  dht.transmissionbt.com:6881
```

**Fluxo atual:**
```
Host:
  1. Gera room_id aleatório (32 bytes)
  2. Anuncia no DHT: PUT(room_id, ip:porta)
  3. Jogo exibe link: "pixm://join/<room_id_hex>"
  4. Jogador compartilha o link (Discord, WhatsApp, etc.)

Guest:
  1. Clica no link → jogo abre via protocol handler do OS
  2. DHT GET(room_id) → ip:porta do host
  3. Conecta via UDP direto → partida começa
```

**Custo:** zero. Sem servidores próprios. Dados do jogo nunca passam pelo DHT — só ip:porta da sala.

**Futuro (v2 — Discord Bot):**
- Bot monitora canal de matchmaking
- Jogadores fazem `/queue` → bot faz o par
- Bot envia o link `pixm://join/<room_id>` via DM para ambos
- Fluxo de conexão idêntico — só o "apresentador" muda

---

## Fase de Pré-Partida (DHT)

Sem servidor central. Descoberta via DHT Kademlia.

```
T+0.0s   DHT lookup pelos outros 9 peers (paralelo)
T+0.8s   STUN: todos descobrem IP público simultâneamente
T+1.5s   UDP hole punching: todos os pares simultâneamente
           (simultaneous open — crítico para NAT simétrico)
T+2.0s   Ping mesh: 45 pares medidos, RTTs coletados
T+2.5s   Eleição do coordenador (menor RTT médio + menor jitter)
T+2.8s   Clock sync inicial (Cristian's Algorithm)
T+3.0s   Coordenador distribui: rng_seed, frame_id_start
T+3.5s   Shadow peer confirmado (backup do coordenador)
T+5.0s   Countdown → jogo começa
```

### Eleição do Coordenador

O coordenador **não está no caminho crítico** de inputs. Só arbitra checksums e failover.

```rust
fn election_score(stats: &PeerStats) -> f32 {
    let rtt_score    = 1.0 / (stats.avg_rtt_ms + 1.0);
    let stable_score = 1.0 / (stats.jitter_ms  + 1.0);
    rtt_score * 0.6 + stable_score * 0.4
}

// Desqualificado se:
//   jitter > 15ms  |  packet_loss > 1.5%  |  rtt_max > 180ms
```

---

## Topologia durante o Jogo

**Full mesh direto** — cada peer tem 9 conexões UDP abertas.

```
P1 ←──────────────────→ P2
P1 ←──────────────────→ P3
...
P1 ←──────────────────→ P10
(45 conexões bidirecionais totais)

Inputs vão DIRETO de peer a peer — RTT físico mínimo, sem intermediário.
Coordenador recebe cópias apenas para checksum/arbitragem.
```

---

## Protocolo de Pacotes

### Header (7 bytes)

```
sequence:   u8   — número de sequência (wrap-around em 256)
ack:        u8   — último sequence recebido do peer
ack_bits:   u32  — bitmask dos últimos 32 pacotes (ACK seletivo)
type_flags: u8   — [4 bits tipo] [4 bits flags]
```

### Tipos de Mensagem

```rust
pub enum NetMessage {
    // Gameplay — reliable (re-enviado até ACK)
    InputEvent {
        tick:      u32,
        player:    u8,
        kind:      InputKind,   // MoveGround, Ability(u8), etc.
        target_x:  i32,         // fixed-point raw
        target_y:  i32,
        target_id: u16,         // EntityId comprimido (0 = sem alvo)
    },                          // 13 bytes

    // Verificação — unreliable
    StateChecksum {
        tick:  u32,
        hash:  u64,
    },                          // 12 bytes

    // Keepalive — unreliable, 20Hz
    Heartbeat {
        tick: u32,
    },                          // 4 bytes

    // Clock sync
    PingTime  { t1: u64 },
    PongTime  { t1: u64, t2: u64, t3: u64 },

    // Failover
    Takeover  { from_tick: u32, state_hash: u64 },
}
```

### Confiabilidade Seletiva

```
Mensagem          Confiabilidade    Motivo
──────────────────────────────────────────────────────
InputEvent        reliable ACK      deve chegar — irreversível
AbilityCast       reliable ACK      crítico — erro = estado diferente
StateChecksum     unreliable        se perder, o próximo checksum detecta
Heartbeat         unreliable        só pra detectar desconexão
PingTime          unreliable        re-enviado pelo próprio algoritmo
```

---

## Rollback Netcode

### Quando rollback acontece

```
Tick 100: peer B não mandou input ainda
  → predizimos: B repete último input (dead reckoning)
  → simulamos tick 100 com predição

Tick 101: input de B para tick 100 chega (atrasado)
  → input real ≠ predição
  → rollback para snapshot[100]
  → re-simula tick 100 com input real de B
  → re-simula tick 101 com inputs atuais
  → continuamos do tick 102
```

### Snapshot Buffer

```rust
pub const ROLLBACK_FRAMES: usize = 16;  // ~267ms a 60Hz

pub struct SnapshotBuffer {
    frames: [Option<SimSnapshot>; ROLLBACK_FRAMES],
    // indexado por tick % ROLLBACK_FRAMES
}

// SimSnapshot contém:
//   - estado completo do ECS (entidades + componentes)
//   - estado do RNG
//   - TickId
// Tamanho estimado: ~20-50KB por frame (200 entidades)
// Buffer total: ~800KB — aceitável
```

### Predição de Input

```rust
fn predict_input(peer: PlayerId, world: &World) -> InputEvent {
    // Estratégia: repete último input confirmado do peer
    // Para herói em movimento: continua na mesma direção
    // Para herói parado: input vazio (stop)
    last_confirmed_input[peer].unwrap_or(InputEvent::stop(peer))
}
```

---

## Detecção de Divergência

```
A cada 60 ticks (1 segundo):
  hash = fnv1a(&sim_state)    // hash rápido, não criptográfico
  broadcast StateChecksum { tick, hash }

Se algum peer reportar hash diferente:
  1. Busca binária nos últimos checksums → acha o tick de divergência
  2. Peer majoritário vence
  3. Peer divergente: recebe StateSnapshot completo e faz resync
  4. Log para debug (divergência = bug na simulação determinística)
```

---

## Sincronização de Relógio (Cristian's Algorithm)

Necessário para lockstep — todos precisam saber em que tick estão.

```
16 amostras de ping/pong com o coordenador:
  rtt    = (t4 - t1) - (t3 - t2)        // RTT sem tempo de processamento
  offset = ((t2 - t1) + (t3 - t4)) / 2  // diferença de relógio

Usa mediana das 16 amostras (descarta outliers de jitter)
Precisão: ±0.5ms — suficiente para 60Hz (16ms/tick)

Re-sync a cada 30s durante partida
Drift máximo entre re-syncs: 6ms < 1 tick
```

---

## Failover do Coordenador

```
T+0ms    Coordenador para de responder
T+150ms  3 heartbeats sem resposta — todos detectam simultaneamente
T+151ms  Shadow broadcast: Takeover { from_tick, state_hash }
T+155ms  Peers validam token pré-distribuído na eleição
T+160ms  Shadow assume → jogo congela por ~10ms (imperceptível)
T+161ms  Nova eleição de shadow em background
```

Shadow já tem estado completo (sincronizado a cada tick via checksum).
Sem re-sync necessário — simulação determinística garante estado correto.

---

## Bandwidth Real

```
Por peer, por segundo (partida em andamento):

  InputEvents:    ~3 eventos × 13 bytes × 9 peers  =   351 bytes/s enviados
  Heartbeats:     20 Hz × 4 bytes × 9 peers         =   720 bytes/s
  Checksums:      1 Hz × 12 bytes × 9 peers          =   108 bytes/s

  Total enviado:  ~1.2 KB/s por peer
  Total recebido: ~1.2 KB/s × 9 peers = ~10.8 KB/s

Trivial. Foco de otimização é latência, não bandwidth.
```

---

## NAT Traversal — Ordem de Tentativas

```
1. Mesmo IP local (LAN)          → direto, < 1ms
2. IPv6 sem NAT                  → direto, ~2ms
3. UDP hole punching simultâneo  → direto, ~5ms
4. ICE/STUN (NAT moderado)       → direto, ~10ms
5. TURN relay                    → último recurso, +20-50ms
```

**Simultaneous open é crítico:** os dois peers enviam ao mesmo tempo.
Usar timestamp do clock sync para garantir janela de ~50ms simultânea.

---

## Métricas e Ações Adaptativas

```
Métrica                Alvo       Crítico    Ação
─────────────────────────────────────────────────────────────
RTT (mesma região)     < 15ms     > 40ms     aviso na UI
Latência percebida     ≈ 0ms*     > 50ms     aumenta rollback frames
Jitter                 < 3ms      > 10ms     aumenta input delay
Packet loss            < 3%       > 8%       re-envio reliable
Rollbacks/s            < 2        > 10       aumenta input delay
Clock drift            < 1ms      > 5ms      re-sync imediato
Input delay frames     1-3        > 5        aviso de conexão ruim

* ≈ 0ms percebido via predição local — input delay real = 1 tick (16ms)
```
