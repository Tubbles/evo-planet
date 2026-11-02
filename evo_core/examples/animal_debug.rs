//! Founder energy budget check: are the first grazers finding food?
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
fn main() {
    let mut w = EcoWorld::new(PlanetParams::earth(), 400, 7);
    for day in 0..=120u32 {
        if day % 20 == 0 {
            let n = w.animals.len();
            let alive: Vec<String> = w.animals.iter().take(6).map(|a| {
                let pb: f32 = w.plants.iter().filter(|p| p.patch == a.patch).map(|p| p.biomass).sum();
                let np = w.plants.iter().filter(|p| p.patch == a.patch).count();
                format!("[m {:.1}kg e {:+.2} ws {:.2} ls {:.2} {} patch plants {} ({:.2} kg) tol? water={}]",
                    a.traits.mass, a.energy, a.traits.water_speed, a.traits.land_speed,
                    if a.traits.measured_water { "W" } else { "w" }, np, pb, w.patches[a.patch].water)
            }).collect();
            let pb: f32 = w.plants.iter().map(|p| p.biomass).sum();
            println!("day {day:>3}: animals {n:>3} births {} deaths {} | plants {} ({:.1} kg, mean {:.3}) | pending {}", w.stats.births, w.stats.deaths, w.plants.len(), pb, pb / w.plants.len().max(1) as f32, w.stats.pending_evals);
            for s in alive { println!("   {s}"); }
        }
        w.measure_pending(4);
        w.tick();
    }
}
