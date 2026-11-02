//! Re-run a genome in water with motors on and off: speed, spin, control.
use evo_core::decode::decode;
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
fn main() {
    for f in std::env::args().skip(1) {
        let g = Genome::from_json(&std::fs::read_to_string(&f).unwrap()).unwrap();
        let ph = decode(&g);
        let mut row = String::new();
        for motors in [true, false] {
            let cfg = SimConfig { episode_time: 4.0, motors, ..Default::default() };
            let mut sim = CreatureSim::new(ph.clone(), &PlanetParams::earth(), Environment::Water, cfg);
            let (mut ang, mut n) = (0.0f32, 0u32);
            while !sim.finished() { sim.step(); if sim.time > 1.0 { ang += sim.angular_speed(); n += 1; } }
            let r = sim.result();
            row += &format!(" | motors {}: sustained {:.3} m/s spin {:.1} rad/s exploded {}", if motors { "on " } else { "off" }, r.sustained_speed, ang / n.max(1) as f32, r.exploded);
        }
        println!("{}: parts {} mass {:.2}{}", f.rsplit('/').next().unwrap(), ph.parts.len(), ph.total_mass, row);
    }
}
