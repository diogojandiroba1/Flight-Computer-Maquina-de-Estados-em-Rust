use crate::rocket::{Rocket, Abortable}; // Puxa struct Rocket
use super::{Ignition, Aborted, MaxQ}; // Puxa as etiquetas do mod.rs

//Acessado apenas quando estiver em ignition
impl Rocket<Ignition>{

    // transição ignition -> maxq
    pub fn init_ascent(self) -> Rocket<MaxQ>{
        log::info!("Inicio da Ascensão atmosférica");
        self.transition()
    }

}


impl Abortable for Rocket<Ignition> {
    fn abort(self) -> Rocket<Aborted> {
        log::error!("ABORT — fase Ignition");
        self.transition()
    }
}
