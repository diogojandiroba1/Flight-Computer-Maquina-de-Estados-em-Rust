use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlightError {
    #[error("Pressao critica no motor: {0:.1} psi")]
    EngineOverpressure(f64),

    #[error("Perda de telemetria apos {0}ms")]
    LossOfSignal(u64),

    #[error("Falha no atuador de estagio")]
    ActuatorFailure,

    #[error("Desvio de trajetoria em {altitude:.0}m: real={actual:.1}m/s esperado={expected:.1}m/s ({deviation:+.1}%)")]
    NavigationDeviation {
        altitude: f64,
        actual: f64,
        expected: f64,
        deviation: f64,
    },
}