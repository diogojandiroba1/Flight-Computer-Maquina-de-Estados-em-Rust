use crate::rocket::Rocket; // Puxa struct Rocket
use super::{Separation, Orbit}; // Puxa as etiquetas do mod.rs

impl Rocket<Separation>{

pub fn orbit_insertion(self) -> Rocket<Orbit>{
    log::info!("Inserção em orbita");
    self.transition()
}

}