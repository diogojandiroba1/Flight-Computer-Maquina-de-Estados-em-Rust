pub mod states; // Declara a pasta states
pub mod rocket; // Declara o arquivo rocket.rs

use rocket::Rocket;
use states::PreLaunch;

fn main() {

    env_logger::init();

    // Inicia no PreLaunch e vai encadeando as transições
    Rocket::<PreLaunch>::new()
        .ignite()
        .init_ascent()
        .final_ascent()
        .separate_stage()
        .orbit_insertion()
        .deploy_payload();
}