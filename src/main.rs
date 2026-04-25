pub mod states;
pub mod rocket;
pub mod events;
pub mod sensors;
pub mod errors;
pub mod subsystems; // ADICIONADO: conecta a pasta subsystems/

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::Local;
use rocket::Rocket;
use states::PreLaunch;

use crate::events::{FlightEvent, FlightCommand};
use crate::rocket::RocketState;
use crate::subsystems::propulsion::PropulsionState;

fn main() {
    // --- Inicialização do logger ---
    let arquivo_log = OpenOptions::new()
        .create(true).write(true).truncate(true)
        .open("flight_log.txt")
        .unwrap();

    env_logger::Builder::new()
        .target(env_logger::Target::Pipe(Box::new(arquivo_log)))
        .format(|buf, record| {
            writeln!(buf, "[{}] [{}] {}",
                Local::now().format("%H:%M:%S%.3f"),
                record.level(),
                record.args())
        })
        .filter(None, log::LevelFilter::Info)
        .init();

    // --- Canais ---

    // Canal principal: todos os subsistemas enviam eventos para cá.
    let (tx_main, rx_main) = mpsc::channel::<FlightEvent>();

    // Canal dedicado de comandos para o subsistema de propulsão.
    // Separado do canal principal para não misturar comandos com eventos de telemetria.
    let (tx_command, rx_command) = mpsc::channel::<FlightCommand>();

    // Canal de dados para o subsistema de navegação.
    // main.rs faz forward de (altitude, velocity) de cada evento Telemetry.
    let (tx_nav_data, rx_nav_data) = mpsc::channel::<(f64, f64)>();

    // --- Estado Global ---
    let foguete_inicial = RocketState::PreLaunch(Rocket::<PreLaunch>::new());
    let estado_global = Arc::new(Mutex::new(Some(foguete_inicial)));

    // Estado de propulsão compartilhado entre PropulsionSubsystem e TelemetrySubsystem.
    // Telemetria lê fuel_kg e thrust_n sem precisar de canal adicional.
    let prop_state = Arc::new(Mutex::new(PropulsionState::new(
        549_054.0 * 0.35, // ~35% da massa total é combustível (LOX+RP-1 aproximado)
    )));

    // --- Spawn dos Subsistemas ---

    let _propulsion_handle = subsystems::propulsion::spawn(
        rx_command,
        Arc::clone(&prop_state),
    );

    let _telemetry_handle = subsystems::telemetry::spawn(
        tx_main.clone(),
        Arc::clone(&prop_state),
    );

    let _navigation_handle = subsystems::navigation::spawn(
        rx_nav_data,
        tx_main.clone(),
    );

    // Thread de ignição: dispara o comando Ignite após T+2s.
    // Em uma missão real, este seria o sequenciador de lançamento (launch sequencer).
    {
        let tx_ignite = tx_main.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            log::info!("[Sequencer] T+2s — enviando comando de ignição.");
            let _ = tx_ignite.send(FlightEvent::Command(FlightCommand::Ignite));
        });
    }

    // Thread de monitor: exibe o estado atual sem bloquear o event loop.
    // CORREÇÃO: não faz mais lock dentro de sleep — apenas lê e solta o lock.
    {
        let monitor_arc = Arc::clone(&estado_global);
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(1));
                // Lock adquirido, lido e solto imediatamente — sem sleep dentro do lock.
                let fase = {
                    let lock = monitor_arc.lock().unwrap();
                    match lock.as_ref() {
                        Some(RocketState::PreLaunch(_))  => "Pre-Launch",
                        Some(RocketState::Ignition(_))   => "Ignition",
                        Some(RocketState::MaxQ(_))       => "Max-Q",
                        Some(RocketState::MECO(_))       => "MECO",
                        Some(RocketState::Separation(_)) => "Separation",
                        Some(RocketState::Orbit(_))      => "Orbit",
                        Some(RocketState::Aborted(_))    => "ABORTED",
                        None                             => "UNKNOWN",
                    }
                };
                println!("[Monitor] Fase atual: {fase}");
                if fase == "ABORTED" || fase == "Orbit" {
                    break;
                }
            }
        });
    }

    log::info!("Flight Computer online. Aguardando eventos...");

    // --- Event Loop Principal ---
    for event in &rx_main {
        let mut lock = estado_global.lock().unwrap();
        let estado_atual = lock.take().expect("Estado global corrompido.");

        let novo_estado = match event {

            FlightEvent::Telemetry { altitude, velocity } => {
                log::info!("[FC] Telemetria — alt={altitude:.1}m | vel={velocity:.1}m/s");

                // Forward para navegação APENAS durante fases de voo ativo.
                // Durante PreLaunch, o altímetro já lê valores não-zero (ruído + rampa),
                // mas o perfil de trajetória só é válido após ignição. Enviar dados
                // de nav antes da ignição garante falsos positivos de desvio.
                let em_voo = matches!(
                    &estado_atual,
                    RocketState::Ignition(_)
                    | RocketState::MaxQ(_)
                    | RocketState::MECO(_)
                    | RocketState::Separation(_)
                );
                if em_voo {
                    let _ = tx_nav_data.send((altitude, velocity));
                }

                // Máquina de estados: transições por altitude.
                // NOTA: estas altitudes são placeholders para a simulação sem Rapier.
                // Com Rapier2D (Fase 7), os limiares seriam baseados em pressão
                // dinâmica real, delta-v acumulado e parâmetros orbitais.
                match estado_atual {
                    RocketState::Ignition(f) if altitude > 1_000.0 => {
                        log::info!("[FC] Transição: Ignition → MaxQ");
                        RocketState::MaxQ(f.init_ascent())
                    }
                    RocketState::MaxQ(f) if altitude > 3_000.0 => {
                        log::info!("[FC] Transição: MaxQ → MECO");
                        let _ = tx_command.send(FlightCommand::CutEngine);
                        RocketState::MECO(f.final_ascent())
                    }
                    RocketState::MECO(f) if altitude > 5_000.0 => {
                        log::info!("[FC] Transição: MECO → Separation");
                        RocketState::Separation(f.separate_stage())
                    }
                    RocketState::Separation(f) if altitude > 6_000.0 => {
                        log::info!("[FC] Transição: Separation → Orbit");
                        RocketState::Orbit(f.orbit_insertion())
                    }
                    other => other,
                }
            }

            // MaxQDetected vem do subsistema de navegação.
            // Por ora apenas loga — em Fase 8 pode ajustar o perfil de empuxo.
            FlightEvent::MaxQDetected => {
                log::warn!("[FC] MaxQ confirmado pelo subsistema de navegação.");
                estado_atual
            }

            FlightEvent::Command(FlightCommand::Ignite) => {
                match estado_atual {
                    RocketState::PreLaunch(f) => {
                        log::info!("[FC] Ignição confirmada.");
                        // Repassa o comando para o subsistema de propulsão.
                        let _ = tx_command.send(FlightCommand::Ignite);
                        RocketState::Ignition(f.ignite())
                    }
                    other => {
                        log::warn!("[FC] Ignite ignorado — estado inválido.");
                        other
                    }
                }
            }

            FlightEvent::SubsystemFault(erro) => {
                log::error!("[FC] FALHA DE SUBSISTEMA: {erro}");

                use crate::rocket::Abortable;
                match estado_atual {
                    RocketState::Ignition(f)  => RocketState::Aborted(f.abort()),
                    RocketState::MaxQ(f)      => RocketState::Aborted(f.abort()),
                    RocketState::MECO(f)      => RocketState::Aborted(f.abort()),
                    RocketState::PreLaunch(_) => {
                        log::error!("[FC] Aborto no pad.");
                        estado_atual
                    }
                    other => {
                        log::warn!("[FC] Falha recebida mas fase não é abortável ({:?}).",
                            std::mem::discriminant(&other));
                        other
                    }
                }
            }

            _ => estado_atual,
        };

        let terminal = matches!(
            &novo_estado,
            RocketState::Aborted(_) | RocketState::Orbit(_)
        );

        *lock = Some(novo_estado);
        drop(lock); // Libera o lock antes de qualquer operação extra.

        if terminal {
            log::info!("[FC] Estado terminal atingido. Encerrando event loop.");
            break;
        }
    }

    log::info!("[FC] Flight Computer encerrado.");
}