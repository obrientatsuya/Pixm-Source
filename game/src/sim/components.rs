/// Componentes da simulação — dados puros, zero lógica.
///
/// O engine não conhece "herói" ou "minion" — são composições destes componentes.
/// Toda lógica fica nos sistemas em sim/systems/.

use crate::core::types::{Fixed, Vec2Fixed, PlayerId};

// ─── Espaciais ───────────────────────────────────────────────────────────────

/// Posição no mundo (fixed-point).
#[derive(Debug, Clone, Copy, Default)]
pub struct Position(pub Vec2Fixed);

/// Posição no início do tick anterior — usada para interpolação de render.
#[derive(Debug, Clone, Copy, Default)]
pub struct PrevPosition(pub Vec2Fixed);

/// Velocidade atual (unidades/tick).
#[derive(Debug, Clone, Copy, Default)]
pub struct Velocity(pub Vec2Fixed);

/// Destino de movimento (click-to-move).
#[derive(Debug, Clone, Copy)]
pub struct MoveTarget(pub Vec2Fixed);

/// Velocidade máxima de movimento (unidades/tick).
#[derive(Debug, Clone, Copy)]
pub struct MoveSpeed(pub Fixed);

// ─── Combate ─────────────────────────────────────────────────────────────────

/// Pontos de vida.
#[derive(Debug, Clone, Copy)]
pub struct Health {
    pub current: i32,
    pub max:     i32,
}

impl Health {
    pub fn new(max: i32) -> Self { Self { current: max, max } }
    pub fn is_dead(&self) -> bool { self.current <= 0 }
    pub fn apply_damage(&mut self, amount: i32) { self.current = (self.current - amount).max(0); }
}

/// Range de ataque (fixed-point — comparar com dist_sq).
#[derive(Debug, Clone, Copy)]
pub struct AttackRange(pub Fixed);

/// Dano base de auto-attack.
#[derive(Debug, Clone, Copy)]
pub struct AttackDamage(pub i32);

/// Ticks entre auto-attacks.
#[derive(Debug, Clone, Copy)]
pub struct AttackSpeed(pub u32);

/// Cooldown atual de auto-attack (ticks restantes).
#[derive(Debug, Clone, Copy, Default)]
pub struct AttackCooldown(pub u32);

/// Chance de crítico (0..100 inteiro).
#[derive(Debug, Clone, Copy, Default)]
pub struct CritChance(pub u8);

// ─── Crowd Control ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CcKind {
    Stun,
    Root,
    Slow(u8), // porcentagem de redução (0..100)
    Knockup,
    Silence,
}

/// CC ativo nesta entidade.
#[derive(Debug, Clone, Copy)]
pub struct CrowdControl {
    pub kind:            CcKind,
    pub ticks_remaining: u32,
}

// ─── Buffs ───────────────────────────────────────────────────────────────────

/// Buff ou debuff com ID e duração em ticks.
#[derive(Debug, Clone, Copy)]
pub struct Buff {
    pub id:              u16,
    pub stacks:          u8,
    pub ticks_remaining: u32,
    pub magnitude:       i32, // valor do efeito (depende do buff_id)
}

// ─── Identidade ──────────────────────────────────────────────────────────────

/// Dono da entidade (qual player controla).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Owner(pub PlayerId);

/// Time da entidade (0 = time A, 1 = time B).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Team(pub u8);

/// Range de visão (para fog of war).
#[derive(Debug, Clone, Copy)]
pub struct VisionRange(pub Fixed);

// ─── IA ──────────────────────────────────────────────────────────────────────

/// Entidade alvo da IA (minions, torres).
#[derive(Debug, Clone, Copy)]
pub struct AiTarget(pub hecs::Entity);

/// Waypoints do caminho fixo (minions seguem lanes).
#[derive(Debug, Clone, Copy)]
pub struct WaypointIndex(pub u8);

/// Lane à qual este minion pertence — índice em LanePaths.
#[derive(Debug, Clone, Copy)]
pub struct LaneId(pub u8);

// ─── Projéteis ───────────────────────────────────────────────────────────────

/// Dados de um projétil em voo.
#[derive(Debug, Clone, Copy)]
pub struct Projectile {
    pub owner:    hecs::Entity,
    pub damage:   i32,
    pub speed:    Fixed,
    pub target:   ProjectileTarget,
}

#[derive(Debug, Clone, Copy)]
pub enum ProjectileTarget {
    Entity(hecs::Entity),            // projétil trackeia entidade (homing)
    Point(Vec2Fixed),                // projétil vai para ponto fixo (skillshot)
}

// ─── Pathfinding ─────────────────────────────────────────────────────────────

/// Caminho computado pelo A* — waypoints para seguir até o destino.
/// Criado por pathfinding_system, consumido por move_target_system.
#[derive(Debug, Clone)]
pub struct Path {
    pub waypoints:   Vec<Vec2Fixed>,
    pub current:     usize,
    pub destination: Vec2Fixed, // destino original (detecta novo click)
}

impl Path {
    /// Waypoint atual (None = caminho esgotado).
    pub fn current_wp(&self) -> Option<Vec2Fixed> {
        self.waypoints.get(self.current).copied()
    }

    pub fn advance(&mut self) {
        self.current += 1;
    }

    pub fn exhausted(&self) -> bool {
        self.current >= self.waypoints.len()
    }
}

// ─── Habilidades ─────────────────────────────────────────────────────────────

/// Efeito de uma habilidade — o engine aplica, o jogo configura.
#[derive(Debug, Clone, Copy)]
pub enum AbilityEffect {
    /// Dano no inimigo mais próximo do ponto clicado.
    InstantDamage { amount: i32, hit_radius: Fixed, cc: Option<(CcKind, u32)> },
    /// Dano em todos os inimigos no raio do ponto clicado.
    AreaDamage    { radius: Fixed, amount: i32 },
    /// Projétil viajando até o alvo (Fase 11 implementa; por ora direct damage).
    Projectile    { speed: Fixed, damage: i32 },
    /// Dash em direção ao ponto clicado.
    Dash          { distance: Fixed },
}

/// Definição de uma habilidade — dados puros.
#[derive(Debug, Clone, Copy)]
pub struct AbilityDef {
    pub range:    Fixed, // alcance do lançador ao ponto alvo
    pub cooldown: u32,   // ticks entre usos
    pub effect:   AbilityEffect,
}

/// 4 slots de habilidade por entidade (Q/W/E/R).
#[derive(Debug, Clone)]
pub struct AbilitySlots(pub [Option<AbilityDef>; 4]);

impl Default for AbilitySlots {
    fn default() -> Self { Self([None; 4]) }
}

/// Cooldowns ativos por slot (ticks restantes).
#[derive(Debug, Clone, Copy, Default)]
pub struct AbilityCooldowns(pub [u32; 4]);

/// Habilidade pendente de processar neste tick.
/// Adicionada por apply_inputs, consumida por ability_system.
#[derive(Debug, Clone, Copy)]
pub struct PendingAbility {
    pub slot:     u8,
    pub target_x: Fixed,
    pub target_y: Fixed,
}

// ─── Marcadores (zero dados — presença = propriedade) ────────────────────────

/// Marcador: entidade será removida ao fim do tick.
pub struct Dying;

/// Marcador: entidade está morta (aguardando respawn).
pub struct Dead;
