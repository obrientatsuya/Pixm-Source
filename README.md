# Pixm

MOBA 5v5 2D P2P com foco em duelo 1v1. Sem servidor central.

## Stack

| Camada | Tecnologia |
|---|---|
| Engine / Simulação | Rust (DOD, ECS) |
| Render / UI | Godot 4 (GDExtension) |
| Rede | UDP direto + DHT Kademlia |
| Netcode | Rollback GGPO-style |

## Estrutura

```
net/    → transporte UDP, DHT, clock sync, rollback netcode
game/   → ECS, simulação determinística, bridge com Godot
docs/   → arquitetura, ECS spec, game loop, netcode spec, roadmap
```

## Docs

- [Arquitetura](docs/architecture.md)
- [ECS Spec](docs/ecs.md)
- [Game Loop](docs/game-loop.md)
- [Netcode](docs/netcode.md)
- [Roadmap de Rede](docs/roadmap-net.md)

## Build

```bash
# Rust (net + game)
cargo build

# Godot: abrir game/godot/project.godot com Godot 4.2+
# A extensão Rust é carregada automaticamente via game/godot/pixm.gdextension
```

## Princípios

- ECS puro — entidades são IDs, componentes são dados, sistemas são lógica
- Simulação 100% determinística — fixed-point, RNG seed compartilhado
- Zero servidor central — peers se encontram via DHT
- Engine genérico — o jogo é configuração do engine, não o contrário
- Arquivos nunca ultrapassam 250 linhas
