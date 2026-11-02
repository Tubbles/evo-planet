//! Population-level feeding probe: who eats, who starves, and what is in their patch.
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
fn main() {
    let mut w = EcoWorld::new(PlanetParams::earth(), 600, 4);
    for _ in 0..900 {
        w.measure_pending(3);
        w.tick();
    }
    let s = &w.stats;
    println!("day {} animals {} | NPP {:.1} graze {:.2} cost {:.2} | starved {} old {}", s.day, s.animals, s.primary_production, s.grazing_intake, s.metabolic_cost, s.deaths_starved, s.deaths_old);
    let mut rows: Vec<String> = Vec::new();
    for a in &w.animals {
        let plants: Vec<&evo_core::eco::Plant> = w.plants.iter().filter(|p| p.patch == a.patch).collect();
        let pb: f32 = plants.iter().map(|p| p.biomass).sum();
        let acc: f32 = plants.iter().map(|p| (p.biomass - 0.004).max(0.0) * p.body().leaf_mass / p.body().target_mass * 0.5).sum();
        let (ep, _) = a.efficiencies();
        let water = w.patches[a.patch].water;
        rows.push(format!(
            "m {:5.2}/{:5.2} kg | res {:+.3} cap {:.3} | gut {:.3}/{:.3} | intake {:.4} | ws {:.2} ls {:.2} meas {}{} | patch {} plants {} ({:.2} kg, accessible {:.3}) water={} T {:.0}C | ate P {:.2} M {:.2} C {:.2} | eff_plant {:.2}",
            a.mass, a.traits.mass, a.reserve, (0.05 + a.genome.fat_frac) * a.mass, a.gut_plant + a.gut_meat, a.genome.gut_frac * a.mass, a.intake_rate,
            a.traits.water_speed, a.traits.land_speed, if a.traits.measured_water { "W" } else { "w" }, if a.traits.measured_land { "L" } else { "l" },
            a.patch, plants.len(), pb, acc, water, w.patches[a.patch].temp_c, a.ate_plant, a.ate_meat, a.ate_carrion, ep));
    }
    rows.sort();
    for r in rows.iter().take(40) { println!("{r}"); }
    let juv = w.animals.iter().filter(|a| a.mass < 0.7 * a.traits.mass).count();
    let ages: Vec<f32> = w.animals.iter().map(|a| a.age).collect();
    println!("juveniles {} / {} | mean age {:.0} | births {} deaths {}", juv, w.animals.len(), ages.iter().sum::<f32>() / ages.len().max(1) as f32, w.stats.births, w.stats.deaths);
}
