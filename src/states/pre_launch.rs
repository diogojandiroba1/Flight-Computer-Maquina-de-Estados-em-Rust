use crate::rocket::Rocket; // Puxa struct Rocket
use super::{PreLaunch, Ignition}; // Puxa as etiquetas do mod.rs
use std::marker::PhantomData;

//Acessado apenas quando estiver em Pré Lançamento
impl Rocket<PreLaunch>{
    
    //Inicia o pré lançamento do foguete
    pub fn new() -> Self{
        Rocket{
            mass_kg: 549_054.0,
            fuel_level: 1.0,
            altitude_m: 0.0,
            velocity_ms: 0.0,
            _state: PhantomData,
        }
    }

    // Faz ignição e a transição para o State
    pub fn ignite(self) -> Rocket<Ignition>{
        log::info!("Ignição iniciada - T+0");
        self.transition()
    }
}
