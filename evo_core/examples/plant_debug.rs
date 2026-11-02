//! Plant economy check without animals: do plants grow and seed?
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
fn main() {
    let mut w = EcoWorld::new(PlanetParams::earth(), 300, 7);
    w.animals.clear();
    let mut last_births = 0usize;
    for day in 0..=600u32 {
        if day % 100 == 0 {
            let n = w.plants.len();
            let bio: f32 = w.plants.iter().map(|p| p.biomass).sum();
            let en: f32 = w.plants.iter().map(|p| p.energy).sum();
            let la: f32 = w.plants.iter().map(|p| p.leaf_area()).sum();
            let mut lin: Vec<u32> = w.plants.iter().map(|p| p.lineage).collect();
            lin.sort(); lin.dedup();
            let _seeded = n.saturating_sub(lin.len());
            println!("day {day:>4}: plants {n:>4} | lineages {:>3} | mean biomass {:.4} kg | mean energy {:.4} | mean leaf {:.3} m^2 | land plants {} | new since last {}",
                lin.len(), bio / n.max(1) as f32, en / n.max(1) as f32, la / n.max(1) as f32, w.stats.land_plants, n as i64 - last_births as i64);
            last_births = n;
            if let Some(p) = w.plants.first() {
                let b = p.body();
                println!("   sample: biomass {:.4}/{:.4} target, energy {:.4}, leaf {:.3} m^2 (full {:.3}), light {:.3}, water {:.2}, cap {:.2}, seed {:.4}, age {:.0}/{:.0}",
                    p.biomass, b.target_mass, p.energy, p.leaf_area(), b.leaf_area, p.light_frac, p.water_frac, b.growth_cap, p.genome.seed_mass, p.age, p.genome.lifespan);
            }
        }
        w.tick();
    }
}
