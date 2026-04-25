use rand::Rng;

pub trait Sensor {
    fn read(&mut self) -> f64;
}

pub struct Altimeter {
    pub current_altitude: f64,
}

impl Altimeter {
    pub fn new() -> Self {
        Altimeter { current_altitude: 0.0 }
    }
}

impl Sensor for Altimeter {
    fn read(&mut self) -> f64 {
        let mut rng = rand::thread_rng();
        let noise: f64 = rng.gen_range(-2.5..2.5);
        self.current_altitude += 150.0;
        self.current_altitude + noise
    }
}