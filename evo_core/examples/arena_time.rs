//! How long does one arena pair cost? Random bodies in water and on land.
use evo_core::arena::run_pair;
use evo_core::decode::decode;
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::Environment;

fn main() {
    let planet = PlanetParams::earth();
    for (a, b) in [(1u64, 2u64), (3, 4), (5, 6)] {
        let pp = decode(&Genome::random(a));
        let pq = decode(&Genome::random(b));
        for env in [Environment::Water, Environment::Land] {
            let t0 = std::time::Instant::now();
            let o = run_pair(&pp, &pq, &planet, env, (1.0, [0.0, 1.0, 0.0]), (1.0, [0.0, 1.0, 0.0]), 7);
            println!(
                "{:?} {} vs {} parts | eaten {:.0}% fled {:.0}% survived {:.0}% | {:.2} s per pair",
                env, pp.parts.len(), pq.parts.len(), o.eaten * 100.0, o.fled * 100.0, o.survived * 100.0, t0.elapsed().as_secs_f32()
            );
        }
    }
}
