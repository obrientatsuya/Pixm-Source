# ECS Spec — Pixm Engine

## Conceito Central

```
Entity    = u64 (ID puro, sem dados, sem métodos)
Component = struct de dados pura (sem lógica)
System    = fn(&mut World) que itera componentes e transforma dados
Event     = dado imutável emitido por sistemas, consumido por outros sistemas
```

O engine não sabe o que é um "herói", "minion" ou "torre".
Isso é configuração do jogo — componentes compostos pelo jogo, não pelo engine.

---

## Entidades

```rust
// Entity é só um número
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EntityId(pub u64);

// Criação
let entity = world.spawn();           // gera EntityId único
world.despawn(entity);                // remove entity e todos os componentes

// O engine nunca tipifica entidades
// ERRADO:  enum EntityKind { Hero, Minion, Tower }
// CERTO:   a combinação de componentes define o que a entidade é
```

---

## Componentes Base (core/)

Componentes são dados. Nada mais.

```rust
use fixed::types::I32F32;
pub type Fixed = I32F32;

// Posição no mundo (fixed-point, determinístico)
#[derive(Clone, Copy, Debug, Default)]
pub struct Position { pub x: Fixed, pub y: Fixed }

// Velocidade (unidades/tick)
#[derive(Clone, Copy, Debug, Default)]
pub struct Velocity { pub x: Fixed, pub y: Fixed }

// Saúde
#[derive(Clone, Copy, Debug)]
pub struct Health { pub current: i32, pub max: i32 }

// Dono (qual peer controla esta entidade)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner(pub PlayerId);

// Marcador de time (sem dados — presença = pertence ao time)
#[derive(Clone, Copy, Debug)]
pub struct Team(pub u8);

// Velocidade de movimento
#[derive(Clone, Copy, Debug)]
pub struct MoveSpeed(pub Fixed);

// Destino de movimento (click-to-move)
#[derive(Clone, Copy, Debug)]
pub struct MoveTarget { pub x: Fixed, pub y: Fixed }
```

**Regra:** componentes não têm métodos além de `new()` e derives. Toda lógica fica nos sistemas.

---

## Composição de Entidades (definida pelo jogo, não pelo engine)

```rust
// O jogo (não o engine) define o que é um "herói":
fn spawn_hero(world: &mut World, player: PlayerId, pos: (Fixed, Fixed)) -> EntityId {
    world.spawn_with((
        Position { x: pos.0, y: pos.1 },
        Velocity::default(),
        Health { current: 1000, max: 1000 },
        MoveSpeed(Fixed::from_num(4)),
        Owner(player),
        Team(player.team()),
        AbilitySet::default(),  // componente definido pelo jogo
    ))
}

// "Minion" é outra composição:
fn spawn_minion(world: &mut World, team: u8, pos: (Fixed, Fixed)) -> EntityId {
    world.spawn_with((
        Position { x: pos.0, y: pos.1 },
        Velocity::default(),
        Health { current: 400, max: 400 },
        MoveSpeed(Fixed::from_num(2)),
        Team(team),
        AiTarget::default(),  // componente de IA
    ))
}
// O engine não diferencia herói de minion — são só arquétipos de componentes
```

---

## Sistemas

Sistemas são funções puras que operam sobre o World. Sem estado próprio.

```rust
// Signature padrão de sistema
pub fn system_name(world: &mut World, events: &mut EventQueue<OutputEvent>)

// Exemplo — sistema de movimento (engine genérico, não sabe o que é herói)
pub fn movement_system(world: &mut World, _events: &mut EventQueue<()>) {
    for (_, (pos, vel)) in world.query_mut::<(&mut Position, &Velocity)>() {
        pos.x += vel.x;
        pos.y += vel.y;
    }
}

// Exemplo — sistema de resolução de destino (click-to-move)
pub fn move_target_system(world: &mut World, _events: &mut EventQueue<()>) {
    for (_, (pos, vel, target, speed)) in world
        .query_mut::<(&Position, &mut Velocity, &MoveTarget, &MoveSpeed)>()
    {
        let dx = target.x - pos.x;
        let dy = target.y - pos.y;
        let dist = (dx * dx + dy * dy).sqrt();  // fixed-point sqrt
        if dist > Fixed::from_num(0) {
            vel.x = (dx / dist) * speed.0;
            vel.y = (dy / dist) * speed.0;
        }
    }
}
```

**Regras dos sistemas:**
- Sem estado interno (sem `static`, sem campos)
- Sem I/O
- Sem `f32/f64`
- Sem lógica específica de jogo (não conhece "herói", "habilidade Q")
- Funções curtas — se passou de 30 linhas, divida

---

## Event Bus (comunicação entre sistemas)

Sistemas não se chamam diretamente. Comunicam via eventos síncronos.

```rust
// EventQueue é um Vec drenado no fim de cada tick
pub struct EventQueue<T> {
    events: Vec<T>,
}

impl<T> EventQueue<T> {
    pub fn emit(&mut self, event: T) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        self.events.drain(..)
    }
}

// Eventos são dados imutáveis — sem callbacks, sem closures
#[derive(Debug, Clone)]
pub struct DamageEvent {
    pub source: EntityId,
    pub target: EntityId,
    pub amount: i32,
    pub tick:   TickId,
}

#[derive(Debug, Clone)]
pub struct DeathEvent {
    pub entity:  EntityId,
    pub killer:  EntityId,
    pub tick:    TickId,
}
```

### Fluxo de eventos por tick

```
tick N:
  movement_system   → emite CollisionEvent
  combat_system     → consome CollisionEvent, emite DamageEvent
  health_system     → consome DamageEvent, emite DeathEvent
  cleanup_system    → consome DeathEvent, despawn entidades
  [fim do tick]     → todos os EventQueues são drenados
```

Nunca carregue evento de um tick pro próximo. Cada tick começa com queues vazias.

---

## Ordem de Execução dos Sistemas (por tick)

```rust
pub fn run_tick(world: &mut World, inputs: &[InputEvent], events: &mut EventBus) {
    // 1. Aplicar inputs — converte InputEvent em componentes (MoveTarget, etc.)
    input_system(world, inputs, &mut events.input_results);

    // 2. Resolver intenções de movimento
    move_target_system(world, &mut events.dummy);

    // 3. Integrar posições
    movement_system(world, &mut events.dummy);

    // 4. Detectar colisões
    collision_system(world, &mut events.collisions);

    // 5. Resolver combate
    combat_system(world, &mut events.collisions, &mut events.damage);

    // 6. Aplicar dano
    health_system(world, &mut events.damage, &mut events.deaths);

    // 7. Processar mortes
    death_system(world, &mut events.deaths);

    // 8. Atualizar buffs/cooldowns
    buff_system(world, &mut events.dummy);

    // 9. Drena todos os eventos (não carrega pro próximo tick)
    events.clear_all();
}
```

---

## Snapshot para Rollback

O estado do ECS precisa ser serializável para rollback.

```rust
// Snapshot é uma cópia completa do estado da simulação
// Salvo a cada tick, mantido em buffer circular de 16 frames
pub struct SimSnapshot {
    pub tick:     TickId,
    pub entities: Vec<(EntityId, ComponentSet)>,
    pub rng_state: u64,
}

// ComponentSet usa serde para serialização determinística
// hecs suporta serialização via hecs::serialize
```

---

## Proibições Absolutas no ECS

```
❌  if entity_type == EntityType::Hero { ... }   — tipo hardcoded
❌  hero.cast_ability_q()                        — método de jogo em entidade
❌  static WORLD: Mutex<World>                   — global state
❌  fn big_system() { /* 200 linhas */ }         — função grande
❌  pos.x += vel.x as f32                        — f32 na simulação
❌  use std::collections::HashMap em componente  — ordem não garantida
```
