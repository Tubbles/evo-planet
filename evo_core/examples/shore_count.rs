//! How much of the land is interior at each resolution and seed.
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
fn main() {
    for &n in &[600usize, 1500, 3000, 6000] {
        for seed in 1..=4u64 {
            let w = EcoWorld::new_with_area(PlanetParams::earth(), n, seed, 100.0);
            let (mut sea, mut shore, mut inland) = (0, 0, 0);
            for p in &w.patches {
                if p.water { sea += 1 } else if p.submersion > 0.0 { shore += 1 } else { inland += 1 }
            }
            println!("{n:>5} cells seed {seed}: sea {sea:>5} shore {shore:>5} inland {inland:>5} ({:.0}% of land)", 100.0 * inland as f32 / (shore + inland).max(1) as f32);
        }
    }
}
