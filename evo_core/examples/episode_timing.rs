use evo_core::decode::decode;
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
fn main() {
    let planet = PlanetParams::earth();
    let t = std::time::Instant::now();
    let mut n = 0;
    for seed in 0..60u64 {
        let g = Genome::random(seed);
        let ph = decode(&g);
        let cfg = SimConfig { episode_time: 3.0, ..Default::default() };
        let _ = CreatureSim::run_episode(ph.clone(), &planet, Environment::Water, cfg.clone());
        let _ = CreatureSim::run_episode(ph, &planet, Environment::Land, cfg);
        n += 2;
    }
    println!("{} episodes in {:.2} s = {:.1} ms each", n, t.elapsed().as_secs_f32(), 1000.0 * t.elapsed().as_secs_f32() / n as f32);
}
