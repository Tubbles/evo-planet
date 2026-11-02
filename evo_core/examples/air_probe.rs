//! Can any body register sustained air speed in the 3 s drop episode?
use evo_core::decode::decode;
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
fn main() {
    let planet = PlanetParams::earth();
    let mut winged = 0;
    let mut fliers = 0;
    let mut best = (0.0f32, 0.0f32, 0.0f32);
    for seed in 0..400u64 {
        let g = Genome::random(seed);
        let ph = decode(&g);
        if ph.membrane_area / ph.total_mass.max(1e-3) < 0.02 { continue; }
        winged += 1;
        let cfg = SimConfig { episode_time: 3.0, ..Default::default() };
        let r = CreatureSim::run_episode(ph.clone(), &planet, Environment::Air, cfg);
        if r.air_distance > 0.5 { fliers += 1; }
        if r.air_distance > best.0 { best = (r.air_distance, r.airborne_frac, ph.membrane_area / ph.total_mass); }
    }
    println!("winged {winged}/400 | fliers by criterion {fliers} | best distance {:.2} m airborne {:.2} area/mass {:.3}", best.0, best.1, best.2);
}
