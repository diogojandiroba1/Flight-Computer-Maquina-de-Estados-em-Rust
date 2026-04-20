use crate::rocket::{Rocket, Abortable}; // Puxa struct Rocket
use super::{MECO, Separation, Aborted}; // Puxa as etiquetas do mod.rs


// Acessado apenas quando estiver em MECO
impl Rocket<MECO>{

    //Separação do propulsor e cápsula
    pub fn separate_stage(self) -> Rocket<Separation>{
        log::info!("Separação de estágio");
        self.transition()
    }
}


impl Abortable for Rocket<MECO> {
    fn abort(self) -> Rocket<Aborted> {
        log::error!("ABORT — fase Meco");
        self.transition()
    }
}