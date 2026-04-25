use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::events::FlightEvent;
use crate::sensors::{Sensor, Altimeter};
use crate::subsystems::propulsion::PropulsionState;

/// Frequência de amostragem da telemetria: 1Hz.
/// Em sistemas reais seria 10–100Hz dependendo da fase de voo.
const TELEMETRY_RATE_MS: u64 = 1_000;

/// Timeout de watchdog: se nenhuma leitura de sensor ocorrer nesse intervalo,
/// emite `FlightEvent::SubsystemFault` com `FlightError::LossOfSignal`.
const WATCHDOG_TIMEOUT_MS: u64 = 5_000;

/// Spawna a thread de telemetria.
///
/// Esta thread é o **clock da simulação** — ela determina o ritmo em que o
/// event loop principal recebe dados e toma decisões de transição de fase.
///
/// # Responsabilidades
/// 1. Ler o `Altimeter` a cada tick.
/// 2. Ler `fuel_kg` e `current_thrust_n` do `PropulsionState` compartilhado.
/// 3. Calcular velocidade aproximada por diferença de altitude / tempo.
/// 4. Emitir `FlightEvent::Telemetry` no canal principal.
/// 5. Watchdog: se o loop travar por mais de `WATCHDOG_TIMEOUT_MS`, emitir falha.
///
/// # Por que telemetria lê de PropulsionState diretamente?
/// Evita que o event loop principal precise repassar dados de propulsão de volta
/// para a telemetria. O `Arc<Mutex<PropulsionState>>` é a fonte de verdade de
/// dados de motor — telemetria é apenas um consumidor passivo desses dados.
pub fn spawn(
    tx_event: Sender<FlightEvent>,
    prop_state: Arc<Mutex<PropulsionState>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut altimeter = Altimeter::new();
        let mut last_altitude: f64 = 0.0;
        // last_tick começa como None para que o primeiro tick não calcule velocidade
        // com um Δt de microssegundos (que resultaria em velocidades absurdas).
        let mut last_tick: Option<Instant> = None;

        loop {
            let now = Instant::now();
            let elapsed_s = last_tick
                .map(|t| now.duration_since(t).as_secs_f64())
                .unwrap_or(0.0);

            // Leitura do altímetro (com ruído gaussiano simulado).
            let altitude = altimeter.read();

            // No primeiro tick (elapsed_s == 0.0), velocidade é indefinida.
            // Reportamos 0.0 para não poluir o canal de navegação com dados inválidos.
            let velocity = if elapsed_s > 0.1 {
                (altitude - last_altitude) / elapsed_s
            } else {
                0.0
            };

            // Lê dados de propulsão sem manter o lock além do necessário.
            let (fuel_kg, thrust_n) = {
                let state = prop_state.lock().unwrap();
                (state.fuel_kg, state.current_thrust_n)
            };

            log::info!(
                "[Telemetry] alt={:.1}m | vel={:.1}m/s | thrust={:.0}N | fuel={:.1}kg",
                altitude, velocity, thrust_n, fuel_kg
            );

            // Falha ao enviar → event loop encerrou. Thread encerra silenciosamente.
            if tx_event
                .send(FlightEvent::Telemetry { altitude, velocity })
                .is_err()
            {
                log::warn!("[Telemetry] Receiver desconectado. Thread encerrando.");
                return;
            }

            last_altitude = altitude;
            last_tick = Some(now);

            let before_sleep = Instant::now();
            thread::sleep(Duration::from_millis(TELEMETRY_RATE_MS));

            // Watchdog: detecta se o sistema operacional atrasou o wakeup além
            // do tolerável. Um atraso > WATCHDOG_TIMEOUT_MS indica sobrecarga
            // severa de sistema ou suspensão do processo — situação anômala.
            let actual_sleep_ms = before_sleep.elapsed().as_millis() as u64;
            if actual_sleep_ms > WATCHDOG_TIMEOUT_MS {
                log::error!("[Telemetry] WATCHDOG — wakeup atrasado {actual_sleep_ms}ms (limite: {WATCHDOG_TIMEOUT_MS}ms).");
                let _ = tx_event.send(FlightEvent::SubsystemFault(
                    crate::errors::FlightError::LossOfSignal(actual_sleep_ms),
                ));
                return;
            }
        }
    })
}