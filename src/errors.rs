use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlightError {
    #[error("Pressao critica no motor: {0} psi")]
    EngineOverpressure(f64),
    
    #[error("Perda de telemetria após {0}ms")]
    LossOfSignal(u64),
    
    #[error("Falha no atuador de estagio")]
    ActuatorFailure,
}