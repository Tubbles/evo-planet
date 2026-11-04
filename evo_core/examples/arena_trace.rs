//! One bout, step by step: where the predator goes and how close its mouth gets.
use evo_core::arena::ArenaSim;
use evo_core::decode::decode;
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
fn main() {
    let planet = PlanetParams::earth();
    let path = std::env::args().nth(1).unwrap();
    let g: Genome = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let p = decode(&g);
    let mut sheet = Genome::random(11);
    sheet.nodes.truncate(1);
    sheet.nodes[0].connections.clear();
    sheet.nodes[0].flat = 1.0;
    sheet.nodes[0].len = 1.0;
    sheet.nodes[0].r0 = 0.3;
    sheet.nodes[0].r1 = 0.1;
    let ps = decode(&sheet);
    let env = Environment::Land;
    let r = CreatureSim::run_episode(p.clone(), &planet, env, SimConfig { episode_time: 4.0, ..Default::default() });
    let rs = CreatureSim::run_episode(p.clone(), &planet, env, SimConfig { episode_time: 4.0, steer: 0.6, ..Default::default() });
    let sign = (rs.yaw_rate - r.yaw_rate).signum();
    println!("predator: {} parts, speed {:.2}, yaw {:.2} vs steered {:.2}, sign {sign}", p.parts.len(), r.sustained_speed, r.yaw_rate, rs.yaw_rate);
    let mut sim = ArenaSim::new(p, ps, &planet, env, (sign, r.travel_local), (1.0, [0.0, 1.0, 0.0]), 0.0, 0.0);
    let mut next = 0.0;
    while !sim.finished() {
        if sim.time >= next {
            let (pc, qc, m, h, gap) = sim.snapshot();
            println!("t={:.1} dist {:.2} | pred ({:.2},{:.2},{:.2}) heading ({:.2},{:.2}) mouth ({:.2},{:.2},{:.2}) | prey ({:.2},{:.2},{:.2}) | gap {:.3} hold {:.2} bitten {}", sim.time, sim.distance(), pc.x, pc.y, pc.z, h.x, h.z, m.x, m.y, m.z, qc.x, qc.y, qc.z, gap, sim.hold, sim.bitten);
            next += 0.5;
        }
        sim.step();
    }
    println!("result {:?}", sim.result());
}
