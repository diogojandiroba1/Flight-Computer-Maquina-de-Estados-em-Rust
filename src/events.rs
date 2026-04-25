use crate::errors::FlightError;

/// Comandos enviados do event loop principal para os subsistemas.
/// Trafegam no canal `mpsc::Sender<FlightCommand>` do subsistema de propulsão.
#[derive(Debug, Clone)]
pub enum FlightCommand {
    /// Ativa a ignição dos motores.
    Ignite,
    /// Corta os motores (Main Engine Cutoff).
    CutEngine,
    /// Solicita separação de estágio ao subsistema de propulsão.
    /// Por ora não implementado — reservado para Fase 8.
    StageSeparation,
    /// Deploy de payload após inserção em órbita.
    /// Por ora não implementado — reservado para Fase 8.
    DeployPayload,
}

/// Eventos produzidos pelos subsistemas e consumidos pelo event loop principal.
///
/// # Fluxo
/// ```
/// Telemetry Thread ──► FlightEvent::Telemetry ──► main.rs (decisão de fase)
/// Navigation Thread ──► FlightEvent::MaxQDetected ──► main.rs (transição MaxQ)
/// Navigation Thread ──► FlightEvent::SubsystemFault ──► main.rs (aborto)
/// Sensor Thread ──► FlightEvent::Command ──► main.rs (ignição inicial)
/// ```
#[derive(Debug)]
pub enum FlightEvent {
    /// Leitura de telemetria: altitude (m) e velocidade (m/s).
    Telemetry { altitude: f64, velocity: f64 },
    /// Pressão dinâmica máxima detectada pelo subsistema de navegação.
    /// Não é uma falha — é uma transição de fase esperada.
    MaxQDetected,
    /// Falha em subsistema que requer ação do flight computer.
    /// Pode resultar em aborto dependendo da fase atual.
    SubsystemFault(FlightError),
    /// Comando de controle (ex: Ignite disparado pela thread de telemetria no T+2s).
    Command(FlightCommand),
}