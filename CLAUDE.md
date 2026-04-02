# Pixm — Regras de Engenharia (lidas a cada sessão)

## Stack
- Rust (simulação, rede, lógica)
- Godot 4 (render, input, UI — zero lógica de jogo)
- gdext (GDExtension bridge)

## Lei dos arquivos
**Nenhum arquivo ultrapassa 250 linhas.** Se estourou, está fazendo coisa demais — divida.

## Arquitetura obrigatória
- **ECS puro:** entidades = IDs, componentes = dados, sistemas = lógica
- **Clean Arch nas bordas:** network, Godot FFI, input são adaptadores
- **DOD no núcleo:** simulação determinística orientada a dados
- **Event queue síncrona:** eventos coletados e drenados no fim de cada tick — nunca async dentro da sim loop
- **Sem dependência hardcoded entre módulos** — módulos se comunicam via eventos ou traits

## Regras de código

```
SEMPRE:
  - Separar componente (struct de dados pura) de sistema (fn que opera sobre componentes)
  - Manter funções pequenas e focadas (< 30 linhas como alvo)
  - Usar tipos newtype para IDs (EntityId, PlayerId, TickId)
  - fixed-point (I32F32) em toda aritmética da simulação — nunca f32/f64 na sim
  - Documentar decisões não óbvias com comentário inline

NUNCA:
  - Lógica de jogo dentro de sistemas genéricos do engine
  - Tipos hardcoded de entidade (não existe struct Hero no engine, só componentes)
  - Global state (sem static mut, sem lazy_static com estado mutável)
  - Herança — sempre composição
  - f32/f64 na simulação determinística
  - Alocar no hot path (sim tick, net send/recv)
  - Ultrapassar 250 linhas por arquivo
```

## Módulos e responsabilidades

| Módulo | Responsabilidade única |
|---|---|
| `core/` | ECS substrate, event bus, tipos primitivos |
| `sim/` | Simulação determinística (sistemas, componentes) |
| `net/` | Transporte P2P, serialização, DHT |
| `rollback/` | Snapshot buffer, re-simulação |
| `input/` | Coleta e normalização de input |
| `godot_bridge/` | FFI com Godot — sem lógica de jogo |

## Game loop (sempre seguir este design)

```
frame:
  1. collect_inputs()          ← input module
  2. receive_net_packets()     ← net module
  3. predict_missing_inputs()  ← rollback module
  4. simulate_tick()           ← sim module (fixed 60Hz, determinístico)
  5. drain_event_queue()       ← core event bus
  6. save_snapshot()           ← rollback module
  7. send_inputs_to_peers()    ← net module
  8. render(interpolation_alpha) ← godot_bridge (desacoplado da sim)
```

## Netcode
- DHT Kademlia para descoberta (sem servidor central)
- UDP direto entre peers durante partida
- Rollback GGPO-style como netcode principal
- Lockstep híbrido para peers com RTT baixo
- Clock sync obrigatório (Cristian's algorithm, re-sync a cada 30s)
- Input: event-based, não polling — só envia quando há evento

## Antes de codar qualquer feature
1. Identificar em qual módulo pertence
2. Definir os componentes (dados) separados dos sistemas (lógica)
3. Definir quais eventos são emitidos e consumidos
4. Confirmar que não há dependência hardcoded entre módulos
5. Garantir que o arquivo resultante ficará < 250 linhas

## Design como engine reutilizável
O engine não sabe que o jogo é um MOBA. MOBA é configuração do engine.
Nenhum sistema deve conter lógica específica de jogo.
