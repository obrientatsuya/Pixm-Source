/// GameLoopNode — ponto de entrada Godot. Roda o game loop.
///
/// Godot chama _process() a cada frame. Aqui:
///   - Lê inputs do Godot → coleta no InputCollector
///   - Avança SimWorld (tick fixo 60Hz com accumulator)
///   - Lê posições da sim → atualiza Node2D no Godot (interpolado)
///
/// Zero lógica de jogo aqui — só tradução de fronteira.

use godot::prelude::*;
use crate::sim::world::SimWorld;
use crate::input::collector::InputCollector;
use crate::core::types::Fixed;

const TICK_DUS: f64 = 1_000_000.0 / 60.0; // ~16_667µs por tick
const TICK_DT:  f64 = 1.0 / 60.0;          // segundos por tick

#[derive(GodotClass)]
#[class(base=Node)]
pub struct GameLoopNode {
    sim:         Option<SimWorld>,
    collector:   InputCollector,
    accumulator: f64,    // segundos acumulados para fixed timestep
    base:        Base<Node>,
}

#[godot_api]
impl INode for GameLoopNode {
    fn init(base: Base<Node>) -> Self {
        Self {
            sim:         None,
            collector:   InputCollector::new(),
            accumulator: 0.0,
            base,
        }
    }

    fn ready(&mut self) {
        // Inicializa sim com seed 0 (substituída pela seed da partida no join)
        self.sim = Some(SimWorld::new(0));
        tracing::info!("GameLoopNode pronto");
    }

    fn process(&mut self, delta: f64) {
        let Some(ref mut sim) = self.sim else { return };

        self.accumulator += delta;

        // Executa tantos ticks fixos quanto o accumulator permitir
        while self.accumulator >= TICK_DT {
            let inputs = self.collector.drain();
            sim.run_tick(&inputs);
            self.accumulator -= TICK_DT;
        }

        // Alpha para interpolação visual (0.0 .. 1.0)
        let alpha = Fixed::from_num(self.accumulator / TICK_DT);
        self.update_render(alpha);
    }
}

#[godot_api]
impl GameLoopNode {
    /// Chamado pelo Godot quando o jogador clica no terreno.
    #[func]
    pub fn on_ground_click(&mut self, world_x: f32, world_y: f32) {
        let event = crate::bridge::input_bridge::on_ground_click(
            0, // player_id local (substituir pelo ID real)
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

    /// Inicializa a partida com seed do DHT.
    #[func]
    pub fn start_match(&mut self, rng_seed: i64) {
        self.sim = Some(SimWorld::new(rng_seed as u64));
        self.accumulator = 0.0;
        tracing::info!("partida iniciada com seed {rng_seed}");
    }

    /// Retorna tick atual (para UI/debug).
    #[func]
    pub fn current_tick(&self) -> i64 {
        self.sim.as_ref().map(|s| s.tick as i64).unwrap_or(0)
    }
}

impl GameLoopNode {
    /// Lê posições da sim e atualiza nodes Godot (interpolados).
    fn update_render(&mut self, _alpha: Fixed) {
        // TODO: iterar entidades com Position + um componente de "node path"
        // e atualizar Node2D.position com interpolação linear.
        //
        // Exemplo de estrutura esperada:
        //   for (_, (pos, node_ref)) in sim.world.query::<(&Position, &GodotNodeRef)>() {
        //       if let Some(mut node) = node_ref.try_get() {
        //           node.set_position(Vector2::new(pos.x.to_num(), pos.y.to_num()));
        //       }
        //   }
        //
        // Implementado quando as cenas Godot estiverem criadas.
    }
}
