# Game Loop Design — Pixm Engine

## Visão Geral

Três loops independentes rodando em threads separadas:

```
Thread Rede  (Core 0)  — recebe/envia pacotes UDP, sem bloqueio
Thread Sim   (Core 1)  — simulação determinística a 60Hz fixo
Thread Render(Core 2)  — renderização a 60Hz+, interpolada
```

Comunicação entre threads: **ring buffers lock-free** (sem mutex no hot path).

---

## Fixed Timestep

A simulação roda em ticks fixos. O render interpola entre ticks.

```
TICK_RATE = 60          // 60 simulações por segundo
TICK_DUS  = 16_667      // microssegundos por tick (1_000_000 / 60)

loop:
  now        = monotonic_clock_us()
  delta      = now - last_time
  last_time  = now
  accumulator += delta

  while accumulator >= TICK_DUS:
    simulate_tick()          // sempre com timestep FIXO
    accumulator -= TICK_DUS

  alpha = accumulator / TICK_DUS   // 0.0 .. 1.0
  render(alpha)                    // interpolação visual
```

**Por que isso importa:**
- `simulate_tick()` nunca recebe `delta_time` — sempre timestep idêntico
- Resultado: todos os peers produzem exatamente o mesmo estado dado os mesmos inputs
- Render suave sem afetar determinismo da simulação

---

## Detalhamento de `simulate_tick()`

```
tick N:
  ┌─ INPUT ──────────────────────────────────────────────────────┐
  │  1. collect_local_inputs()                                    │
  │     → lê InputCollector (preenchido pela thread de input)    │
  │     → faz coalescing: último move_ground vence               │
  │                                                              │
  │  2. receive_remote_inputs()                                  │
  │     → drena ring buffer da thread de rede                    │
  │     → separa por tick de origem (pode ser input atrasado)    │
  └──────────────────────────────────────────────────────────────┘
  ┌─ ROLLBACK ───────────────────────────────────────────────────┐
  │  3. check_late_inputs()                                      │
  │     → se chegou input de tick anterior: rollback()           │
  │     → restaura snapshot[tick_atrasado]                       │
  │     → re-simula do tick_atrasado até tick atual              │
  └──────────────────────────────────────────────────────────────┘
  ┌─ PREDIÇÃO ───────────────────────────────────────────────────┐
  │  4. predict_missing_inputs()                                 │
  │     → para peers sem input neste tick: repete último input   │
  │     → marcado como "predito" — será corrigido se errado      │
  └──────────────────────────────────────────────────────────────┘
  ┌─ SIMULAÇÃO ──────────────────────────────────────────────────┐
  │  5. apply_inputs_to_world()                                  │
  │     → InputEvent → MoveTarget, habilidade ativada, etc.      │
  │                                                              │
  │  6. run_systems()                                            │
  │     → move_target → movement → collision                     │
  │     → combat → health → death → buffs                        │
  │                                                              │
  │  7. drain_event_queue()                                      │
  │     → todos os eventos do tick são processados e descartados │
  └──────────────────────────────────────────────────────────────┘
  ┌─ NETCODE ────────────────────────────────────────────────────┐
  │  8. save_snapshot(tick_id)                                   │
  │     → snapshot circular buffer (16 frames)                   │
  │                                                              │
  │  9. send_local_inputs()                                      │
  │     → envia InputEvent para todos os peers via ring buffer   │
  │                                                              │
  │  10. checksum_broadcast() [a cada 60 ticks]                  │
  │      → hash do SimState → detecta divergência entre peers    │
  └──────────────────────────────────────────────────────────────┘
```

---

## Input Model (MOBA — eventos esparsos)

MOBA não usa polling de 60Hz. Inputs são eventos discretos.

```
InputEvent:
  type:      MoveGround | MoveAttack | Ability(slot) | Stop | AttackMove
  target_x:  Fixed (coordenada do mundo)
  target_y:  Fixed
  target_id: Option<EntityId>   (se click em entidade)
  tick:      TickId             (quando foi gerado)

Coalescing por tick:
  5 clicks de move em 16ms → apenas o último é enviado
  Q + W no mesmo tick      → ambos enviados (habilidades não coalescem)
```

**Taxa de envio real:** ~3 eventos/segundo (vs 60Hz = 60/s).
Resultado: ~95% menos pacotes de input vs polling.

---

## Thread de Render

O render **não bloqueia** e **não tem acesso direto** ao SimState.

```
Thread Render (60Hz+):
  1. Lê SimSnapshot[tick_atual] e SimSnapshot[tick_anterior]
     (cópia thread-safe via double buffer)

  2. Interpola posições com alpha:
     rendered_pos = lerp(snap_prev.pos, snap_curr.pos, alpha)

  3. Envia posições interpoladas para Godot via godot_bridge

  4. Godot atualiza nodes (Node2D.position, AnimationPlayer, etc.)
```

**Regra:** thread de render nunca escreve em SimState. Só lê snapshots.

---

## Thread de Rede

```
Thread Rede (Core 0, non-blocking):
  loop:
    recv_all_udp()          → drena socket, coloca em inbound_ring
    process_inbound()       → deserializa NetMessage, roteia:
                              Input → input_ring[peer]
                              Checksum → checksum_queue
                              Heartbeat → peer_stats[peer].last_seen
    send_pending()          → drena outbound_ring, envia UDP
    update_peer_stats()     → RTT, jitter, packet_loss por peer
    sleep_if_idle(50µs)     → evita busy-wait desnecessário
```

---

## Comunicação entre Threads

```
Thread Sim ──[outbound_ring: Vec<NetMessage>]──→ Thread Rede
Thread Rede ──[inbound_ring: Vec<NetPacket>]──→ Thread Sim
Thread Sim ──[render_double_buffer]──────────→ Thread Render
Thread Input ──[input_queue: Vec<RawInput>]──→ Thread Sim
```

Todos os rings são **lock-free (SPSC onde possível)**.
Sem mutex no hot path da simulação.

---

## Lockstep vs Rollback — Quando usar cada um

```
RTT < 16ms  (LAN/mesma cidade):  lockstep puro, input delay = 1 tick
RTT 16-66ms (mesma região):      lockstep + 1-3 ticks de delay adaptativo
RTT 66-200ms (cross-region):     rollback, predição de input
RTT > 200ms:                     rollback agressivo + aviso de latência na UI
```

O `RollbackSession` decide automaticamente com base no RTT medido de cada peer.

---

## Tick Rate — Por que 60Hz

O foco do jogo é o **duelo 1v1** como mecânica central.

```
30Hz:  tick a cada 33ms — perceptível em trocas de habilidade precisas
60Hz:  tick a cada 16ms — transparente para o jogador

Custo extra de 60Hz:
  CPU sim:      dobro (ainda trivial — sim é O(n) sobre entidades)
  Rollback:     dobro de frames para re-simular (ainda < 1ms total)
  Bandwidth:    mesmos eventos (input event-based, não 60/s)

Benefício:
  Input lag mínimo: 16ms vs 33ms — diferença de 17ms é sentida em duelo
```

60Hz é a escolha certa dado o objetivo de fluidez no duelo.

---

## Clock Sync (obrigatório para lockstep)

```
Algoritmo: Cristian's Algorithm adaptado para P2P
Referência: peer com menor RTT médio (coordenador eleito)
Precisão:   ±0.5ms (suficiente para lockstep a 60Hz = 16ms/frame)
Re-sync:    a cada 30s durante partida
            drift máximo entre re-syncs: 200ppm × 30s = 6ms < 1 frame

game_tick():
  usar game_clock.current_tick()  → nunca wall clock
  game_clock.current_tick() = (monotonic_us() + offset_us) / TICK_DUS
```
