use crate::rocket::Rocket; // Puxa struct Rocket
use super::{Orbit}; // Puxa as etiquetas do mod.rs


impl Rocket<Orbit>{

    pub fn deploy_payload(self){
        log::info!("Fim de linha");
    }

}