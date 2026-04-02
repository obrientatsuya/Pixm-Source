# Arquitetura do Sistema — Pixm Engine

## Visão Geral

Engine de jogo P2P genérico, projetado para baixa latência e alta performance.
O jogo (MOBA 5v5) é uma configuração do engine, não parte dele.

```
┌─────────────────────────────────────────────────────┐
│                   GODOT 4 (render)                  │
│         cenas, animações, UI, áudio, input raw      │
└────────────────────┬────────────────────────────────┘
                     │ GDExtension (gdext)
┌────────────────────▼────────────────────────────────┐
│               godot_bridge/  (adaptador)            │
│     converte input Godot → InputEvent               │
│     converte SimState → posições/animações Godot    │
└──────┬──────────────────────────────────┬───────────┘
       │                                  │
┌──────▼──────┐                  ┌────────▼────────┐
│   input/    │                  │     net/        │
│  adaptador  │                  │   adaptador     │
│  normaliza  │                  │  UDP, DHT,      │
│  InputEvent │                  │  serialização   │
└──────┬──────┘                  └────────┬────────┘
       │                                  │
┌──────▼──────────────────────────────────▼────────┐
│                    core/                          │
│   ECS substrate · EventBus · tipos primitivos    │
│              TickId, EntityId, Fixed             │
└──────────────────────┬───────────────────────────┘
                       │
┌──────────────────────▼───────────────────────────┐
│                    sim/                           │
│     sistemas determinísticos · componentes       │
│     movimento, combate, habilidades, buff        │
└──────────────────────┬───────────────────────────┘
                       │
┌──────────────────────▼───────────────────────────┐
│                 rollback/                         │
│     snapshot buffer · re-simulação · clock sync  │
└──────────────────────────────────────────────────┘
```

---

## Princípios Arquiteturais

### 1. Clean Architecture nas bordas, DOD no núcleo
- `godot_bridge`, `net`, `input` são **adaptadores** — conversam com o mundo externo
- `core`, `sim`, `rollback` são **domínio puro** — sem I/O, sem dependências externas
- Dependências só apontam para dentro: bordas → core, nunca o contrário

### 2. ECS (Entity Component System)
- **Entity:** ID numérico puro — sem métodos, sem dados
- **Component:** struct de dados pura — sem lógica
- **System:** função que itera sobre componentes — sem estado próprio
- Separação absoluta entre dados e comportamento

### 3. Event-Driven Communication
- Módulos não se chamam diretamente
- Comunicação via `EventBus` síncrono — emite e drena no mesmo tick
- Zero eventos async dentro da simulation loop

### 4. Sem dependências hardcoded entre módulos
```
ERRADO:  sim::movement depende de net::PeerId
CERTO:   sim::movement emite MovementEvent, net consome via EventBus
```

### 5. Composição sobre herança
```
ERRADO:  struct Hero extends Character
CERTO:   entity com componentes Position + Health + MoveSpeed + AbilitySet
```

---

## Módulos

### `core/` — ECS Substrate e Primitivos
**Responsabilidade única:** fornecer os tipos base que todos os outros módulos usam.

```
core/
  ecs.rs         — World, EntityId, archetype storage
  events.rs      — EventBus, EventQueue<T>
  types.rs       — Fixed (I32F32), TickId, PlayerId, Vec2Fixed
  components.rs  — componentes base (Position, Velocity, Health...)
```

Sem dependências externas ao engine. Sem lógica de jogo.

### `sim/` — Simulação Determinística
**Responsabilidade única:** avançar o estado do jogo deterministicamente dado inputs.

```
sim/
  world.rs       — SimWorld: tick(), aplica inputs, roda sistemas
  systems/
    movement.rs  — aplica velocidade → posição
    combat.rs    — resolve dano, morte
    abilities.rs — cooldowns, efeitos
    buffs.rs     — aplica/remove buffs por duração
  rng.rs         — RNG determinístico (LCG com seed compartilhado)
```

**Lei:** nenhum sistema conhece o tipo do jogo. `combat.rs` não sabe o que é um "herói".

### `net/` — Transporte P2P
**Responsabilidade única:** enviar e receber dados entre peers confiável e com baixa latência.

```
net/
  socket.rs      — UDP non-blocking, send/recv
  dht.rs         — Kademlia: descoberta de peers, matchmaking
  protocol.rs    — serialização de NetMessage (bitcode)
  messages.rs    — definição de NetMessage enum
  clock.rs       — sincronização de relógio (Cristian's algorithm)
```

Sem lógica de jogo. Só transporte.

### `rollback/` — Netcode
**Responsabilidade única:** garantir estado consistente entre peers via rollback.

```
rollback/
  session.rs     — RollbackSession: avança, detecta divergência, rollback
  buffer.rs      — SnapshotBuffer circular (16 frames)
  prediction.rs  — predição de input de peers ausentes
  checksum.rs    — hash de estado para detecção de divergência
```

### `input/` — Adaptador de Input
**Responsabilidade única:** normalizar input raw (Godot ou teclado) em `InputEvent`.

```
input/
  collector.rs   — coleta InputEvent por tick, faz coalescing
  events.rs      — definição de InputEvent (tipo + posição + tick)
```

### `godot_bridge/` — Adaptador Godot
**Responsabilidade única:** traduzir entre o mundo Godot e o engine Rust.

```
godot_bridge/
  lib.rs         — extensão GDExtension
  renderer.rs    — lê SimState, atualiza posições de nodes Godot
  input_bridge.rs — converte InputEvent Godot → input::InputEvent
```

Zero lógica de simulação aqui.

---

## Regras de Dependência entre Módulos

```
godot_bridge → input, core, sim
input        → core
net          → core
rollback     → sim, core, net
sim          → core
core         → (nada do engine)
```

Se precisar de uma dependência fora desse grafo: **usar evento**.

---

## Determinismo — Contrato Absoluto

Toda função dentro de `sim/` deve ser determinística:
- Sem `f32` ou `f64` — usar `Fixed` (I32F32)
- Sem `HashMap` com ordem não garantida — usar `BTreeMap` ou Vec ordenado
- Sem `rand` — usar `sim::rng::DeterministicRng`
- Sem `SystemTime` — usar `TickId`
- Sem I/O de qualquer tipo

Violação de determinismo = divergência de estado entre peers = jogo quebrado.
