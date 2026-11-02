use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
fn main() {
    let mut w = EcoWorld::new(PlanetParams::earth(), 400, 7);
    // pick an animal in a patch with plants
    let idx = (0..w.animals.len()).find(|&i| w.plants.iter().any(|p| p.patch == w.animals[i].patch)).unwrap();
    let key = w.animals[idx].key.clone();
    let patch = w.animals[idx].patch;
    let pb: f32 = w.plants.iter().filter(|p| p.patch == patch).map(|p| p.biomass).sum();
    let p0 = w.plants.iter().find(|p| p.patch == patch).unwrap();
    println!("before: mass {:.2} reserve {:.3} gut {:.3} patch plants {:.3} kg | plant0 biomass {:.3} leaf_mass {:.4} target {:.4} height {:.2} parts {}", w.animals[idx].mass, w.animals[idx].reserve, w.animals[idx].gut_plant, pb, p0.biomass, p0.body().leaf_mass, p0.body().target_mass, p0.height, p0.body().part_count);
    for day in 1..=3 {
        w.tick();
        if let Some(a) = w.animals.iter().find(|a| a.key == key) {
            let pb: f32 = w.plants.iter().filter(|p| p.patch == a.patch).map(|p| p.biomass).sum();
            println!("day {day}: patch {} (same {}) reserve {:.3} gut {:.3} ate_plant {:.4} intake {:.4} plants here {:.3} kg", a.patch, a.patch == patch, a.reserve, a.gut_plant, a.ate_plant, a.intake_rate, pb);
        } else { println!("animal gone"); }
    }
}
