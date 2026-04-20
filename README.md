Flight Computer — Máquina de Estados em Rust

> Projeto de aprendizado de Rust aplicado à engenharia de software aeroespacial,
> com foco em sistemas críticos e modelagem formal de estados.

---

## 🎯 Objetivo

Construir um **Flight Computer simulado** para um foguete, modelando cada fase do voo
como um estado formal no sistema de tipos do Rust. O sistema vai gerenciar transições de estado,
detectar falhas, e simular subsistemas paralelos comunicando entre si — exatamente como
sistemas críticos reais são projetados na indústria aeroespacial.

---

## 🚀 Fases do Voo que vou modelar

```
PreLaunch → Ignition → MaxQ → MECO → Separation → Orbit
                ↓          ↓       ↓
             Abort       Abort   Abort
```

Cada estado carrega seus próprios dados e só permite transições válidas.
Uma tentativa de transição inválida vai causar **erro em tempo de compilação** — não em runtime.

---

## 🦀 O que usarei em Rust

### 1. Typestate Pattern
Vou usar o sistema de tipos do Rust para **tornar estados inválidos impossíveis de compilar**.
Em vez de checar estados em runtime com `if`, o compilador vai rejeitar código que tente,
por exemplo, acionar a separação de estágio antes do MECO.

```rust
// Isso não vai nem compilar:
let rocket = Rocket::<PreLaunch>::new();
rocket.separate_stage(); // ❌ erro de compilação — separação só existe no estado MECO
```

### 2. Enums com dados
Vou modelar eventos, falhas e telemetria como enums ricos, onde cada variante
carrega exatamente os dados que fazem sentido para aquela situação.

```rust
enum FlightEvent {
    SensorReading { altitude: f64, velocity: f64, timestamp: u64 },
    FaultDetected { subsystem: Subsystem, severity: Severity },
    StageCompleted { stage: FlightStage, elapsed_ms: u64 },
}
```

### 3. Concorrência com Channels
Vou simular subsistemas independentes (propulsão, telemetria, navegação) rodando
em threads separadas e se comunicando via `mpsc::channel` — como processos
reais de um computador de voo.

```rust
// Thread de telemetria enviando dados para o flight computer
let (tx, rx) = mpsc::channel::<FlightEvent>();
thread::spawn(move || {
    tx.send(FlightEvent::SensorReading { ... }).unwrap();
});
```

### 4. Arc<Mutex<>> para estado compartilhado
O estado global do foguete vai ser compartilhado entre threads com segurança
usando `Arc<Mutex<>>` — o Rust vai garantir em tempo de compilação que não
haverá race conditions.

### 5. Traits para abstração de sensores
Vou criar uma trait `Sensor` genérica para que qualquer tipo de sensor
(altímetro, acelerômetro, GPS) possa ser plugado no sistema sem alterar o código principal.

### 6. Result e tratamento de falhas
Todo evento crítico vai retornar `Result<T, FlightError>`, forçando o tratamento
explícito de cada possível falha — uma prática obrigatória em sistemas de missão crítica.

---

## 🗂️ Estrutura do projeto que vou construir

```
flight-computer/
├── src/
│   ├── main.rs              # Ponto de entrada e loop principal
│   ├── states/
│   │   ├── mod.rs           # Declaração dos estados
│   │   ├── pre_launch.rs
│   │   ├── ignition.rs
│   │   ├── max_q.rs
│   │   ├── meco.rs
│   │   ├── separation.rs
│   │   └── orbit.rs
│   ├── subsystems/
│   │   ├── propulsion.rs    # Thread de propulsão
│   │   ├── telemetry.rs     # Thread de telemetria
│   │   └── navigation.rs    # Thread de navegação
│   ├── events.rs            # Enum de eventos do voo
│   ├── errors.rs            # Enum de falhas críticas
│   └── sensors.rs           # Trait genérica de sensores
├── tests/
│   └── state_transitions.rs # Testes de transições válidas e inválidas
└── Cargo.toml
```

---

## 📦 Dependências que vou usar

| Crate | Para que serve |
|---|---|
| `rand` | Gerar leituras simuladas de sensores |
| `chrono` | Timestamps precisos de telemetria |
| `thiserror` | Derivar erros tipados de forma ergonômica |
| `log` + `env_logger` | Sistema de log estruturado por subsistema |

---

## 🧪 Como vou testar

Vou escrever testes que garantem:
- Transições válidas funcionam corretamente
- Transições inválidas são rejeitadas pelo compilador
- Falhas em subsistemas ativam o modo de abort
- O log de telemetria registra todos os eventos em ordem

---

## 📈 Progressão do projeto

```
Fase 1 — Modelar os estados com typestate pattern
    ↓
Fase 2 — Implementar transições com validação em compile time
    ↓
Fase 3 — Criar subsistemas como threads com channels
    ↓
Fase 4 — Adicionar Arc<Mutex<>> para estado compartilhado
    ↓
Fase 5 — Implementar trait de sensores e injetar dados simulados
    ↓
Fase 6 — Sistema de abort automático por falha de subsistema
    ↓
Fase 7 — Log completo de telemetria em arquivo
```

---

## 📚 Referências

- [The Rust Programming Language](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- *Digital Apollo* — David Mindell
- *Safeware: System Safety and Computers* — Nancy Leveson
- [AeroRust Community](https://aerorust.org)
- [MAVLink Protocol](https://mavlink.io)

---

> *"Software is never finished, only abandoned — except in aerospace, where it has to work the first time."*
