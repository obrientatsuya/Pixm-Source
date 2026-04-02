# Roadmap — Pixm Engine

Cada fase entrega algo testável antes de avançar.
Fases concluídas têm ✅. Em progresso têm 🔧. Pendentes têm ⬜.

---

## Camada de Rede (`net/`)

### ✅ Fase 1 — Transporte UDP
`net/src/transport.rs`, `net/src/ack.rs`

- UdpSocket non-blocking, send/recv
- Header 7 bytes: sequence + ack + ack_bits + kind
- ACK seletivo com bitmask de 32 frames
- Re-envio de pacotes reliable (retry após 50ms)
- Testes: header roundtrip, send/recv local

### ✅ Fase 2 — Protocolo e Serialização
`net/src/protocol.rs`

- `NetMessage` com todas as variantes (Hello, Input, Checksum, Ping/Pong, etc.)
- Serialização/deserialização com bitcode
- `RoomId([u8; 32])` com to/from hex, random()
- `InputKind` enum para todas as ações do jogador

### ✅ Fase 3 — DHT e Matchmaking
`net/src/dht.rs`

- Bootstrap nos nós mainnet BitTorrent (router.bittorrent.com, etc.)
- `pixm://join/<room_id_hex>` — link de convite sem servidor central
- Stub Kademlia (libp2p comentado até fase de integração real)
- `announce()`, `resolve()`, `make_join_link()`, `parse_join_link()`

### ✅ Fase 4 — NAT Traversal
`net/src/nat.rs` ⚠️ implementado, não testado (requer 2 PCs em redes diferentes)

- Descoberta de IP público via STUN (RFC 5389)
- Lista de STUN servers públicos com fallback
- UDP hole punching simultâneo (PIXM_PUNCH/PIXM_PONG, 10 tentativas × 50ms)
- Parse de XOR-MAPPED-ADDRESS

### ✅ Fase 5 — Clock Sync
`net/src/clock.rs`

- Cristian's Algorithm: 16 amostras ping/pong
- Filtra outliers > 3× mediana; usa offset do min-RTT
- `current_tick()`, `needs_resync()` (30s), re-sync automático
- `GameClock` com `now_us()` e `update_sync()`

### ✅ Fase 6 — Rollback Netcode
`net/src/rollback/`

- `SnapshotBuffer`: circular de 16 frames, push/get/oldest/newest
- `InputPredictor`: repete último input confirmado; mede accuracy()
- `RollbackSession<S: Simulation>`: advance, receive_remote, rollback_to
- Trait `Simulation`: serialize/deserialize/step/checksum/rng_state

### ✅ Fase 7 — NetSession (integração rede completa)
`net/src/session.rs`, `net/src/peer.rs`, `net/src/election.rs`

- `NetSession<S>` coordena transport + rollback + clock + eleição
- `Peer` com RTT adaptativo (16 amostras), jitter EMA, input_delay dinâmico
- Eleição de coordenador: score = 0.6×rtt + 0.4×estabilidade; tie-break por ID
- Heartbeat 20Hz, desconexão em 150ms, shadow peer como backup
- Checksum broadcast a cada 60 ticks (1Hz), log de divergência

**Critérios pendentes de validação real:**
```
RTT localhost     < 1ms    (não medido)
Clock drift 5min  < 1ms    (não medido)
Rollback 5% loss  → zero divergência  (não testado multi-peer)
Failover          < 160ms  (não testado)
```

---

## Camada de Jogo (`game/`)

### ✅ Fase 8 — ECS Core e Simulação Determinística
`game/src/core/`, `game/src/sim/`

- `core/types`: Fixed (I32F32), PlayerId, Vec2Fixed (length_sq, normalize, lerp)
- `core/events`: EventBus síncrono, DamageEvent/DeathEvent/BuffEvent/AbilityEvent
- `core/ecs`: WorldExt trait sobre hecs
- `sim/components`: Position, Velocity, Health, Combat (range/damage/speed/crit),
  CrowdControl (root/stun/slow/knockup/silence), Buff, Owner, Team, AI, Projectile
- `sim/rng`: LCG determinístico com Knuth constants, snapshot/restore
- `sim/systems/movement`: move_target (CC-aware), movement, clear_arrived
- `sim/systems/combat`: cooldown, auto_attack (crit + tie-break por EntityId), health, death, cleanup
- `sim/systems/buffs`: crowd_control_system, buff_system com BuffEvent on expiry
- `sim/world`: SimWorld.run_tick() pipeline + Simulation trait + FNV-1a checksum

### ✅ Fase 8b — Input e Godot Bridge
`game/src/input/`, `game/src/bridge/`

- `input/events`: InputEvent com bitcode (Fixed como i64 bits — workaround I32F32)
- `input/collector`: coalescing por jogador (move sobrescreve, ability preserva)
- `bridge/input_bridge`: Godot Vector2 → InputEvent (única fronteira f32→Fixed)
- `bridge/renderer`: GameLoopNode GodotClass, fixed timestep 60Hz com accumulator

**52 testes passando (31 net + 21 game).**

---

### ⬜ Fase 9 — Pathfinding Determinístico
`game/src/sim/systems/pathfinding.rs`

- A* em grid inteiro (não navmesh — evita float)
- Tie-breaking determinístico (menor EntityId vence)
- `PathCache` por entidade — recalcula só quando bloqueado
- Waypoints como Vec de posições fixas; movement system os consome
- Obstáculos como bitset de células ocupadas

### ⬜ Fase 10 — Sistema de Habilidades
`game/src/sim/systems/abilities.rs`

- Consome `AbilityEvent` do EventBus
- Verifica cooldown, mana (se houver), silêncio (CC)
- Despacha efeitos: spawn projétil, aplicar CC, AOE dano, dash
- `AbilityDef` como dado puro (alcance, cooldown, efeito) — sem lógica de jogo no engine
- Suporte a: projétil linear, área instantânea, alvo único, dash de reposicionamento

### ⬜ Fase 11 — Projéteis
`game/src/sim/systems/projectiles.rs`

- `Projectile { speed, damage, source, target: Entity | Point }`
- Movimento linear determinístico (fixed-point)
- Colisão por distância quadrática (`dist_sq <= radius_sq`)
- Ao colidir: emite DamageEvent, despawna

### ⬜ Fase 12 — Minions e IA Básica
`game/src/sim/systems/ai.rs`

- Minions seguem waypoints (lane path)
- Aggro: ataca inimigo mais próximo em range; tie-break por EntityId (menor)
- Não usa pathfinding completo — só waypoints lineares de lane
- `AiTarget` component: mantém alvo entre ticks

### ⬜ Fase 13 — Cenas Godot e Loop Visual
`game/godot/scenes/`

- `main.tscn`: GameLoopNode como autoload
- `hero.tscn`: Node2D com sprite, anima por estado (idle/walk/attack)
- `GodotNodeRef` component: path para o Node2D correspondente
- `update_render(alpha)` real: itera Position + GodotNodeRef, interpola
- Câmera seguindo o herói local

### ⬜ Fase 14 — Integração P2P Completa (end-to-end)
Requer: Fases 9–13 + NAT testado (2 PCs)

- Partida 1v1: link pixm://, NAT punch, clock sync, rollback ativo
- Checksum broadcast validando determinismo em tempo real
- Desconexão e reconexão graciosa
- Seed da partida distribuída pelo coordenador via DHT

### ⬜ Fase 15 — MOBA Config (jogo sobre o engine)
`game/src/config/` ou `game/src/moba/`

- Definições de heróis como dados (JSON ou constantes Rust)
- 4 habilidades por herói (Q/W/E/R) instanciando AbilityDef
- Torres, inibidores, nexo como entidades com componentes genéricos
- Fog of War via VisionRange + bitset de células visíveis
- Gold, XP, level como componentes puros

---

## Dependências entre fases

```
Fases 1–7 (net) ──────────────────────────────→ Fase 14
Fase 8 (ECS core + sim) ──→ Fase 9 ──→ Fase 10 ──→ Fase 11 ──→ Fase 14
                        └──→ Fase 12 ──────────────────────────→ Fase 14
                        └──→ Fase 13 ──────────────────────────→ Fase 14
Fase 14 ──→ Fase 15
```

Fases 9–12 podem ser desenvolvidas em paralelo entre si.
Fase 13 (Godot) pode começar com stubs de rendering antes das fases 9–12.

---

## Critérios de MVP jogável (Fase 14 completa)

```
2 jogadores conectam via link pixm://          ✓ arquitetura pronta
Inputs chegam e são aplicados deterministicamente  ✓ sim pronta
Rollback transparente com 100ms RTT simulado   ✓ rollback pronto
Zero divergência de checksum em 10 min         ⬜ pendente validação
Reconexão em < 5s após queda                   ⬜ pendente
Loop visual rodando a 60fps estável            ⬜ pendente Fase 13
```
