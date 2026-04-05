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

- `core/types`: Fixed (I32F32), PlayerId, Vec2Fixed (length_sq, normalize, lerp, dot)
- `core/events`: EventBus síncrono, DamageEvent/DeathEvent/BuffEvent/AbilityEvent
- `core/ecs`: WorldExt trait sobre hecs
- `sim/components`: Position, Velocity, Health, Combat (range/damage/speed/crit),
  CrowdControl (root/stun/slow/knockup/silence), Buff, Owner, Team, AI, Projectile,
  AccelProfile (accel/decel_zone/momentum), WallContact
- `sim/rng`: LCG determinístico com Knuth constants, snapshot/restore
- `sim/world`: SimWorld.run_tick() pipeline + Simulation trait + FNV-1a checksum

### ✅ Fase 8b — Input e Godot Bridge
`game/src/input/`, `game/src/bridge/`

- `input/events`: InputEvent com bitcode (Fixed como i64 bits)
- `input/collector`: coalescing por jogador (move sobrescreve, ability preserva)
- `bridge/input_bridge`: Godot Vector2 → InputEvent (única fronteira f32→Fixed)
- `bridge/renderer`: GameLoopNode GodotClass, fixed timestep 60Hz com accumulator,
  interpolação de render (PrevPosition + lerp alpha), `get_hero_hp/speed`, `get_entity_hp/pos`

### ✅ Fase 9 — Pathfinding Determinístico
`game/src/sim/pathfinding/`

- A* 8-direções em grid inteiro (bitset 128×128), custo cardinal=10/diagonal=14
- Tie-breaking determinístico por coordenada (sem float)
- String-pulling via LOS greedy: reduz N centros de célula a poucos pontos de virada
- Linha de visão livre → caminho direto sem A*
- Destino bloqueado → path vazio (entidade para)
- Recomputa automaticamente se herói perde LOS ao waypoint atual (wall-slide desvia rota)
- `Path` component com `current_wp()`, `advance()`, `exhausted()`

### ✅ Fase 10 — Sistema de Habilidades
`game/src/sim/systems/abilities.rs`

- `PendingAbility` component → ability_system despacha efeitos no tick
- Verifica cooldown e silêncio (CC)
- Efeitos: dano direto, spawn projétil, aplicar CC, AOE
- `AbilitySlots` + `AbilityCooldowns` por entidade
- ability_cooldown_system decrementa a cada tick

### ✅ Fase 11 — Projéteis
`game/src/sim/systems/projectiles.rs`

- `Projectile { speed, damage, source_entity, kind: Linear | Homing }`
- Movimento linear determinístico (fixed-point)
- Homing: rastreia entidade-alvo via posição atual
- Colisão por distância quadrática (`dist_sq <= radius_sq`)
- Ao colidir: emite DamageEvent, despawna

### ✅ Fase 12 — Minions e IA Básica
`game/src/sim/systems/ai.rs`

- Minions seguem waypoints de lane (LanePaths component)
- Aggro: ataca inimigo mais próximo em range; tie-break por EntityId (menor)
- Para de avançar na lane enquanto tem alvo em combate
- `AiTarget` component: mantém alvo entre ticks
- Line-of-sight check antes de atacar

### ✅ Fase 13 — Cenas Godot e Loop Visual
`game/godot/scenes/`

- `TestArena.gd`: herói, 3 bonecos inimigos, 3 minions aliados, parede de obstáculo
- Interpolação de render (PrevPosition + alpha) desacoplada do tick de sim
- Câmera com lock/unlock (Y), edge scroll livre e zoom limitado (1×–2.5×)
- HP bars: azul=aliado / vermelho=inimigo, segmentos por 25/50/75%, barra de mana amarela
- MinionHPBar: barra simples 28×3px
- HUD: FPS + ms (canto superior direito), painel de stats com HP/speed/pos (canto inferior esquerdo)
- **Movimento Apex-style**: `AccelProfile` com aceleração suave, decel_zone, momentum em virada
- `coast_system`: herói desacelera suavemente após S (friction 0.94/tick)
- Wall-slide automático: tenta X, tenta Y, para se totalmente bloqueado
- `WallContact` component: detecta vizinhos bloqueados (base para wall-run/wall-jump futuros)

**44 testes passando (31 net + 13 game).**

---

### ⬜ Fase 14 — Integração P2P Completa (end-to-end)
Requer: Fases 1–13 + NAT testado (2 PCs)

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
Fases 1–7 (net) ──────────────────────────────────→ Fase 14
Fases 8–13 (game) ────────────────────────────────→ Fase 14
Fase 14 ──→ Fase 15
```

---

## Critérios de MVP jogável (Fase 14 completa)

```
2 jogadores conectam via link pixm://              ✓ arquitetura pronta
Inputs chegam e são aplicados deterministicamente  ✓ sim pronta
Rollback transparente com 100ms RTT simulado       ✓ rollback pronto
Loop visual 60fps estável com interpolação         ✓ pronto (Fase 13)
Zero divergência de checksum em 10 min             ⬜ pendente validação multi-peer
Reconexão em < 5s após queda                       ⬜ pendente
NAT traversal em redes reais                       ⬜ pendente (requer 2 PCs)
```
