// Declara que os arquivos existem
pub mod pre_launch;
pub mod ignition;
pub mod meco;
pub mod max_q;
pub mod orbit;
pub mod separation;

//(Zero-Sized Types)
pub struct PreLaunch;
pub struct Ignition;
pub struct MaxQ;
pub struct MECO;
pub struct Separation;
pub struct Orbit;
pub struct Aborted;