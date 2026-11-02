use evo_core::decode::decode;
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
fn main() {
    let f = std::env::args().nth(1).unwrap();
    let g = Genome::from_json(&std::fs::read_to_string(&f).unwrap()).unwrap();
    let ph = decode(&g);
    for (i, p) in ph.parts.iter().enumerate() { println!("part {i} {:?} len {:.3} r {:.3} mass {:.3} joint {:?} limit {:.2} amp {:.2} freq_mult {:.1} max_torque {:.2}", p.kind, p.len, p.radius, p.mass, p.joint, p.limit, p.cpg.amp, p.cpg.freq_mult, p.max_torque); }
    println!("base_freq {:.2}", g.base_freq);
    let cfg = SimConfig { episode_time: 4.0, ..Default::default() };
    let mut sim = CreatureSim::new(ph.clone(), &PlanetParams::earth(), Environment::Water, cfg);
    let mut n = 0;
    while !sim.finished() {
        sim.step(); n += 1;
        if n % 30 == 0 {
            let w0 = sim.part_angvel(0); let w1 = sim.part_angvel(1);
            let (p0, v0) = sim.part_state(0); let (p1, _) = sim.part_state(1);
            println!("t {:.2} root w ({:.1},{:.1},{:.1}) child w ({:.1},{:.1},{:.1}) rel {:.1} | root v ({:.2},{:.2},{:.2}) root y {:.2} child y {:.2}", sim.time, w0.x, w0.y, w0.z, w1.x, w1.y, w1.z, (w1 - w0).norm(), v0.x, v0.y, v0.z, p0.y, p1.y);
        }
    }
}
