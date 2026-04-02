/// SimWorld — estado completo da simulação determinística.
///
/// Implementa net::rollback::session::Simulation para integrar com rollback.
/// run_tick() executa todos os sistemas na ordem correta.

use hecs::World;
use crate::core::events::EventBus;
use crate::sim::rng::DeterministicRng;
use crate::sim::systems::{abilities, movement, combat, buffs};
use crate::sim::pathfinding::{pathfinding_system, NavigationGrid};
use crate::input::events::InputEvent;
use net::rollback::prediction::RawInput;
use net::rollback::session::Simulation;

pub struct SimWorld {
    pub world:    World,
    pub events:   EventBus,
    pub rng:      DeterministicRng,
    pub tick:     u64,
    pub nav_grid: NavigationGrid,
}

impl SimWorld {
    pub fn new(rng_seed: u64) -> Self {
        Self {
            world:    World::new(),
            events:   EventBus::new(),
            rng:      DeterministicRng::new(rng_seed),
            tick:     0,
            nav_grid: NavigationGrid::default_128(),
        }
    }

    /// Executa 1 tick completo com os inputs fornecidos.
    pub fn run_tick(&mut self, inputs: &[InputEvent]) {
        // 1. Aplica inputs → componentes (MoveTarget, abilities, etc.)
        self.apply_inputs(inputs);

        // 2. Pathfinding: MoveTarget → Path de waypoints (só quando muda)
        pathfinding_system(&mut self.world, &self.nav_grid);

        // 3. Movimento
        movement::move_target_system(&mut self.world);
        movement::movement_system(&mut self.world);
        movement::clear_arrived_targets(&mut self.world);

        // 4. Habilidades pendentes → efeitos
        abilities::ability_system(&mut self.world, &mut self.events);

        // 5. Cooldowns (auto-attack + habilidades)
        combat::cooldown_system(&mut self.world);
        abilities::ability_cooldown_system(&mut self.world);

        // 6. Auto-attack → DamageEvents
        combat::auto_attack_system(&mut self.world, &mut self.rng, &mut self.events);

        // 7. Resolve dano → DeathEvents
        combat::health_system(&mut self.world, &mut self.events);

        // 8. Processa mortes → marca Dying
        combat::death_system(&mut self.world, &mut self.events);

        // 9. Buffs/CC
        buffs::crowd_control_system(&mut self.world);
        buffs::buff_system(&mut self.world, &mut self.events);

        // 10. Remove entidades mortas
        combat::cleanup_system(&mut self.world);

        // 10. Drena eventos restantes (não carrega pro próximo tick)
        self.events.clear();
        self.tick += 1;
    }

    fn apply_inputs(&mut self, inputs: &[InputEvent]) {
        use crate::core::types::Vec2Fixed;
        use crate::sim::components::{MoveTarget, Owner, PendingAbility};
        use crate::input::events::bits_to_fixed;

        for input in inputs {
            match input {
                InputEvent::MoveGround { x_bits, y_bits, .. } => {
                    let target = Vec2Fixed::new(bits_to_fixed(*x_bits), bits_to_fixed(*y_bits));
                    // Extrai player_id do enum
                    let pid = input.player_id();
                    // Atualiza MoveTarget existente
                    for (_, (owner, mt)) in
                        self.world.query_mut::<(&Owner, &mut MoveTarget)>()
                    {
                        if owner.0 == pid { mt.0 = target; }
                    }
                    // Adiciona MoveTarget se ainda não tem
                    let entities: Vec<hecs::Entity> = self.world
                        .query::<(&Owner,)>()
                        .iter()
                        .filter(|(_, (o,))| o.0 == pid)
                        .map(|(e, _)| e)
                        .collect();
                    for e in entities {
                        if self.world.get::<&MoveTarget>(e).is_err() {
                            let _ = self.world.insert_one(e, MoveTarget(target));
                        }
                    }
                }
                InputEvent::Ability { player_id, slot, x_bits, y_bits, .. } => {
                    let pending = PendingAbility {
                        slot:     *slot,
                        target_x: bits_to_fixed(*x_bits),
                        target_y: bits_to_fixed(*y_bits),
                    };
                    let pid = *player_id;
                    let entities: Vec<hecs::Entity> = self.world
                        .query::<&Owner>().iter()
                        .filter(|(_, o)| o.0 == pid)
                        .map(|(e, _)| e)
                        .collect();
                    for e in entities {
                        let _ = self.world.insert_one(e, pending);
                    }
                }
                _ => {}
            }
        }
    }

    /// Hash rápido FNV-1a do estado — para detecção de divergência.
    pub fn checksum(&self) -> u64 {
        use crate::sim::components::{Position, Health};
        let mut hash: u64 = 0xcbf29ce484222325;
        for (e, (pos, hp)) in self.world.query::<(&Position, &Health)>().iter() {
            let id_bytes = e.id().to_le_bytes();
            let x_bytes  = pos.0.x.to_bits().to_le_bytes();
            let hp_bytes = hp.current.to_le_bytes();
            for b in id_bytes.iter().chain(&x_bytes).chain(&hp_bytes) {
                hash ^= *b as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        hash
    }
}

// ─── Integração com rollback ──────────────────────────────────────────────────

impl Clone for SimWorld {
    fn clone(&self) -> Self {
        // nav_grid não muda durante a partida — clona direto (barato)
        // Estado da sim clona via serialização para garantir equivalência
        let data = self.serialize();
        let mut s = Self::deserialize(&data);
        s.nav_grid = self.nav_grid.clone();
        s
    }
}

impl Simulation for SimWorld {
    fn serialize(&self) -> Vec<u8> {
        use crate::sim::components::{Position, Health};
        use crate::core::types::Fixed;

        let mut buf = Vec::new();
        buf.extend(self.tick.to_le_bytes());
        buf.extend(self.rng.state().to_le_bytes());
        for (e, (pos, hp)) in self.world.query::<(&Position, &Health)>().iter() {
            buf.extend(e.id().to_le_bytes());
            buf.extend(pos.0.x.to_bits().to_le_bytes());
            buf.extend(pos.0.y.to_bits().to_le_bytes());
            buf.extend(hp.current.to_le_bytes());
            buf.extend(hp.max.to_le_bytes());
        }
        buf
    }

    fn deserialize(data: &[u8]) -> Self {
        // Snapshot mínimo — implementação completa requer hecs::serialize
        if data.len() < 16 { return Self::new(0); }
        let tick      = u64::from_le_bytes(data[0..8].try_into().unwrap_or_default());
        let rng_state = u64::from_le_bytes(data[8..16].try_into().unwrap_or_default());
        let mut sim = Self::new(0);
        sim.tick = tick;
        sim.rng.restore(rng_state);
        sim
    }

    fn step(&mut self, inputs: &[RawInput]) {
        // Converte RawInput → InputEvent via bitcode
        let events: Vec<InputEvent> = inputs.iter()
            .filter_map(|raw| bitcode::decode(&raw.data).ok())
            .collect();
        self.run_tick(&events);
    }

    fn checksum(&self) -> u64 { self.checksum() }
    fn rng_state(&self) -> u64 { self.rng.state() }
}
