//! Watch the first animals a centimetre world makes: where they are born and
//! how they fare, day by day.
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
fn main() {
    let seed: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(4);
    let mut planet = PlanetParams::earth();
    planet.water_coverage = 0.2;
    let mut w = EcoWorld::new_with_area(planet, 600, seed, 100.0);
    let mut seen = 0u64;
    let mut watched = 0;
    for day in 1..=6000 {
        w.measure_pending(16);
        w.tick();
        let s = &w.stats;
        if s.plant_to_animal > seen {
            println!(
                "day {day}: plant-to-animal births now {} | animals alive {} | deaths so far: starved {} dried {} old {} suffocated {} | births {} deaths {}",
                s.plant_to_animal, w.animals.len(), s.deaths_starved, s.deaths_dried, s.deaths_old, s.deaths_suffocated, s.births, s.deaths
            );
            seen = s.plant_to_animal;
        }
        if !w.animals.is_empty() && watched < 40 {
            watched += 1;
            let a = &w.animals[0];
            let p = &w.patches[a.patch];
            println!(
                "day {day}: {} animals ({} born from plants so far) | first: mass {:.2e} kg reserve {:.2e} intake {:.2e}/day water deficit {:.2e} | cell submersion {:.2} moisture {:.2} | membrane {:.2e} m² mouth {:.2e} m² breathing {:.2} motility {:.2} | deaths: starved {} dried {} old {} suffocated {}",
                w.animals.len(), seen, a.mass, a.reserve, a.intake_rate, a.water, p.submersion, p.moisture, a.traits.membrane_area, a.traits.mouth_area, a.genome.breathing, a.genome.motility, s.deaths_starved, s.deaths_dried, s.deaths_old, s.deaths_suffocated
            );
        }
        if watched >= 40 {
            break;
        }
    }
    println!("plant-to-animal births: {seen}; ended day {} with {} animals", w.day, w.animals.len());
}
