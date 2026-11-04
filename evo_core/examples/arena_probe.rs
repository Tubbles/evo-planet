//! What does the arena say about real pairs? Takes the bodies an ecology has
//! produced, fights the most common types against each other in their
//! medium, and prints the outcome shares and the cost per pair.
use evo_core::arena::{run_pair, BOUTS};
use evo_core::decode::{body_metrics, decode};
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
use evo_core::sim::Environment;
use std::collections::HashMap;

fn main() {
    let days: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1500);
    let planet = PlanetParams::earth();
    let mut w = EcoWorld::new(planet.clone(), 400, 4);
    for _ in 0..days {
        w.measure_pending(8);
        w.tick();
    }
    println!("day {}: {} animals, {} lineages", w.day, w.animals.len(), w.stats.animal_lineages);
    // Most common body types with a measured water gait.
    let mut counts: HashMap<String, (usize, usize)> = HashMap::new();
    for (i, a) in w.animals.iter().enumerate() {
        let e = counts.entry(a.key.clone()).or_insert((0, i));
        e.0 += 1;
    }
    let mut top: Vec<(String, usize, usize)> = counts.into_iter().map(|(k, (n, i))| (k, n, i)).collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));
    let top: Vec<(String, usize, usize)> = top.into_iter().take(4).collect();
    for (k, n, i) in &top {
        let a = &w.animals[*i];
        let ph = decode(&a.genome);
        let bm = body_metrics(&ph);
        println!(
            "type {}.. n={} mass {:.3} kg parts {} | water {:.2} m/s turn {:.2} sign {:+.0} | land {:.2} m/s | reach {:.2} mouth {:.4} m² claws {:.2}",
            &k[..12], n, ph.total_mass, ph.parts.len(), a.traits.water_speed, a.traits.turn_water, a.traits.turn_sign_water, a.traits.land_speed, bm.reach, bm.mouth_area, bm.claws
        );
    }
    for (kp, _, ip) in &top {
        for (kq, _, iq) in &top {
            let pp = decode(&w.animals[*ip].genome);
            let pq = decode(&w.animals[*iq].genome);
            let (sp, sq) = (w.animals[*ip].traits.turn_sign_water, w.animals[*iq].traits.turn_sign_water);
            let t0 = std::time::Instant::now();
            let o = run_pair(&pp, &pq, &planet, Environment::Water, (sp, w.animals[*ip].traits.heading_water), (sq, w.animals[*iq].traits.heading_water), 1);
            println!(
                "  {}.. eats {}..: eaten {:.0}% fled {:.0}% survived {:.0}% over {} bouts in {:.1} s",
                &kp[..8], &kq[..8], o.eaten * 100.0, o.fled * 100.0, o.survived * 100.0, BOUTS, t0.elapsed().as_secs_f32()
            );
        }
    }
}
