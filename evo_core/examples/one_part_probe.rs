//! Does a single capsule register any speed in the water episode?
use evo_core::decode::decode;
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
fn main() {
    let planet = PlanetParams::earth();
    for seed in 0..12u64 {
        let mut g = Genome::random(seed);
        g.nodes.truncate(1);
        g.nodes[0].connections.clear();
        g.root = 0;
        let ph = decode(&g);
        let cfg = SimConfig { episode_time: 6.0, ..Default::default() };
        let w = CreatureSim::run_episode(ph.clone(), &planet, Environment::Water, cfg.clone());
        let l = CreatureSim::run_episode(ph.clone(), &planet, Environment::Land, cfg);
        println!("seed {seed}: parts {} mass {:.3} lung {:.2} | water dist {:.3} m ({:.4} m/s) | land dist {:.3} mean_h {:.3}", ph.parts.len(), ph.total_mass, ph.lung_frac, w.distance, w.distance / 6.0, l.distance, l.mean_height);
    }
}
