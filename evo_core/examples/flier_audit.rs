//! Replay-audit the registered fliers and the most numerous lineage of a run.
use evo_core::decode::decode;
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;
use evo_core::sim::{ballistic_distance, CreatureSim, Environment, SimConfig};
use std::collections::HashMap;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let days: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(3000);
    let out = args.get(2).cloned().unwrap_or_else(|| ".".into());
    let planet = PlanetParams::earth();
    let mut w = EcoWorld::new(planet.clone(), 600, 4);
    for _ in 0..days { w.measure_pending(8); w.tick(); }
    println!("day {} animals {} fliers(stat) {}", w.day, w.animals.len(), w.stats.fliers);
    // Fliers: distinct bodies with glide or air speed.
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut fl: Vec<usize> = Vec::new();
    for (i, a) in w.animals.iter().enumerate() {
        if (a.traits.glide_distance > 0.5 || a.traits.air_speed > 0.0) && !seen.contains_key(&a.key) {
            seen.insert(a.key.clone(), i);
            fl.push(i);
        }
    }
    fl.sort_by(|&a, &b| w.animals[b].traits.glide_distance.partial_cmp(&w.animals[a].traits.glide_distance).unwrap());
    println!("--- {} distinct flier bodies; stone lands at {:.2} m ---", fl.len(), ballistic_distance(planet.gravity));
    for (k, &i) in fl.iter().take(4).enumerate() {
        let a = &w.animals[i];
        let ph = decode(&a.genome);
        let mut row = String::new();
        for motors in [true, false] {
            let cfg = SimConfig { episode_time: 4.0, motors, ..Default::default() };
            let mut sim = CreatureSim::new(ph.clone(), &planet, Environment::Air, cfg);
            let mut marks = Vec::new();
            while !sim.finished() {
                sim.step();
                if (sim.time * 2.0).fract() < 1.0 / 120.0 * 2.0 && marks.len() < 8 { let c = sim.com(); marks.push(((c.x * 100.0).round() / 100.0, (c.y * 100.0).round() / 100.0)); }
            }
            let r = sim.result();
            row += &format!(" | motors {}: air dist {:.2} m, airborne {:.2}, aloft {:.2} s, exploded {}, com (x,y) {:?}", if motors { "on " } else { "off" }, r.air_distance, r.airborne_frac, r.time_aloft, r.exploded, marks);
        }
        println!("flier #{k}: lineage {} parts {} mass {:.3} kg membranes {:.4} m2 ({:.3} m2/kg) trait glide {:.2} m air {:.2} m/s{}", a.lineage, ph.parts.len(), ph.total_mass, ph.membrane_area, ph.area_per_mass(), a.traits.glide_distance, a.traits.air_speed, row);
        std::fs::write(format!("{out}/flier_{k}.json"), a.genome.to_json()).unwrap();
    }
    // Most numerous lineage: its most prolific member.
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for a in &w.animals { *counts.entry(a.lineage).or_insert(0) += 1; }
    let (&lin, &n) = counts.iter().max_by_key(|(_, &c)| c).unwrap();
    if let Some((g, env)) = w.representative(lin) {
        let ph = decode(&g);
        let t = w.animals.iter().find(|a| a.lineage == lin).map(|a| a.traits).unwrap();
        let mut row = String::new();
        for motors in [true, false] {
            let cfg = SimConfig { episode_time: 4.0, motors, ..Default::default() };
            let mut sim = CreatureSim::new(ph.clone(), &planet, Environment::Water, cfg);
            let (mut ang, mut nn) = (0.0f32, 0u32);
            while !sim.finished() { sim.step(); if sim.time > 1.0 { ang += sim.angular_speed(); nn += 1; } }
            let r = sim.result();
            row += &format!(" | motors {}: sustained {:.3} m/s dist {:.2} m spin {:.2} rad/s com h {:.2} exploded {}", if motors { "on " } else { "off" }, r.sustained_speed, r.distance, ang / nn.max(1) as f32, sim.com().y - sim.ground_y, r.exploded);
        }
        println!("top lineage {lin}: n={n} env {:?} parts {} mass {:.3} kg membranes {:.4} m2 | traits water {:.3} m/s turn {:.2} rad/s climb {:.2} m/s{}", env, ph.parts.len(), ph.total_mass, ph.membrane_area, t.water_speed, t.turn_water, t.climb_water, row);
        std::fs::write(format!("{out}/top_lineage.json"), g.to_json()).unwrap();
    }
}
