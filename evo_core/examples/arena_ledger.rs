//! Run a world and print what the arena found: the outcome shares of every
//! pair of lineages that attacked each other, and their distribution.
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;

fn main() {
    let days: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1100);
    let mut w = EcoWorld::new(PlanetParams::earth(), 400, 4);
    for _ in 0..days {
        w.measure_pending(8);
        w.tick();
    }
    let mut outs = w.arena_outcomes();
    outs.sort_by(|a, b| b.3.eaten.partial_cmp(&a.3.eaten).unwrap());
    let fought = outs.iter().filter(|o| o.3.bouts > 0).count();
    let settled = outs.len() - fought;
    println!("day {}: {} animals, {} pairs fought, {} settled without a bout (no mouth, no claws), {} waiting, kills today {}", w.day, w.animals.len(), fought, settled, w.stats.arena_pending, w.stats.kills);
    let n = outs.len().max(1) as f32;
    let (mut e, mut f, mut s) = (0.0, 0.0, 0.0);
    let mut any_eaten = 0;
    for o in &outs {
        e += o.3.eaten / n;
        f += o.3.fled / n;
        s += o.3.survived / n;
        if o.3.eaten > 0.0 {
            any_eaten += 1;
        }
    }
    println!("mean shares: eaten {:.0}% fled {:.0}% survived {:.0}% | pairs with any kill: {}", e * 100.0, f * 100.0, s * 100.0, any_eaten);
    for o in outs.iter().take(12) {
        println!("  lineage {} eats {} in {:?}: eaten {:.0}% fled {:.0}% survived {:.0}%", o.0, o.1, o.2, o.3.eaten * 100.0, o.3.fled * 100.0, o.3.survived * 100.0);
    }
}
