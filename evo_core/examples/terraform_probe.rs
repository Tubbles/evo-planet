//! Change the planet under a running world: does the world survive and respond?
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
fn main() {
    let mut w = EcoWorld::new(PlanetParams::earth(), 400, 3);
    let run = |w: &mut EcoWorld, days: u32| { for _ in 0..days { w.measure_pending(8); w.tick(); } };
    run(&mut w, 600);
    let sea = |w: &EcoWorld| w.patches.iter().filter(|p| p.water).count();
    println!("day {} | sea {} | plants {} animals {} | pending {}", w.day, sea(&w), w.plants.len(), w.animals.len(), w.stats.pending_evals);
    let mut p = w.planet.clone();
    p.water_coverage = 0.5; p.temp_c += 10.0; p.gravity = 20.0;
    let land_plants_before = w.plants.iter().filter(|q| !w.patches[q.patch].water).count();
    w.set_planet(p);
    println!("after set_planet: sea {} | plants {} (land before {}) | pending {} | peak temp {:.0}", sea(&w), w.plants.len(), land_plants_before,
        w.stats.pending_evals, w.patches.iter().max_by(|a, b| a.elevation.partial_cmp(&b.elevation).unwrap()).unwrap().temp_c);
    run(&mut w, 400);
    println!("day {} | sea {} | plants {} animals {} | pending {} | evals {} | tallest {:.2}", w.day, sea(&w), w.plants.len(), w.animals.len(), w.stats.pending_evals, w.stats.trait_evals,
        w.plants.iter().map(|q| q.height).fold(0.0f32, f32::max));
}
