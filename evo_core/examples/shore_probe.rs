//! Shore test sanity: do bodies register beach progress only under their own power?
use evo_core::decode::decode;
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
fn main() {
    let planet = PlanetParams::earth();
    let mut files: Vec<String> = std::env::args().skip(1).collect();
    if files.is_empty() {
        for seed in 0..6u64 { files.push(format!("random:{seed}")); }
    }
    for f in files {
        let g = if let Some(seed) = f.strip_prefix("random:") { Genome::random(seed.parse().unwrap()) } else { Genome::from_json(&std::fs::read_to_string(&f).unwrap()).unwrap() };
        let ph = decode(&g);
        let mut row = String::new();
        for motors in [true, false] {
            let cfg = SimConfig { episode_time: 6.0, motors, ..Default::default() };
            let mut sim = CreatureSim::new(ph.clone(), &planet, Environment::Shore, cfg);
            let c0 = sim.com();
            while !sim.finished() { sim.step(); }
            let c1 = sim.com();
            let r = sim.result();
            row += &format!(" | motors {}: start ({:.2},{:.2}) end ({:.2},{:.2}) beach {:.2} m sustained {:.3} exploded {}", if motors { "on " } else { "off" }, c0.x, c0.y, c1.x, c1.y, r.shore_progress, r.sustained_speed, r.exploded);
        }
        println!("{f}: parts {} mass {:.2} kg{}", ph.parts.len(), ph.total_mass, row);
    }
}
