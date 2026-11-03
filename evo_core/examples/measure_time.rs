//! Time one land measurement (drive + sham + turn episodes) for random bodies.
use evo_core::decode::decode;
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};

fn main() {
    let planet = PlanetParams::earth();
    let mut total = 0.0f64;
    let n = 40;
    let mut parts = 0usize;
    for seed in 0..n {
        let g = Genome::random(seed as u64);
        let pheno = decode(&g);
        parts += pheno.parts.len();
        let t0 = std::time::Instant::now();
        for cfg in [SimConfig { episode_time: 4.0, ..Default::default() }, SimConfig { episode_time: 4.0, motors: false, ..Default::default() }, SimConfig { episode_time: 4.0, steer: 0.6, ..Default::default() }] {
            let _ = CreatureSim::run_episode(pheno.clone(), &planet, Environment::Land, cfg);
        }
        total += t0.elapsed().as_secs_f64();
    }
    println!("{n} bodies, mean parts {:.1}: {:.3} s per body (three land episodes), {:.1} bodies/s on one core", parts as f32 / n as f32, total / n as f64, n as f64 / total);
}
