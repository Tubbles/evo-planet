//! Re-measure the ecology's "fliers": are they really getting through the air?
use evo_core::decode::decode;
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
fn main() {
    let mut w = EcoWorld::new(PlanetParams::earth(), 400, 4);
    for _ in 0..900 { w.measure_pending(8); w.tick(); }
    let mut shown = 0;
    for a in &w.animals {
        if a.traits.air_speed > 0.5 && shown < 8 {
            shown += 1;
            let ph = decode(&a.genome);
            let cfg = SimConfig { episode_time: 3.0, ..Default::default() };
            let r = CreatureSim::run_episode(ph.clone(), &PlanetParams::earth(), Environment::Air, cfg);
            println!("trait air {:.2} m/s | parts {} mass {:.3} area/mass {:.3} | re-run: air_distance {:.2} total distance {:.2} airborne {:.2} aloft {:.2} s exploded {}",
                a.traits.air_speed, ph.parts.len(), ph.total_mass, ph.area_per_mass(), r.air_distance, r.distance, r.airborne_frac, r.time_aloft, r.exploded);
        }
    }
    println!("fliers stat {} of {}", w.stats.fliers, w.animals.len());
}
