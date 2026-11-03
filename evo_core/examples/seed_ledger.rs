//! Follow the first motile seeds in an algae-only world: daily energy ledger.
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
fn main() {
    let mut w = EcoWorld::new(PlanetParams::earth(), 600, 4);
    let mut followed: Vec<(String, u32)> = Vec::new(); // (key, lineage)
    let mut shown = 0;
    for d in 1..=5000u32 {
        w.measure_pending(8);
        w.tick();
        for a in &w.animals {
            if !followed.iter().any(|(k, _)| *k == a.key) && followed.len() < 3 {
                followed.push((a.key.clone(), a.lineage));
                let pt = &w.patches[a.patch];
                println!("day {d}: NEW motile seed lineage {} | mass {:.4} kg (adult {:.3}) membranes {:.3} m2 reach {:.3} | motility {:.2} heterotrophy {:.2} offspring_frac {:.2} | patch water {} sub {:.1} light_left {:.2} bacteria {:.4} kg dead {:.2} kg plants {}",
                    a.lineage, a.mass, a.traits.mass, a.traits.membrane_area, a.traits.reach, a.genome.motility, a.genome.heterotrophy, a.genome.offspring_frac,
                    pt.water, pt.submersion, pt.light_left, pt.bacteria, pt.dead_matter, w.plants.iter().filter(|p| p.patch == a.patch).count());
            }
        }
        if shown < 90 {
            for (k, lin) in &followed {
                if let Some(a) = w.animals.iter().find(|a| a.key == *k) {
                    println!("  day {d} lin {lin}: mass {:.4} reserve {:.4} gross intake {:.5} ate plant {:.4} carrion {:.4} age {:.0} patch {} water_speed {:.3}", a.mass, a.reserve, a.intake_rate, a.ate_plant, a.ate_carrion, a.age, a.patch, a.traits.water_speed);
                    shown += 1;
                }
            }
        }
        if followed.len() >= 3 && shown >= 90 { break; }
    }
}
