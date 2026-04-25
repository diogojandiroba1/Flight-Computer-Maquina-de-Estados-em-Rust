pub mod states;
pub mod rocket;
pub mod events; 
pub mod sensors;
pub mod errors;

use std::fs::OpenOptions;
use std::io::Write;
use chrono::Local;
use rocket::Rocket;
use states::PreLaunch;
use std::sync::{mpsc, Arc, Mutex}; 
use std::thread;
use std::time::Duration;
use crate::events::{FlightEvent, FlightCommand};
use crate::rocket::RocketState; 
use crate::sensors::{Sensor, Altimeter};

fn main() {
    let arquivo_log = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true) // Apaga o log antigo a cada voo novo
        .open("flight_log.txt")
        .unwrap();

    env_logger::Builder::new()
        .target(env_logger::Target::Pipe(Box::new(arquivo_log)))
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] [{}] {}",
                Local::now().format("%H:%M:%S%.3f"), // Timestamp com milissegundos
                record.level(),
                record.args()
            )
        })
        .filter(None, log::LevelFilter::Info)
        .init();

    let (tx, rx) = mpsc::channel::<FlightEvent>();
    let tx_telemetry = tx.clone();

    let foguete_inicial = RocketState::PreLaunch(Rocket::<PreLaunch>::new());
    let estado_global = Arc::new(Mutex::new(Some(foguete_inicial)));

    let monitor_arc = Arc::clone(&estado_global);


    // Thread que vê o estado do foguete
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));

            let lock = monitor_arc.lock().unwrap();
            
            if let Some(estado) = lock.as_ref() {
                match estado {
                    RocketState::PreLaunch(_) => println!("[Monitor] Foguete no Pad."),
                    RocketState::Ignition(_) => println!("[Monitor] Motores ligados!"),
                    RocketState::Aborted(_) => {
                        println!("[Monitor] SISTEMA ABORTADO!");
                        break;
                    },
                    _ => println!("[Monitor] Em voo..."),
                }
            }
        }
    });
    
    thread::spawn(move || {

        let mut altimeter = Altimeter::new();

        thread::sleep(Duration::from_secs(2));
        tx_telemetry.send(FlightEvent::Command(FlightCommand::Ignite)).unwrap();

        loop {
            thread::sleep(Duration::from_secs(1));
            
            let alt_reading = altimeter.read();
            
            tx_telemetry.send(FlightEvent::Telemetry { 
                altitude: alt_reading, 
                velocity: 340.0 
            }).unwrap();

            if alt_reading > 4000.0 && alt_reading < 4500.0 {
                tx_telemetry.send(FlightEvent::SubsystemFault(
                    crate::errors::FlightError::EngineOverpressure(5200.5)
                )).unwrap();
            }
        }
    });

    log::info!("Flight Computer iniciado. Aguardando eventos...");

    for event in rx {

        let mut lock = estado_global.lock().unwrap();
        
        // "Rouba" o foguete do Mutex temporariamente
        let mut estado_atual = lock.take().expect("Foguete desapareceu da memória!");

        // Processa o evento
            
            estado_atual = match event {

            FlightEvent::Telemetry { altitude, velocity } => {
                log::info!("Altitude: {:.2}m | Vel: {}m/s", altitude, velocity);

                match estado_atual {
                    RocketState::Ignition(foguete) if altitude > 1000.0 => {
                        RocketState::MaxQ(foguete.init_ascent())
                    }
                    RocketState::MaxQ(foguete) if altitude > 3000.0 => {
                        RocketState::MECO(foguete.final_ascent())
                    }
                    RocketState::MECO(foguete) if altitude > 5000.0 => {
                        RocketState::Separation(foguete.separate_stage())
                    }
                    RocketState::Separation(foguete) if altitude > 6000.0 => {
                        RocketState::Orbit(foguete.orbit_insertion())
                    }
                    _ => estado_atual,
                }
            }


            FlightEvent::Command(FlightCommand::Ignite) => {
                if let RocketState::PreLaunch(foguete) = estado_atual {
                    RocketState::Ignition(foguete.ignite())
                } else {
                    log::warn!("Comando Ignite ignorado: Estado inválido.");
                    estado_atual
                }
            }


            FlightEvent::SubsystemFault(erro_critico) => {
                log::error!("FALHA DETECTADA: {}", erro_critico);
                
                use crate::rocket::Abortable; // Garante que a trait está disponível
                
                match estado_atual {
                    RocketState::PreLaunch(_) => {
                        log::error!("Aborto no Pad! Desligando sistemas.");
                        // PreLaunch não tem trait Abortable no seu código atual, 
                        // então só retornamos o mesmo estado ou criamos uma lógica.
                        estado_atual 
                    }
                    RocketState::Ignition(foguete) => RocketState::Aborted(foguete.abort()),
                    RocketState::MaxQ(foguete) => RocketState::Aborted(foguete.abort()),
                    RocketState::MECO(foguete) => RocketState::Aborted(foguete.abort()),
                    _ => {
                        log::warn!("Muito tarde para abortar! Foguete já em separação/órbita.");
                        estado_atual
                    }
                }
            }


            _ => estado_atual,
        };
        
        // Devolve o foguete atualizado para dentro do Mutex
        *lock = Some(estado_atual);
        
        // Se abortou, quebra o loop
        if let Some(RocketState::Aborted(_)) = lock.as_ref() {
            log::error!("Voo encerrado devido a aborto.");
            break;
        }
    }
}