# Determinismo — Mecânicas LoL-like

O próprio LoL é determinístico — replays funcionam re-simulando apenas inputs.
Todas as mecânicas abaixo são implementáveis de forma determinística.

## Regra absoluta

> Tudo na simulação usa `Fixed` (I32F32) ou inteiros (`i32`, `u32`).
> Nunca `f32`. RNG sempre vem de `DeterministicRng` com seed compartilhada.

---

## Mecânicas e como torná-las determinísticas

### Movimento (click-to-move)
```
posição:   Vec2Fixed (fixed-point)
velocidade: Vec2Fixed por tick
destino:   MoveTarget { x: Fixed, y: Fixed }
```
Movimento = interpolação linear fixed-point entre posição e destino por tick.

### Pathfinding
Grid de células inteiras. A* com tie-breaking explícito.

```
Mapa → grid 512×512 de células inteiras
Caminho → sequência de (grid_x: u16, grid_y: u16)
Tie-break → quando dois nós têm custo igual, sempre menor índice de célula
```

Sem navmesh (usa float internamente). Grid fixo garante mesmo caminho em todos os peers.

### Dano e Crítico (RNG)
```rust
// RNG compartilhado — seed distribuída via DHT no início da partida
let roll = rng.next_u32() % 100;
let is_crit = roll < crit_chance_pct; // inteiro 0-100

// Dano variável (ex: 80-120)
let variance = rng.next_u32() % 41;
let damage = 80u32 + variance;
```

### Projéteis (Caitlyn Q, Ezreal Q)
```
posição:   Vec2Fixed, avança por tick
velocidade: Vec2Fixed constante
colisão:   circle_hits() com fixed-point (sem sqrt — compara quadrado da distância)
```

```rust
fn circle_hits(origin: Vec2Fixed, radius: Fixed, target: Vec2Fixed) -> bool {
    let dx = target.x - origin.x;
    let dy = target.y - origin.y;
    dx * dx + dy * dy <= radius * radius  // fixed-point, sem sqrt
}
```

### Crowd Control (Stun, Slow, Root, Knockup)
```
Componente: CrowdControl { kind: CcKind, ticks_remaining: u32 }
Sistema:    reduz ticks_remaining por tick, aplica efeito enquanto > 0
```
Duração em ticks, não segundos. Determinístico.

### Dash (Katarina E, Lee Sin Q2)
```
Componente: Dash { target: Vec2Fixed, ticks_total: u32, ticks_elapsed: u32 }
Sistema:    interpola posição via lerp fixed-point por tick
            ao terminar: remove componente Dash
```

### Buffs/Debuffs (duração, stacking)
```
Componente: Buff { id: BuffId, stacks: u8, ticks_remaining: u32 }
Sistema:    decrementa ticks por tick, remove quando = 0
            stacking: query por BuffId, incrementa stacks
```

### Aggro de Minion (alvo de menor HP, tie-break por EntityId)
```rust
fn select_minion_target(world: &World, attacker: EntityId) -> Option<EntityId> {
    world.query::<(&Health, &Team)>()
        .iter()
        .filter(|(_, (_, team))| team.0 != attacker_team)
        .min_by_key(|(id, (hp, _))| (hp.current, id.0))  // menor HP, tie: menor ID
        .map(|(id, _)| id)
}
```

### Fog of War
```
VisionMap: grid de bits (1 bit por célula, 1 = visível)
Calculado deterministicamente a cada tick por posição de heróis + wards
Idêntico em todos os peers — é parte da simulação, não do render
```

### Auto-attack
```
range check: circle_hits(attacker_pos, attack_range, target_pos)
attack_speed: ticks entre ataques (inteiro)
animação de ataque: responsabilidade do render, não da sim
```

### Objetivos (Dragão, Barão)
```
Entidade normal com componentes Health + Team(neutro) + AiTarget
Drops (buffs): aplicados via evento DamageEvent → DeathEvent → SpawnBuffEvent
```

---

## Proibições na simulação

```
❌  f32/f64 em qualquer cálculo
❌  rand::thread_rng() — usar apenas DeterministicRng
❌  HashMap para iterar entidades — usar BTreeMap ou Vec ordenado
❌  Navmesh com coordenadas float
❌  std::time na lógica de simulação
❌  Física externa (Rapier, Box2D)
```

---

## Verificação de divergência

A cada 60 ticks: hash FNV-1a do estado completo da simulação.
Broadcast para todos os peers. Hash diferente = bug de determinismo.
Log do tick onde divergiu → facilita debug.
