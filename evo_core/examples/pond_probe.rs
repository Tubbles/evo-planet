//! Plants per cell in a pond of 1 m² cells.
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
fn main() {
    let mut planet = PlanetParams::earth();
    planet.water_coverage = 0.5;
    let mut w = EcoWorld::new_with_area(planet, 200, 4, 1.0);
    for day in 0..=400 {
        if day % 100 == 0 {
            let mut per = vec![0usize; w.patches.len()];
            for p in &w.plants { per[p.patch] += 1; }
            let seeds_blocked = w.stats.births;
            let big = w.plants.iter().map(|p| p.biomass).fold(0.0f32, f32::max);
            let e = w.plants.iter().map(|p| p.energy).fold(f32::MIN, f32::max);
            println!("day {day}: plants {} max per cell {} births so far {} | biggest {:.2e} kg, max energy {:.2e}, seed mass of first {:.2e}, repro frac {:.2}", w.plants.len(), per.iter().max().unwrap(), seeds_blocked, big, e, w.plants[0].genome.seed_mass, w.plants[0].genome.repro_frac);
        }
        w.tick();
    }
}
