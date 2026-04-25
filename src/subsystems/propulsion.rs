use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::events::FlightCommand;

/// Parâmetros físicos do motor Merlin 1D (referência: Falcon 9 bloco 5).
/// Valores aproximados para fins de simulação.
const THRUST_VACUUM_N: f64 = 934_000.0; // Empuxo no vácuo por motor (N)
const ISP_VACUUM_S: f64 = 348.0;        // Impulso específico no vácuo (s)
const G0: f64 = 9.80665;               // Aceleração gravitacional padrão (m/s²)

// Vazão mássica derivada: ṁ = F / (Isp × g0)
const MASS_FLOW_KGS: f64 = THRUST_VACUUM_N / (ISP_VACUUM_S * G0);

/// Estado interno do subsistema de propulsão.
/// Isolado do estado global — só é exposto via canal ou Arc.
pub struct PropulsionState {
    pub thrust_active: bool,
    pub current_thrust_n: f64,
    pub fuel_kg: f64,
}

impl PropulsionState {
    pub fn new(initial_fuel_kg: f64) -> Self {
        PropulsionState {
            thrust_active: false,
            current_thrust_n: 0.0,
            fuel_kg: initial_fuel_kg,
        }
    }
}

/// Spawna a thread de propulsão.
///
/// # Parâmetros
/// - `rx_command`: canal de recepção de `FlightCommand` vindo do event loop principal.
/// - `prop_state`: estado de propulsão compartilhado via `Arc<Mutex<>>` para que o
///   subsistema de telemetria possa ler `fuel_kg` e `current_thrust_n` sem acoplamento direto.
///
/// # Comportamento
/// - Ao receber `FlightCommand::Ignite`: ativa o empuxo.
/// - Ao receber `FlightCommand::CutEngine`: desativa o empuxo (MECO simulado).
/// - Loop interno decrementa combustível a cada tick enquanto o motor está ativo.
/// - Se `fuel_kg <= 0`, desativa o motor automaticamente (MECO forçado por esgotamento).
pub fn spawn(
    rx_command: Receiver<FlightCommand>,
    prop_state: Arc<Mutex<PropulsionState>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        // Tick de atualização do motor: 100ms → simula loop de controle a 10Hz.
        // Em hardware real seria orientado por interrupção de timer, não sleep.
        const TICK_MS: u64 = 100;
        const TICK_S: f64 = TICK_MS as f64 / 1000.0;

        loop {
            // Drena todos os comandos pendentes sem bloquear.
            // `try_recv` retorna `Err(TryRecvError::Empty)` se não há comandos,
            // e `Err(TryRecvError::Disconnected)` se o sender foi dropado (shutdown).
            loop {
                match rx_command.try_recv() {
                    Ok(FlightCommand::Ignite) => {
                        let mut state = prop_state.lock().unwrap();
                        state.thrust_active = true;
                        state.current_thrust_n = THRUST_VACUUM_N;
                        log::info!(
                            "[Propulsion] Motor ativado — empuxo: {:.0}N | combustível: {:.1}kg",
                            state.current_thrust_n,
                            state.fuel_kg
                        );
                    }
                    Ok(FlightCommand::CutEngine) => {
                        let mut state = prop_state.lock().unwrap();
                        state.thrust_active = false;
                        state.current_thrust_n = 0.0;
                        log::info!("[Propulsion] Motor desligado (MECO).");
                    }
                    // StageSeparation e DeployPayload não são responsabilidade de propulsão.
                    Ok(_) => {}
                    // Canal vazio: sai do loop de comandos, vai atualizar física.
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    // Sender dropado: encerra a thread de propulsão.
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        log::warn!("[Propulsion] Canal de comandos fechado. Thread encerrando.");
                        return;
                    }
                }
            }

            // Atualiza o consumo de combustível se o motor está ativo.
            {
                let mut state = prop_state.lock().unwrap();
                if state.thrust_active {
                    let consumed = MASS_FLOW_KGS * TICK_S;
                    state.fuel_kg -= consumed;

                    if state.fuel_kg <= 0.0 {
                        state.fuel_kg = 0.0;
                        state.thrust_active = false;
                        state.current_thrust_n = 0.0;
                        // MECO forçado por esgotamento de combustível.
                        // O event loop principal detecta fuel=0 via telemetria.
                        log::warn!("[Propulsion] Combustível esgotado — MECO automático.");
                    }
                }
            }

            thread::sleep(Duration::from_millis(TICK_MS));
        }
    })
}