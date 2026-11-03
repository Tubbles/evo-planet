//! Algae-only world: does heterotrophy spread among sessile plants, and do motile seeds persist?
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
fn main() {
    let days: u32 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(3000);
    let mut w = EcoWorld::new(PlanetParams::earth(), 600, 4);
    for d in 1..=days {
        w.measure_pending(8);
        w.tick();
        if d % 1000 == 0 || d == days {
            let n = w.plants.len().max(1) as f32;
            let h: Vec<f32> = w.plants.iter().map(|p| p.genome.heterotrophy).collect();
            let m: Vec<f32> = w.plants.iter().map(|p| p.genome.motility).collect();
            let mean = |v: &Vec<f32>| v.iter().sum::<f32>() / n;
            let frac = |v: &Vec<f32>, t: f32| v.iter().filter(|x| **x > t).count() as f32 / n;
            let max = |v: &Vec<f32>| v.iter().cloned().fold(0.0f32, f32::max);
            let dark = w.plants.iter().filter(|p| w.patches[p.patch].water && p.light_frac < 0.0005).count();
            println!(
                "day {d}: plants {} | heterotrophy mean {:.3} max {:.2} | >0.1: {:.1}% >0.3: {:.1}% | motility mean {:.3} max {:.2} >0.3: {:.1}% | plants with ~no light {} | P->A so far {} | animals {} | bacteria {:.1} kg dead {:.1} kg",
                w.plants.len(), mean(&h), max(&h), 100.0 * frac(&h, 0.1), 100.0 * frac(&h, 0.3), mean(&m), max(&m), 100.0 * frac(&m, 0.3), dark, w.stats.plant_to_animal, w.animals.len(),
                0.0f32, w.patches.iter().map(|p| p.dead_matter).sum::<f32>()
            );
        }
    }
}
