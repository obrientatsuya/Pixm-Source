/// GameLoopNode — ponto de entrada Godot. Roda o game loop.
///
/// Godot chama _process() a cada frame:
///   - Accumulator → ticks fixos 60Hz
///   - update_render(alpha) → posição interpolada dos nodes Godot
///
/// Zero lógica de jogo aqui — só tradução de fronteira.

use godot::prelude::*;
use crate::sim::world::SimWorld;
use crate::input::collector::InputCollector;
use crate::core::types::Fixed;

const TICK_DT: f64 = 1.0 / 60.0; // segundos por tick

#[derive(GodotClass)]
#[class(base=Node)]
pub struct GameLoopNode {
    sim:         Option<SimWorld>,
    collector:   InputCollector,
    accumulator: f64,
    hero_node:   Option<Gd<Node2D>>, // node do herói local (jogador 0)
    base:        Base<Node>,
}

#[godot_api]
impl INode for GameLoopNode {
    fn init(base: Base<Node>) -> Self {
        Self {
            sim:         None,
            collector:   InputCollector::new(),
            accumulator: 0.0,
            hero_node:   None,
            base,
        }
    }

    fn ready(&mut self) {
        tracing::info!("GameLoopNode pronto — aguardando start_match()");
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
    /// Inicializa a partida com seed do DHT.
    #[func]
    pub fn start_match(&mut self, rng_seed: i64) {
        self.sim = Some(SimWorld::new(rng_seed as u64));
        self.accumulator = 0.0;
        tracing::info!("partida iniciada seed={rng_seed}");
    }

    /// Spawna herói do jogador local na posição inicial.
    /// Chamar após start_match(), antes do primeiro frame.
    #[func]
    pub fn spawn_hero(&mut self, start_x: f32, start_y: f32) {
        use crate::sim::components::{Position, Velocity, MoveSpeed, Owner};
        use crate::core::types::{PlayerId, Vec2Fixed};

        let Some(ref mut sim) = self.sim else { return };
        sim.world.spawn((
            Position(Vec2Fixed::new(Fixed::from_num(start_x), Fixed::from_num(start_y))),
            Velocity::default(),
            MoveSpeed(Fixed::from_num(3.0)), // 3 unid/tick = 180 unid/s @ 60Hz
            Owner(PlayerId(0)),
        ));
    }

    /// Registra o Node2D que representa o herói local.
    /// Após isso, update_render() move este node automaticamente.
    #[func]
    pub fn set_hero_node(&mut self, node: Gd<Node2D>) {
        self.hero_node = Some(node);
    }

    /// Chamado pelo Godot quando o jogador clica no terreno (botão direito).
    #[func]
    pub fn on_ground_click(&mut self, world_x: f32, world_y: f32) {
        let event = crate::bridge::input_bridge::on_ground_click(
            0,
            Vector2::new(world_x, world_y),
        );
        self.collector.push(event);
    }

    /// Chamado pelo Godot ao pressionar tecla de habilidade.
    #[func]
    pub fn on_ability(&mut self, slot: i32, world_x: f32, world_y: f32) {
        let event = crate::bridge::input_bridge::on_ability_key(
            0, slot as u8,
            Vector2::new(world_x, world_y),
            None,
        );
        self.collector.push(event);
    }

    /// HP do herói local: retorna Vector2i(current, max).
    /// Usado pelo GDScript para atualizar a barra de vida.
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

    /// Tick atual (para debug/UI).
    #[func]
    pub fn current_tick(&self) -> i64 {
        self.sim.as_ref().map(|s| s.tick as i64).unwrap_or(0)
    }
}

impl GameLoopNode {
    /// Lê posição da sim e atualiza o hero_node (sem interpolação — snap).
    /// TODO: interpolar entre prev_pos e cur_pos usando alpha para suavidade.
    fn update_render(&mut self, _alpha: Fixed) {
        use crate::sim::components::{Position, Owner};
        use crate::core::types::PlayerId;

        // Lê posição do jogador local sem mover o borrow de hero_node
        let hero_pos = self.sim.as_ref().and_then(|sim| {
            sim.world
                .query::<(&Position, &Owner)>()
                .iter()
                .find(|(_, (_, o))| o.0 == PlayerId(0))
                .map(|(_, (pos, _))| {
                    Vector2::new(pos.0.x.to_num::<f32>(), pos.0.y.to_num::<f32>())
                })
        });

        if let (Some(p), Some(node)) = (hero_pos, self.hero_node.as_mut()) {
            node.set_position(p);
        }
    }
}
