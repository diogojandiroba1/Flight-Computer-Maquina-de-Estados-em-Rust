use crate::rocket::{Rocket, Abortable};
use super::{Aborted,MaxQ, MECO};

//Acessado apenas quando estiver em ignition
impl Rocket<MaxQ>{

    // transição maxq -> meco
    pub fn final_ascent(self) -> Rocket<MECO>{
        log::info!("Final da Ascensão atmosférica");
        self.transition()
    }

}


impl Abortable for Rocket<MaxQ> {
    fn abort(self) -> Rocket<Aborted> {
        log::error!("ABORT — fase Max-Q");
        self.transition()
    }
}