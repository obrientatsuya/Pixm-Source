/// ECS substrate — wrapper fino sobre hecs.
///
/// Entidades = IDs puros. Componentes = dados. Sistemas = funções externas.
/// Este módulo não conhece nenhum componente específico do jogo.

pub use hecs::{World, Entity, QueryBorrow};

/// Extensões de conveniência no World.
pub trait WorldExt {
    /// Spawna entidade com bundle de componentes.
    fn spawn_with<B: hecs::DynamicBundle>(&mut self, bundle: B) -> Entity;
    /// Adiciona componente a entidade existente.
    fn add<C: hecs::Component>(&mut self, entity: Entity, component: C);
    /// Remove componente de entidade existente.
    fn remove<C: hecs::Component>(&mut self, entity: Entity);
    /// Lê componente imutavelmente.
    fn get<C: hecs::Component>(&self, entity: Entity) -> Option<hecs::Ref<C>>;
}

impl WorldExt for World {
    fn spawn_with<B: hecs::DynamicBundle>(&mut self, bundle: B) -> Entity {
        self.spawn(bundle)
    }

    fn add<C: hecs::Component>(&mut self, entity: Entity, component: C) {
        self.insert_one(entity, component)
            .unwrap_or_else(|e| tracing::warn!("add component: {e}"));
    }

    fn remove<C: hecs::Component>(&mut self, entity: Entity) {
        if let Err(e) = self.remove_one::<C>(entity) {
            tracing::warn!("remove component: {e}");
        }
    }

    fn get<C: hecs::Component>(&self, entity: Entity) -> Option<hecs::Ref<C>> {
        self.get::<&C>(entity).ok()
    }
}
