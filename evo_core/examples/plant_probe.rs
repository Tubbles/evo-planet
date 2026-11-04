//! One founding plant's numbers, day by day, at the current scale.
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
fn main() {
    let mut w = EcoWorld::new_with_area(PlanetParams::earth(), 600, 4, 100.0);
    for day in 0..120 {
        if day % 20 == 0 {
            let n = w.plants.len();
            let p = w.plants.iter().max_by(|a, b| a.biomass.partial_cmp(&b.biomass).unwrap());
            match p {
                Some(p) => println!("day {day}: {n} plants | biggest: biomass {:.3e} kg energy {:.3e} target {:.3e} leaf area {:.3e} m² height {:.4} m seed mass {:.2e} age {:.0} lifespan gene {:.0}", p.biomass, p.energy, p.body().target_mass, p.leaf_area(), p.height, p.genome.seed_mass, p.age, p.genome.lifespan),
                None => println!("day {day}: no plants"),
            }
        }
        w.tick();
    }
}
