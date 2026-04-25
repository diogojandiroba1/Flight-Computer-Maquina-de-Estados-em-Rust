# Flight Computer — Máquina de Estados para Foguete Simulado

**Domínio:** Engenharia de Software Aeroespacial · Sistemas Críticos · Simulação de Física em Tempo Real  
**Linguagem:** Rust 
**Motor de Física:** Rapier2D  

---

## Sumário

1. [Motivação e Escopo](#1-motivação-e-escopo)
2. [Fundamentos de Engenharia](#2-fundamentos-de-engenharia)
3. [Arquitetura do Sistema](#3-arquitetura-do-sistema)
4. [Máquina de Estados Formal](#4-máquina-de-estados-formal)
5. [Modelo Concorrente](#5-modelo-concorrente)
6. [Integração com Rapier2D](#6-integração-com-rapier2d)
7. [Subsistemas](#7-subsistemas)
8. [Estrutura do Projeto](#8-estrutura-do-projeto)
9. [Dependências](#9-dependências)
10. [Compilação e Execução](#10-compilação-e-execução)
11. [Roadmap e Tarefas Pendentes](#11-roadmap-e-tarefas-pendentes)
12. [Decisões de Design e Trade-offs](#12-decisões-de-design-e-trade-offs)

---

## 1. Motivação e Escopo

Sistemas de controle de voo operam em ambientes onde falhas de software têm consequências físicas irreversíveis. O objetivo deste projeto não é apenas simular a trajetória de um foguete, mas demonstrar que técnicas de engenharia de software podem ser usadas como mecanismo formal de verificação de invariantes de sistema, substituindo parte da validação que em sistemas reais seria feita por análise estática externa ou provas formais.

A premissa central é: **estados inválidos não devem ser representáveis em tempo de execução**. Se a transição `PreLaunch → Orbit` é fisicamente impossível sem passar por `Ignition`, o compilador deve rejeitar código que tente executá-la.

O escopo inclui:

- Modelagem de fases de voo como tipos Rust (Typestate Pattern), com transições verificadas em compile-time.
- Arquitetura concorrente baseada em canais (`mpsc`) para isolamento de subsistemas.
- Estado global compartilhado entre threads via `Arc<Mutex<T>>`.
- Sistema de erros tipados com `thiserror` e propagação explícita de falhas.
- Integração com o motor de física Rapier2D para simulação de forças reais (empuxo, gravidade, arrasto atmosférico).

O projeto **não** tem como objetivo ser um simulador de missão completo. É uma demonstração de arquitetura de software para sistemas de tempo real com restrições de segurança.

---

## 2. Fundamentos de Engenharia

### 2.1 Typestate Pattern

O Typestate Pattern codifica o estado de um objeto no seu tipo estático. Em vez de um campo `state: Enum` verificado em runtime com `match`, cada estado é um tipo distinto de zero tamanho (Zero-Sized Type, ZST), e os métodos de transição só existem no tipo correto.

```rust
// Estado inválido é impossível de compilar:
let rocket = FlightComputer::<PreLaunch>::new();
rocket.separate();   // ERRO: método não existe em PreLaunch
rocket.ignite();     // OK: retorna FlightComputer::<Ignition>
```

A consequência direta é que o grafo de transições de estados é verificado pelo compilador a cada build. Não há custo em runtime, os ZSTs são eliminados durante a compilação (monomorphization).

### 2.2 Zero-Cost Abstractions

O conceito de abstração de custo zero em Rust significa que a modelagem de domínio rica (tipos, traits, genéricos) não gera overhead de memória ou CPU comparado ao código C equivalente escrito manualmente. Os estados do Typestate Pattern são ZSTs (`struct PreLaunch;`), ocupando 0 bytes. As transições são chamadas de método normais; não há dispatch dinâmico, heap allocation ou overhead de estado.

### 2.3 Sistema de Erros Tipados

O crate `thiserror` é usado para derivar implementações de `std::error::Error` de forma declarativa. Cada ponto de falha do sistema tem um tipo de erro específico:

```rust
#[derive(Debug, thiserror::Error)]
pub enum FlightError {
    #[error("Falha crítica de propulsão: {0}")]
    PropulsionFailure(String),

    #[error("Desvio de rota acima do limiar: {deviation:.2}°")]
    NavigationDeviation { deviation: f32 },

    #[error("Timeout de telemetria após {elapsed_ms}ms")]
    TelemetryTimeout { elapsed_ms: u64 },
}
```

Erros não são ignorados silenciosamente, a propagação via `?` força o tratamento explícito em cada camada do sistema.

### 2.4 Modelo de Memória e Concorrência

A ausência de um garbage collector em Rust e as garantias do borrow checker eliminam data races em tempo de compilação. O estado compartilhado entre threads usa `Arc<Mutex<RocketState>>`:

- `Arc` (Atomic Reference Counted): contagem de referências atômica, permite múltiplos proprietários entre threads.
- `Mutex`: garante acesso exclusivo ao estado interno, prevenindo leituras/escritas concorrentes inconsistentes.

O compilador rejeita código que tente acessar dados protegidos por `Mutex` sem adquirir o lock.

---

## 3. Arquitetura do Sistema

O sistema é organizado em três camadas:

```
┌─────────────────────────────────────────────────────┐
│                     main.rs                         │
│          Event Loop · Orquestração de Fases         │
│          Arc<Mutex<RocketState>> · Canais mpsc       │
└────────────┬──────────────┬──────────────┬──────────┘
             │              │              │
     ┌───────▼──────┐ ┌─────▼──────┐ ┌────▼───────────┐
     │  Propulsion  │ │  Telemetry │ │   Navigation   │
     │  Thread      │ │  Thread    │ │   Thread       │
     │              │ │            │ │                │
     │ Rx: Command  │ │ Tx: Event  │ │ Tx: Event      │
     │ Rapier Force │ │ Rapier Read│ │ Route Check    │
     └──────────────┘ └────────────┘ └────────────────┘
             │              │              │
     ┌───────▼──────────────▼──────────────▼──────────┐
     │                  Rapier2D                       │
     │     PhysicsWorld · RigidBody · ForceEngine      │
     └─────────────────────────────────────────────────┘
```

A comunicação entre o event loop principal e os subsistemas é exclusivamente via canais (`mpsc::Sender<FlightEvent>` / `mpsc::Receiver<FlightCommand>`). Não há chamada direta de função entre threads. Isso garante isolamento de falhas: uma pane no subsistema de propulsão não corrompe o estado do subsistema de navegação.

---

## 4. Máquina de Estados Formal

### 4.1 Grafo de Transições

```
                  ┌─────────────┐
                  │  PreLaunch  │◄── estado inicial
                  └──────┬──────┘
                         │ ignite()
                  ┌──────▼──────┐
              ┌──►│  Ignition   │
              │   └──────┬──────┘
              │          │ init_ascent()
              │   ┌──────▼──────┐
     abort()  │   │    MaxQ     │
              │   └──────┬──────┘
              │          │ final_ascent()
              │   ┌──────▼──────┐
              │   │    MECO     │ (Main Engine Cutoff)
              │   └──────┬──────┘
              │          │ separate_stage()
              │   ┌──────▼──────┐
              │   │ Separation  │
              │   └──────┬──────┘
              │          │ orbit_insertion()
              │   ┌──────▼──────┐
              └───│    Abort    │◄── terminal de falha
                  └─────────────┘
                         
                  ┌─────────────┐
                  │    Orbit    │◄── terminal de sucesso
                  └─────────────┘
```

### 4.2 Implementação dos Estados

Cada fase é um ZST. As transições são métodos que consomem `self` e retornam o novo tipo:

```rust
pub struct PreLaunch;
pub struct Ignition;
pub struct MaxQ;
pub struct MECO;
pub struct Separation;
pub struct Orbit;
pub struct Abort { pub reason: FlightError }

pub struct FlightComputer<S> {
    state: Arc<Mutex<RocketState>>,
    _phase: PhantomData<S>,
}

impl FlightComputer<PreLaunch> {
    pub fn ignite(self) -> Result<FlightComputer<Ignition>, FlightError> {
        // valida condições pré-lançamento, retorna Err se falhar
    }
}

impl FlightComputer<Ignition> {
    pub fn max_q_reached(self) -> FlightComputer<MaxQ> { ... }
    pub fn abort(self, reason: FlightError) -> FlightComputer<Abort> { ... }
}
```

O tipo `PhantomData<S>` é necessário para que o compilador associe o parâmetro de tipo `S` à struct sem armazenar um valor de `S` em memória.

### 4.3 Estado Global de Runtime

Paralelamente ao Typestate (que existe só em compile-time), existe um `RocketState` que carrega os dados físicos atuais do foguete, compartilhado entre threads:

```rust
pub struct RocketState {
    pub altitude_m: f32,
    pub velocity_ms: f32,
    pub acceleration_ms2: f32,
    pub fuel_kg: f32,
    pub thrust_n: f32,
    pub drag_n: f32,
    pub phase: PhaseEnum,        // espelho do Typestate para logging/telemetria
}
```

---

## 5. Modelo Concorrente

### 5.1 Topologia de Canais

```
main.rs
  │
  ├─ tx_command ──────────────► propulsion_thread (rx_command)
  │                                  │
  │                                  └── aplica força no Rapier
  │
  ├─ rx_event ◄───────────────── telemetry_thread (tx_event)
  │                                  │
  │                                  └── lê Rapier, emite Telemetry
  │
  └─ rx_event ◄───────────────── navigation_thread (tx_event)
                                     │
                                     └── analisa trajetória, emite Warning/Abort
```

### 5.2 Tipos de Mensagem

```rust
pub enum FlightCommand {
    Ignite,
    CutEngine,
    Abort(FlightError),
}

pub enum FlightEvent {
    Telemetry(TelemetryData),
    MaxQDetected,
    MECOConfirmed,
    OrbitInsertionReady,
    AbortRequired(FlightError),
}

pub struct TelemetryData {
    pub timestamp_ms: u64,
    pub altitude_m: f32,
    pub velocity_ms: f32,
    pub fuel_kg: f32,
}
```

### 5.3 Tratamento de Contenção

O `Mutex<RocketState>` pode causar bloqueio se uma thread mantiver o lock por muito tempo. A disciplina adotada é: **adquirir o lock, ler/escrever, soltar imediatamente**. Nenhuma operação bloqueante (I/O, sleep, cálculo pesado) deve ocorrer dentro do escopo do lock.

---

## 6. Integração com Rapier2D

### 6.1 Modelo Físico

O foguete é modelado como um `RigidBody` com dinâmica 2D (eixo Y = altitude, eixo X = deriva lateral). As forças aplicadas a cada step de simulação são:

| Força | Direção | Origem |
|-------|---------|--------|
| Gravidade | −Y | `PhysicsWorld::gravity` (9,81 m/s²) |
| Empuxo | +Y | `PropulsionSubsystem` quando em `Ignition`/`MaxQ` |
| Arrasto atmosférico | −Y (oposto à velocidade) | função de densidade do ar × Cd × v² |

O arrasto atmosférico é aproximado pela equação:

```
F_drag = 0.5 × ρ(h) × Cd × A × v²
```

Onde `ρ(h)` é a densidade do ar em função da altitude (modelo ISA simplificado), `Cd` é o coeficiente de arrasto do foguete e `A` é a área da seção transversal.

### 6.2 Loop de Simulação

```
┌─────────────────────────────────────────────────────┐
│                   Simulation Step                   │
│                                                     │
│  1. Telemetry Thread lê posição/velocidade do       │
│     RigidBody no Rapier                             │
│                                                     │
│  2. Telemetry Thread publica FlightEvent::Telemetry │
│     no canal mpsc                                   │
│                                                     │
│  3. main.rs consome o evento, atualiza              │
│     Arc<Mutex<RocketState>>, avalia transição       │
│     de fase (ex: atingiu altitude de MaxQ?)         │
│                                                     │
│  4. Se transição ocorre, main.rs envia              │
│     FlightCommand para Propulsion Thread            │
│                                                     │
│  5. Propulsion Thread aplica/remove força no        │
│     RigidBody via rapier2d::RigidBodySet            │
│                                                     │
│  6. physics_world.step() avança a simulação         │
│                                                     │
│  Repetir em ~60Hz (timestep fixo de 16.6ms)         │
└─────────────────────────────────────────────────────┘
```

### 6.3 Configuração do PhysicsWorld

```rust
use rapier2d::prelude::*;

pub struct PhysicsWorld {
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub gravity: Vector<f32>,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: BroadPhase,
    pub narrow_phase: NarrowPhase,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub rocket_handle: RigidBodyHandle,
}

impl PhysicsWorld {
    pub fn new() -> Self {
        let gravity = vector![0.0, -9.81];
        // ... inicialização do pipeline
    }

    pub fn step(&mut self) {
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            // ...
        );
    }

    pub fn apply_thrust(&mut self, force_n: f32) {
        let body = &mut self.rigid_body_set[self.rocket_handle];
        body.add_force(vector![0.0, force_n], true);
    }
}
```

---

## 7. Subsistemas

### 7.1 Propulsion (`subsystems/propulsion.rs`)

Responsabilidades:
- Manter o receptor `rx_command: Receiver<FlightCommand>`.
- Ao receber `FlightCommand::Ignite`, iniciar a aplicação de força no `RigidBody` do Rapier.
- Decrementar `fuel_kg` no `Arc<Mutex<RocketState>>` a cada step com base no consumo específico de impulso (Isp).
- Ao receber `FlightCommand::CutEngine` ou quando `fuel_kg <= 0`, cessar a força e emitir `FlightEvent::MECOConfirmed`.

Modelo de empuxo:

```rust
// Thrust = Isp × g0 × ṁ (vazão mássica de combustível)
fn compute_thrust(isp_s: f32, mass_flow_kgs: f32) -> f32 {
    isp_s * 9.81 * mass_flow_kgs
}
```

### 7.2 Telemetry (`subsystems/telemetry.rs`)

Responsabilidades:
- Ser o **clock da simulação**: a cada step do Rapier, ler posição e velocidade do `RigidBody`.
- Construir `TelemetryData` com timestamp, altitude, velocidade e combustível restante.
- Emitir `FlightEvent::Telemetry(data)` no canal `tx_event`.
- Detectar timeout: se nenhum step ocorrer em `N` ms, emitir `FlightEvent::AbortRequired(FlightError::TelemetryTimeout)`.

O subsistema de telemetria é intencionalmente passivo — ele não toma decisões sobre a missão, apenas coleta e transmite dados.

### 7.3 Navigation (`subsystems/navigation.rs`)

Responsabilidades:
- Manter um perfil de trajetória ideal: conjunto de pares `(altitude_m, velocidade_esperada_ms)`.
- A cada evento `FlightEvent::Telemetry` recebido, comparar velocidade real com esperada para a altitude atual.
- Se o desvio exceder o limiar configurável (`DEVIATION_THRESHOLD_MS`), emitir `FlightEvent::AbortRequired(FlightError::NavigationDeviation { deviation })`.
- Detectar atingimento da altitude de MaxQ (≈ 13.700 m para foguetes reais, configurável) e emitir `FlightEvent::MaxQDetected`.

Perfil de referência (exemplo simplificado):

```rust
const TRAJECTORY_PROFILE: &[(f32, f32)] = &[
    (0.0,     0.0),    // (altitude_m, velocidade_ms)
    (1_000.0, 120.0),
    (5_000.0, 400.0),
    (13_700.0, 900.0), // MaxQ
    (80_000.0, 2_800.0),
];
```

---

## 8. Estrutura do Projeto

```
flight-computer/
├── Cargo.toml
├── Cargo.lock
├── README.md
└── src/
    ├── main.rs                  # Event loop principal, orquestração de fases
    ├── rocket.rs                # RocketState, PhaseEnum, FlightComputer<S>
    ├── events.rs                # FlightEvent, FlightCommand, TelemetryData
    ├── errors.rs                # FlightError (thiserror)
    ├── sensors.rs               # Trait Sensor + implementações mock/rapier
    ├── states/
    │   ├── mod.rs
    │   ├── pre_launch.rs
    │   ├── ignition.rs
    │   ├── max_q.rs
    │   ├── meco.rs
    │   ├── separation.rs
    │   ├── orbit.rs
    │   └── abort.rs
    └── subsystems/
        ├── mod.rs
        ├── propulsion.rs        # Thread de propulsão
        ├── telemetry.rs         # Thread de telemetria + clock da simulação
        ├── navigation.rs        # Thread de navegação e detecção de desvio
        └── physics.rs           # PhysicsWorld (wrapper do Rapier2D)
```

---

## 9. Dependências

```toml
[dependencies]
# Motor de física 2D — dinâmica de corpos rígidos
rapier2d = { version = "0.17", features = ["debug-render"] }

# Erros tipados com derive macro
thiserror = "1.0"

# Logging estruturado
log = "0.4"
env_logger = "0.11"

# Serialização de telemetria (opcional, para exportar dados de missão)
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

Versões fixadas em `Cargo.lock`. Não usar `*` em versões de dependências em sistemas críticos.

---

## 10. Compilação e Execução

```bash
# Compilar em modo release (otimizações habilitadas)
cargo build --release

# Compilar e verificar sem gerar binário (mais rápido para desenvolvimento)
cargo check

# Executar com nível de log configurável
RUST_LOG=info cargo run --release

# Níveis disponíveis: error | warn | info | debug | trace
RUST_LOG=debug cargo run

# Executar testes unitários
cargo test

# Executar testes com output de logs visível
RUST_LOG=debug cargo test -- --nocapture
```

Em Windows (PowerShell):

```powershell
$env:RUST_LOG="info"; cargo run --release
```

---

## 11. Roadmap e Tarefas Pendentes

### Fase 7 — Integração Rapier2D e Implementação dos Subsistemas

- [ ] **`subsystems/physics.rs`**: Implementar `PhysicsWorld::new()`, `PhysicsWorld::step()`, `PhysicsWorld::apply_thrust()`, `PhysicsWorld::get_telemetry()`. Adicionar arrasto atmosférico como função de altitude.

- [ ] **`subsystems/propulsion.rs`**: Extrair lógica de ignição do `main.rs`. Implementar loop de recepção de comandos, cálculo de empuxo baseado em `fuel_kg` e Isp, decrementação de combustível por step.

- [ ] **`subsystems/telemetry.rs`**: Implementar loop de simulação a ~60Hz. Ler dados do `PhysicsWorld` após cada `step()`, construir `TelemetryData`, emitir no canal. Implementar detecção de timeout.

- [ ] **`subsystems/navigation.rs`**: Implementar interpolação no perfil de trajetória ideal. Detectar MaxQ por altitude. Implementar lógica de desvio com limiar configurável.

- [ ] **`Cargo.toml`**: Adicionar `rapier2d` com feature `debug-render` para visualização opcional da trajetória.

- [ ] **`rocket.rs`**: Integrar `RocketState` com o `RigidBodyHandle` do Rapier — altitude e velocidade devem ser lidos do Rapier, não mantidos como campos duplicados.

- [ ] **Testes de integração**: Escrever ao menos um teste que simule a sequência completa `PreLaunch → Ignition → MaxQ → MECO → Separation → Orbit` com dados físicos gerados pelo Rapier.

### Fase 8 — Planejada

- [ ] Exportação de dados de missão em JSON para análise pós-voo.
- [ ] Modelo atmosférico ISA (International Standard Atmosphere) com variação de densidade por altitude.
- [ ] Simulação de falhas injetadas (fuel leak, sensor dropout) para teste do caminho de aborto.

---

*Projeto desenvolvido com foco em demonstração de técnicas de engenharia de software para sistemas críticos. Não certificado para uso em hardware real.*