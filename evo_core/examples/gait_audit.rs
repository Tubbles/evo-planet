//! Do the fastest bodies actually locomote? Re-run the top land and water
//! bodies from an ecology with motors on and off (sham control), and log
//! rotation, height and progress. Genomes are written out for replay.
use evo_core::decode::decode;
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let days: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(2500);
    let out = args.get(2).cloned().unwrap_or_else(|| ".".into());
    let planet = PlanetParams::earth();
    let mut w = EcoWorld::new(planet.clone(), 600, 4);
    for _ in 0..days { w.measure_pending(8); w.tick(); }
    println!("day {} animals {}", w.day, w.animals.len());
    for (env, label) in [(Environment::Land, "land"), (Environment::Water, "water")] {
        let mut idx: Vec<usize> = (0..w.animals.len()).filter(|&i| if env == Environment::Land { w.animals[i].traits.measured_land } else { w.animals[i].traits.measured_water }).collect();
        idx.sort_by(|&a, &b| {
            let sa = if env == Environment::Land { w.animals[a].traits.land_speed } else { w.animals[a].traits.water_speed };
            let sb = if env == Environment::Land { w.animals[b].traits.land_speed } else { w.animals[b].traits.water_speed };
            sb.partial_cmp(&sa).unwrap()
        });
        idx.dedup_by_key(|i| w.animals[*i].key.clone());
        println!("--- top {label} bodies (measured speed, then re-run with motors on / off) ---");
        for (k, &i) in idx.iter().take(5).enumerate() {
            let a = &w.animals[i];
            let ph = decode(&a.genome);
            let mut row = String::new();
            for motors in [true, false] {
                let cfg = SimConfig { episode_time: 4.0, motors, ..Default::default() };
                let mut sim = CreatureSim::new(ph.clone(), &planet, env, cfg);
                let (mut ang, mut n, mut hsum) = (0.0f32, 0u32, 0.0f32);
                let mut marks = Vec::new();
                while !sim.finished() {
                    sim.step();
                    if sim.time > 1.0 { ang += sim.angular_speed(); hsum += sim.com().y - sim.ground_y; n += 1; }
                    let t = sim.time;
                    if (t * 2.0).fract() < 1.0 / 120.0 * 2.0 && marks.len() < 8 { marks.push(sim.horizontal_distance()); }
                }
                let r = sim.result();
                row += &format!(" | motors {}: sustained {:.3} m/s, dist {:.2} m, spin {:.2} rad/s, com h {:.2} m, path {:?}", if motors { "on " } else { "off" }, r.sustained_speed, r.distance, ang / n.max(1) as f32, hsum / n.max(1) as f32, marks.iter().map(|d| (d * 100.0).round() / 100.0).collect::<Vec<_>>());
            }
            let measured = if env == Environment::Land { a.traits.land_speed } else { a.traits.water_speed };
            let (turn, climb) = if env == Environment::Land { (a.traits.turn_land, 0.0) } else { (a.traits.turn_water, a.traits.climb_water) };
            println!("{label} #{k}: lineage {} parts {} mass {:.3} kg membranes {:.3} m2 lung {:.2} measured {:.3} m/s ({:.1} BL/s) turn {:.2} rad/s climb {:.2} m/s{}", a.lineage, ph.parts.len(), ph.total_mass, ph.membrane_area, a.genome.lung_frac, measured, measured / (2.0 * a.traits.reach).max(0.01), turn, climb, row);
            std::fs::write(format!("{out}/gait_{label}_{k}.json"), a.genome.to_json()).unwrap();
        }
    }
}
