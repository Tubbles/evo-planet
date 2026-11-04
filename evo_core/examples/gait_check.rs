//! Land episode of a saved genome: speed, limbs, clearance, mass.
use evo_core::decode::decode;
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
fn main() {
    for path in std::env::args().skip(1) {
        let g: Genome = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let p = decode(&g);
        let r = CreatureSim::run_episode(p.clone(), &PlanetParams::earth(), Environment::Land, SimConfig { episode_time: 4.0, ..Default::default() });
        println!("{path}: parts {} mass {:.1} kg | speed {:.2} m/s limbs {} clearance {:.2} m spin {:.1} COT {:.1} exploded {}", p.parts.len(), p.total_mass, r.sustained_speed, r.stepping_limbs, r.trunk_clearance, r.trunk_spin, r.cost_of_transport, r.exploded);
    }
}
