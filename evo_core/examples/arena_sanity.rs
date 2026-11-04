//! Can the arena ever say "eaten"? Saved walkers as predators against a
//! motionless sheet and against a slow small pill, on land.
use evo_core::arena::run_pair;
use evo_core::decode::{body_metrics, decode};
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
fn main() {
    let planet = PlanetParams::earth();
    let mut sheet = Genome::random(11);
    sheet.nodes.truncate(1);
    sheet.nodes[0].connections.clear();
    sheet.nodes[0].flat = 1.0;
    sheet.nodes[0].len = 1.0;
    sheet.nodes[0].r0 = 0.3;
    sheet.nodes[0].r1 = 0.1;
    let mut small = Genome::random(12);
    for n in small.nodes.iter_mut() { n.flat = 0.0; n.len *= 0.2; n.r0 *= 0.2; n.r1 *= 0.2; }
    let ps = decode(&sheet);
    let pq = decode(&small);
    println!("sheet prey: {:.3} kg thickness {:.4} m; small pill prey: {:.3} kg thickness {:.3} m", ps.total_mass, ps.parts[0].thickness, pq.total_mass, pq.parts[0].thickness);
    for path in std::env::args().skip(1) {
        let g: Genome = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let p = decode(&g);
        let env = Environment::Land;
        let r = CreatureSim::run_episode(p.clone(), &planet, env, SimConfig { episode_time: 4.0, ..Default::default() });
        let rs = CreatureSim::run_episode(p.clone(), &planet, env, SimConfig { episode_time: 4.0, steer: 0.6, ..Default::default() });
        let sign = (rs.yaw_rate - r.yaw_rate).signum();
        let bm = body_metrics(&p);
        let o1 = run_pair(&p, &ps, &planet, env, (sign, r.travel_local), (1.0, [0.0, 1.0, 0.0]), 1);
        let o2 = run_pair(&p, &pq, &planet, env, (sign, r.travel_local), (1.0, [0.0, 1.0, 0.0]), 1);
        println!("{path}: {:.1} kg {} parts speed {:.2} m/s turn {:.2} gape {:.3} m mouth {:.4} m² | vs sheet: eaten {:.0}% fled {:.0}% survived {:.0}% | vs small pill: eaten {:.0}% fled {:.0}% survived {:.0}%", p.total_mass, p.parts.len(), r.sustained_speed, (rs.yaw_rate - r.yaw_rate).abs(), p.parts[0].thickness, bm.mouth_area, o1.eaten*100.0, o1.fled*100.0, o1.survived*100.0, o2.eaten*100.0, o2.fled*100.0, o2.survived*100.0);
    }
}
