//! How do sessile plants actually reproduce: at what fraction of target, with what repro_frac?
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
fn main() {
    let mut w = EcoWorld::new(PlanetParams::earth(), 600, 4);
    for _ in 0..1500 { w.measure_pending(2); w.tick(); }
    let n = w.plants.len() as f32;
    let mut rf: Vec<f32> = w.plants.iter().map(|p| p.genome.repro_frac).collect();
    rf.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let ratio: Vec<f32> = w.plants.iter().map(|p| p.biomass / p.body().target_mass).collect();
    let mean = |v: &Vec<f32>| v.iter().sum::<f32>() / n;
    let water: Vec<&evo_core::eco::Plant> = w.plants.iter().filter(|p| w.patches[p.patch].water).collect();
    let tm: f32 = water.iter().map(|p| p.body().target_mass).sum::<f32>() / water.len().max(1) as f32;
    let bm: f32 = water.iter().map(|p| p.biomass).sum::<f32>() / water.len().max(1) as f32;
    let la: f32 = water.iter().map(|p| p.body().leaf_area_total).sum::<f32>() / water.len().max(1) as f32;
    let lm: f32 = water.iter().map(|p| p.body().leaf_mass).sum::<f32>() / water.len().max(1) as f32;
    println!("plants {} | repro_frac median {:.2} p10 {:.2} p90 {:.2} | biomass/target mean {:.3} | sea plants: target {:.3} kg biomass {:.3} kg leaf area {:.2} m2 leaf mass {:.3} kg (areal {:.3} kg/m2) | seed mass median {:.4}",
        w.plants.len(), rf[rf.len()/2], rf[rf.len()/10], rf[rf.len()*9/10], mean(&ratio), tm, bm, la, lm, lm / la.max(1e-6), { let mut sm: Vec<f32> = w.plants.iter().map(|p| p.genome.seed_mass).collect(); sm.sort_by(|a,b| a.partial_cmp(b).unwrap()); sm[sm.len()/2] });
}
