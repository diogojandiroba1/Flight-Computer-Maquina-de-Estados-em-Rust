use std::marker::PhantomData;
use crate::states::*;

//Struct "Pai"
pub struct Rocket<State> {
    pub _state: PhantomData<State>,
    pub mass_kg: f64,
    pub fuel_level: f64,
    pub altitude_m : f64,
    pub velocity_ms: f64,
}

//Acessado por qualquer estado
impl<State>Rocket<State>{

    //Função responsavél por realizar transição de dados entre States
    pub(crate) fn transition<NewState>(self) -> Rocket<NewState>{
        Rocket{
            _state: PhantomData,
            mass_kg: self.mass_kg,
            fuel_level: self.fuel_level,
            altitude_m : self.altitude_m,
            velocity_ms: self.velocity_ms,
        }
    }    
}

// trait que aborta missão
pub trait Abortable {
    fn abort(self) -> Rocket<Aborted>;
}