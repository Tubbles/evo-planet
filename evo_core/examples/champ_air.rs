use evo_core::decode::decode;
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
fn main() {
    let path = std::env::args().nth(1).expect("genome json path");
    let g = Genome::from_json(&std::fs::read_to_string(path).unwrap()).unwrap();
    let ph = decode(&g);
    let cfg = SimConfig { episode_time: 3.0, ..Default::default() };
    let r = CreatureSim::run_episode(ph.clone(), &PlanetParams::earth(), Environment::Air, cfg);
    println!("mass {:.2} kg area/mass {:.3} | air distance {:.2} m airborne {:.2} | air speed {:.2} m/s", ph.total_mass, ph.area_per_mass(), r.air_distance, r.airborne_frac, if r.air_distance > 0.5 { r.air_distance / 3.0 } else { 0.0 });
}
