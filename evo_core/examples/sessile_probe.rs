//! How do zero-speed animals eat? Dump their daily budget after a long run.
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
fn main() {
    let mut w = EcoWorld::new(PlanetParams::earth(), 600, 4);
    for _ in 0..1500 {
        w.measure_pending(3);
        w.tick();
    }
    let mut still = 0;
    let mut moving = 0;
    let mut shown = 0;
    for a in &w.animals {
        let in_water = w.patches[a.patch].water;
        let sp = if in_water { a.traits.water_speed } else { a.traits.land_speed };
        if sp <= 0.0 { still += 1 } else { moving += 1 }
        if sp <= 0.0 && shown < 12 && a.age > 20.0 {
            shown += 1;
            let plant_b: f32 = w.plants.iter().filter(|p| p.patch == a.patch).map(|p| p.biomass).sum();
            println!(
                "mass {:.4}/{:.4} kg age {:>4.0} reserve {:.4} intake {:.5} | reach {:.3} mouth {:.5} measured w{} l{} | patch plants {:.2} kg carrion {:.3} kg temp {:.0} C | eaten plant {:.3} meat {:.3} carrion {:.3}",
                a.mass, a.traits.mass, a.age, a.reserve, a.intake_rate, a.traits.reach, a.traits.mouth_area, a.traits.measured_water, a.traits.measured_land,
                plant_b, w.patches[a.patch].carrion, w.patches[a.patch].temp_c, a.ate_plant, a.ate_meat, a.ate_carrion
            );
        }
    }
    println!("still {still} moving {moving} of {}", w.animals.len());
    // Parts histogram with mean measured water speed and mass per bin.
    let mut bins: Vec<(usize, f32, f32)> = vec![(0, 0.0, 0.0); 25];
    for a in &w.animals {
        let n = evo_core::decode::decode(&a.genome).parts.len().min(24);
        bins[n].0 += 1;
        bins[n].1 += a.traits.water_speed;
        bins[n].2 += a.traits.mass;
    }
    for (n, (c, sp, m)) in bins.iter().enumerate() {
        if *c > 0 {
            println!("parts {n:>2}: n={c:>4} mean water speed {:.4} m/s mean adult mass {:.3} kg", sp / *c as f32, m / *c as f32);
        }
    }
}
