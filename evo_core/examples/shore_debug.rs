use evo_core::decode::decode;
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
fn main() {
    let f = std::env::args().nth(1).unwrap();
    let g = Genome::from_json(&std::fs::read_to_string(&f).unwrap()).unwrap();
    let ph = decode(&g);
    for p in &ph.parts { println!("part kind {:?} len {:.3} r {:.3} mass {:.3} density {:.0}", p.kind, p.len, p.radius, p.mass, p.density); }
    let cfg = SimConfig { episode_time: 6.0, motors: false, ..Default::default() };
    let mut sim = CreatureSim::new(ph.clone(), &planet_(), Environment::Shore, cfg);
    let mut n = 0;
    while !sim.finished() {
        sim.step();
        n += 1;
        if sim.time > 0.08 || sim.result().exploded {
            for i in 0..ph.parts.len() { let (t, v) = sim.part_state(i); println!("   part {i} pos ({:.3},{:.3},{:.3}) vel ({:.2},{:.2},{:.2})", t.x, t.y, t.z, v.x, v.y, v.z); }
            let c = sim.com();
            println!("t {:.2} com ({:.3},{:.3},{:.3}) lowest {:.3} exploded {}", sim.time, c.x, c.y, c.z, sim.lowest_point(), sim.result().exploded);
            if sim.result().exploded { break; }
        }
    }
}
fn planet_() -> PlanetParams { PlanetParams::earth() }
