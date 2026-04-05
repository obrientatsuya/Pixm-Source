/// GameLoopNode — ponto de entrada Godot. Roda o game loop.
///
/// Godot chama _process() a cada frame:
///   - Accumulator → ticks fixos 60Hz
///   - update_render(alpha) → posição interpolada de todos os nodes linkados
///
/// render_scale: unidades sim → pixels Godot (default 1.0).
/// Usar set_render_scale(8.0) quando coordenadas sim estão em células (0-128).

use std::collections::HashMap;
use godot::prelude::*;
use crate::sim::world::SimWorld;
use crate::sim::spawn;
use crate::input::collector::InputCollector;
use crate::core::types::{Fixed, Vec2Fixed};

const TICK_DT: f64 = 1.0 / 60.0;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct GameLoopNode {
    sim:          Option<SimWorld>,
    collector:    InputCollector,
    accumulator:  f64,
    render_scale: f32,
    hero_node:    Option<Gd<Node2D>>,
    entity_nodes: HashMap<u32, Gd<Node2D>>,
    base:         Base<Node>,
}

#[godot_api]
impl INode for GameLoopNode {
    fn init(base: Base<Node>) -> Self {
        Self {
            sim:          None,
            collector:    InputCollector::new(),
            accumulator:  0.0,
            render_scale: 1.0,
            hero_node:    None,
            entity_nodes: HashMap::new(),
            base,
        }
    }

    fn ready(&mut self) {
        tracing::info!("GameLoopNode pronto");
    }

    fn process(&mut self, delta: f64) {
        let Some(ref mut sim) = self.sim else { return };
        self.accumulator += delta;
        while self.accumulator >= TICK_DT {
            let inputs = self.collector.drain();
            sim.run_tick(&inputs);
            self.accumulator -= TICK_DT;
        }
        let alpha = Fixed::from_num(self.accumulator / TICK_DT);
        self.update_render(alpha);
    }
}

#[godot_api]
impl GameLoopNode {
    #[func]
    pub fn start_match(&mut self, rng_seed: i64) {
        self.sim = Some(SimWorld::new(rng_seed as u64));
        self.accumulator = 0.0;
        self.entity_nodes.clear();
    }

    /// Pixels por unidade de simulação. Afeta rendering e conversão de cliques.
    #[func]
    pub fn set_render_scale(&mut self, scale: f32) {
        self.render_scale = scale.max(0.001);
    }

    #[func]
    pub fn spawn_hero(&mut self, start_x: f32, start_y: f32) {
        use crate::sim::components::*;
        use crate::core::types::PlayerId;
        let Some(ref mut sim) = self.sim else { return };
        let pos = Vec2Fixed::new(Fixed::from_num(start_x), Fixed::from_num(start_y));
        sim.world.spawn((
            Position(pos), PrevPosition(pos), Velocity::default(),
            MoveSpeed(Fixed::from_num(3.0)),
            Owner(PlayerId(0)), Health::new(500), Team(0),
            crate::sim::components::AbilitySlots::default(),
            crate::sim::components::AbilityCooldowns::default(),
        ));
    }

    /// Spawna boneco alvo estático. Retorna entity_id para link_node.
    #[func]
    pub fn spawn_dummy(&mut self, x: f32, y: f32) -> i64 {
        let Some(ref mut sim) = self.sim else { return -1 };
        let pos = Vec2Fixed::new(Fixed::from_num(x), Fixed::from_num(y));
        let e = spawn::spawn_dummy(&mut sim.world, pos, 1);
        e.id() as i64
    }

    /// Spawna minion com IA de lane. Retorna entity_id para link_node.
    #[func]
    pub fn spawn_minion(&mut self, x: f32, y: f32, lane_id: i32) -> i64 {
        let Some(ref mut sim) = self.sim else { return -1 };
        let pos = Vec2Fixed::new(Fixed::from_num(x), Fixed::from_num(y));
        let e = spawn::spawn_minion(&mut sim.world, pos, 0, lane_id as u8);
        e.id() as i64
    }

    /// Associa um Node2D a uma entidade (por entity_id retornado nos spawns).
    #[func]
    pub fn link_node(&mut self, entity_id: i64, node: Gd<Node2D>) {
        self.entity_nodes.insert(entity_id as u32, node);
    }

    /// Marca célula da grade de navegação como bloqueada (obstáculo).
    #[func]
    pub fn set_obstacle(&mut self, cx: i32, cy: i32, blocked: bool) {
        let Some(ref mut sim) = self.sim else { return };
        sim.nav_grid.set_blocked(cx, cy, blocked);
    }

    /// Adiciona ponto à lane especificada (em unidades de simulação).
    #[func]
    pub fn add_lane_point(&mut self, lane_id: i32, x: f32, y: f32) {
        let Some(ref mut sim) = self.sim else { return };
        let lane_id = lane_id as usize;
        while sim.lane_paths.0.len() <= lane_id { sim.lane_paths.0.push(vec![]); }
        let pt = Vec2Fixed::new(Fixed::from_num(x), Fixed::from_num(y));
        sim.lane_paths.0[lane_id].push(pt);
    }

    #[func]
    pub fn set_hero_node(&mut self, node: Gd<Node2D>) { self.hero_node = Some(node); }

    /// Clique de terreno — converte pixel Godot → unidade sim.
    #[func]
    pub fn on_ground_click(&mut self, world_x: f32, world_y: f32) {
        let sx = world_x / self.render_scale;
        let sy = world_y / self.render_scale;
        let event = crate::bridge::input_bridge::on_ground_click(0, Vector2::new(sx, sy));
        self.collector.push(event);
    }

    #[func]
    pub fn on_ability(&mut self, slot: i32, world_x: f32, world_y: f32) {
        let sx = world_x / self.render_scale;
        let sy = world_y / self.render_scale;
        let event = crate::bridge::input_bridge::on_ability_key(
            0, slot as u8, Vector2::new(sx, sy), None,
        );
        self.collector.push(event);
    }

    #[func]
    pub fn get_hero_hp(&self) -> Vector2i {
        use crate::sim::components::{Health, Owner};
        use crate::core::types::PlayerId;
        self.sim.as_ref()
            .and_then(|sim| {
                sim.world.query::<(&Health, &Owner)>().iter()
                    .find(|(_, (_, o))| o.0 == PlayerId(0))
                    .map(|(_, (hp, _))| Vector2i::new(hp.current, hp.max))
            })
            .unwrap_or(Vector2i::new(0, 1))
    }

    #[func]
    pub fn current_tick(&self) -> i64 {
        self.sim.as_ref().map(|s| s.tick as i64).unwrap_or(0)
    }
}

impl GameLoopNode {
    fn update_render(&mut self, alpha: Fixed) {
        use crate::sim::components::{Position, PrevPosition, Owner};
        use crate::core::types::PlayerId;

        let scale = self.render_scale;

        let Some(ref sim) = self.sim else { return };

        // Coleta posições interpoladas de todas as entidades com Position
        let positions: Vec<(u32, Vector2)> = sim.world
            .query::<(&Position, Option<&PrevPosition>)>()
            .iter()
            .map(|(e, (pos, prev))| {
                let from = prev.map(|p| p.0).unwrap_or(pos.0);
                let ip   = from.lerp(pos.0, alpha);
                let gp   = Vector2::new(ip.x.to_num::<f32>() * scale,
                                        ip.y.to_num::<f32>() * scale);
                (e.id(), gp)
            })
            .collect();

        // Herói local
        if let Some(hero) = self.hero_node.as_mut() {
            if let Some(p) = hero_pos(&positions, sim, scale) {
                hero.set_position(p);
            }
        }

        // Todas as entidades linkadas
        for (id, node) in self.entity_nodes.iter_mut() {
            if let Some(&(_, gp)) = positions.iter().find(|(eid, _)| eid == id) {
                node.set_position(gp);
            }
        }

        // Câmera segue herói — posiciona o Node pai (processa via hero_node)
        fn hero_pos(
            positions: &[(u32, Vector2)],
            sim: &SimWorld,
            _scale: f32,
        ) -> Option<Vector2> {
            let hero_eid = sim.world
                .query::<&Owner>().iter()
                .find(|(_, o)| o.0 == crate::core::types::PlayerId(0))
                .map(|(e, _)| e.id())?;
            positions.iter().find(|(id, _)| *id == hero_eid).map(|(_, p)| *p)
        }
    }
}
