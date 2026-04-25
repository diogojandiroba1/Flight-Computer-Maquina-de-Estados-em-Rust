use crate::errors::FlightError;

pub enum FlightCommand {
    Ignite,
    StageSeparation,
    DeployPayload,
}

pub enum FlightEvent {
    Telemetry { altitude: f64, velocity: f64 },
    SubsystemFault(FlightError),
    Command(FlightCommand),
}