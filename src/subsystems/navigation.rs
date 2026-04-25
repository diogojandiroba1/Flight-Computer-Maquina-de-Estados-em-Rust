use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use crate::events::FlightEvent;
use crate::errors::FlightError;

/// Perfil de trajetória de referência: pares (altitude_m, velocidade_esperada_ms).
///
/// Os valores abaixo estão calibrados para o `Altimeter` simulado atual,
/// que incrementa 150m/tick a 1Hz (velocidade constante ~150m/s).
/// Quando Rapier2D for integrado (Fase 7), este perfil será substituído
/// por dados de otimização de trajetória baseados na física real.
///
/// Tolerância de ±30% aplicada para absorver o ruído gaussiano (±2.5m) do sensor.
const TRAJECTORY_PROFILE: &[(f64, f64)] = &[
    (0.0,     0.0),
    (150.0,   150.0),
    (500.0,   150.0),
    (1_000.0, 150.0),
    (2_000.0, 150.0),
    (3_000.0, 150.0),
    (5_000.0, 150.0),
    (8_000.0, 150.0),
    (10_000.0, 150.0),
];

/// Tolerância de desvio: ±30% da velocidade esperada.
/// Valor aumentado em relação ao ideal (±20%) para absorver:
/// - Ruído gaussiano do altímetro simulado (±2.5m por tick)
/// - Imprecisão da derivada numérica de velocidade
/// Com Rapier2D, a velocidade virá diretamente do RigidBody (sem ruído de derivada)
/// e a tolerância poderá ser reduzida para ±15%.
const DEVIATION_TOLERANCE: f64 = 0.30;

/// Altitude a partir da qual MaxQ é detectado (m).
/// Em foguetes reais, MaxQ é determinado pela pressão dinâmica máxima (q = 0.5 × ρ × v²),
/// não só pela altitude. Aqui usamos altitude como proxy para simplificar.
const MAX_Q_ALTITUDE_M: f64 = 8_500.0;

/// Altitude de MECO simulado: motor deve ter sido cortado antes deste ponto.
const MECO_ALTITUDE_M: f64 = 10_000.0;

/// Spawna a thread de navegação.
///
/// # Parâmetros
/// - `rx_nav`: recebe `(altitude, velocity)` via canal dedicado.
///   O event loop principal faz forward dos dados de telemetria para este canal,
///   evitando que navegação precise ler diretamente do `Arc<Mutex<RocketState>>`.
/// - `tx_event`: envia `FlightEvent` de volta ao event loop principal
///   (alertas de desvio, detecção de MaxQ, requisição de aborto).
///
/// # Por que um canal dedicado e não leitura de Arc<Mutex<RocketState>>?
/// O `RocketState` é um enum que embala `Rocket<Phase>`. Para ler altitude
/// dele, precisaríamos de um `match` completo — acoplamento forte com o módulo
/// de estados. O canal de navegação recebe apenas os dados necessários (`f64, f64`),
/// mantendo navegação agnóstica à fase atual do voo.
pub fn spawn(
    rx_nav: Receiver<(f64, f64)>, // (altitude_m, velocity_ms)
    tx_event: Sender<FlightEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut max_q_emitted = false;

        for (altitude, velocity) in &rx_nav {
            // --- Detecção de MaxQ ---
            if !max_q_emitted && altitude >= MAX_Q_ALTITUDE_M {
                log::warn!("[Navigation] MaxQ detectado em {altitude:.0}m.");
                // MaxQ não é falha — é uma transição de fase esperada.
                // Emitimos como evento normal, não como SubsystemFault.
                // O event loop decide se deve transicionar de estado.
                if tx_event.send(FlightEvent::MaxQDetected).is_err() {
                    return;
                }
                max_q_emitted = true;
            }

            // --- Verificação de desvio de trajetória ---
            if let Some(expected_velocity) = interpolate_expected_velocity(altitude) {
                let deviation = (velocity - expected_velocity) / expected_velocity;

                if deviation.abs() > DEVIATION_TOLERANCE {
                    log::error!(
                        "[Navigation] DESVIO DE ROTA — alt={:.0}m | vel_real={:.1}m/s \
                         | vel_esperada={:.1}m/s | desvio={:.1}%",
                        altitude, velocity, expected_velocity, deviation * 100.0
                    );

                    let _ = tx_event.send(FlightEvent::SubsystemFault(
                        FlightError::NavigationDeviation {
                            altitude,
                            actual: velocity,
                            expected: expected_velocity,
                            deviation: deviation * 100.0,
                        },
                    ));
                    // Continua monitorando — o aborto é processado assincronamente.
                }
            }
        }

        // rx_nav foi fechado pelo sender (main.rs encerrou).
        log::info!("[Navigation] Canal de dados encerrado. Thread finalizando.");
    })
}

/// Interpola a velocidade esperada para uma dada altitude usando o perfil de referência.
///
/// Usa interpolação linear entre os dois pontos mais próximos do perfil.
/// Retorna `None` se a altitude estiver fora do intervalo do perfil.
fn interpolate_expected_velocity(altitude: f64) -> Option<f64> {
    if altitude < TRAJECTORY_PROFILE[0].0
        || altitude > TRAJECTORY_PROFILE.last()?.0
    {
        return None;
    }

    // Encontra o segmento que contém a altitude.
    for window in TRAJECTORY_PROFILE.windows(2) {
        let (alt_low, vel_low) = window[0];
        let (alt_high, vel_high) = window[1];

        if altitude >= alt_low && altitude <= alt_high {
            // Fração linear no segmento [alt_low, alt_high].
            let t = (altitude - alt_low) / (alt_high - alt_low);
            return Some(vel_low + t * (vel_high - vel_low));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_midpoint() {
        // Entre (500, 80) e (1000, 150): meio = 750m → esperado = 115m/s
        let result = interpolate_expected_velocity(750.0).unwrap();
        assert!((result - 115.0).abs() < 0.1, "got {result}");
    }

    #[test]
    fn interpolation_exact_point() {
        let result = interpolate_expected_velocity(1_000.0).unwrap();
        assert!((result - 150.0).abs() < 0.1);
    }

    #[test]
    fn interpolation_out_of_range_returns_none() {
        assert!(interpolate_expected_velocity(50_000.0).is_none());
        assert!(interpolate_expected_velocity(-1.0).is_none());
    }
}