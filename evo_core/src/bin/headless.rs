//! Headless planet ecology runner.
//!
//! Usage: evo_headless [--planet N|name] [--days N] [--grid PATCHES] [--seed N]
//!        [--patch-area M2] [--evals-per-day N] [--arena-per-day F] [--list-planets]
//!        [--dump-gaits DIR] [--save FILE] [--load FILE] [--water FRACTION]
//!
//! `--save` writes a checkpoint at every ledger line; `--load` carries on from
//! one (to `--days`, counted from the world's own day).
//!
//! `--eco` is accepted and ignored: the ecology is the only mode.

use evo_core::planet::PlanetParams;

fn run_eco(planet: PlanetParams, days: u64, patches: usize, seed: u64, evals_per_day: usize, patch_area: f32, arena_per_day: f32, dump_dir: Option<&str>, save: Option<&str>, load: Option<&str>) {
    use evo_core::eco::EcoWorld;
    let mut w = match load {
        Some(path) => {
            let w = EcoWorld::load(path).expect("checkpoint");
            println!("restored {path} at day {}", w.day);
            w
        }
        None => EcoWorld::new_with_area(planet.clone(), patches, seed, patch_area),
    };
    w.arena_rate = arena_per_day;
    let d = planet.derive();
    println!(
        "Ecology on {} | {} patches of {:.0} m2 on a sphere | g={:.1} | pO2 x{:.2} | NPP x{:.2} | aridity {:.2} | sea {:.0}% | highest peak {:.0} m ({:.0} C)",
        planet.name, patches, patch_area, planet.gravity, d.o2_rel, d.npp_rel, d.aridity,
        100.0 * w.patches.iter().filter(|p| p.water).count() as f32 / patches as f32,
        w.patches.iter().map(|p| p.elevation).fold(f32::MIN, f32::max),
        w.patches.iter().max_by(|a, b| a.elevation.partial_cmp(&b.elevation).unwrap()).map(|p| p.temp_c).unwrap_or(0.0)
    );
    let start = std::time::Instant::now();
    let first = w.day + 1;
    for day in first..=days {
        w.measure_pending(evals_per_day);
        w.tick();
        if day % (days / 10).max(1) == 0 || day == first {
            if let Some(path) = save {
                match w.save(path) {
                    Ok(()) => println!("saved {path} at day {day}"),
                    Err(e) => println!("save failed: {e}"),
                }
            }
            for l in w.lineages(3) {
                if l.kind == "animal" {
                    println!(
                        "    top lineage {:>5}: n={:>6} mass {:.3} kg parts {:.1} flat {:.2} | mouth {:.5} m² membranes {:.3} m² | land {:.2} water {:.2} m/s | diet plant {:.2} meat {:.2} carrion {:.2} | intake {:.3} /kg/day | lungs {:.2} skin {:.2} | on shore {:.0}% aquatic {:.0}%",
                        l.id, l.count, l.mean_mass, l.parts, l.flat, l.mouth, l.membrane_area, l.land_speed, l.water_speed, l.herb, l.carn, l.detr, l.intake, l.breathing, l.waterproof, l.shore_share * 100.0, l.aquatic_share * 100.0
                    );
                }
            }
            let s = &w.stats;
            let max_h = w.plants.iter().map(|p| p.height).fold(0.0f32, f32::max);
            println!(
                "day {:>5} | plants {:>5} ({:>8.1} kg, on land {:>4} = {:>6.1} kg / {:>3} patches green, tallest {:>4.1} m, {:>3} lin) | animals {:>3} (H {:>3} C {:>3} D {:>3}, aquatic {:>3}, on land {:>3} [{:>3} herb, {:>3} inland at {:.3} kg], best land speed {:.2} m/s, fliers {:>3} powered {:>3}, {:>3} lin) | omni {:>3} meat {:.2} carrion {:.2} | gut {:.2} claws {:.2} plating {:.2} | mass {:.2} kg elems {:.1} | P->A {} A->P {} | flows/day: NPP {:.1} graze {:.2} (mats {:.2}) prey {:.2} carrion {:.2} cost {:.2} | starved {} old {} suffocated {} dried {} | PPMR log10 {:.2} ({:.0}% by larger) satiated {:.0}% tough {:.2} | dead {:>7.1} nutr {:>7.1} | births {:>5} deaths {:>5} kills {:>4} | evals {} pending {} | arena {} pairs ({} waiting; eaten {:.0}% fled {:.0}%)",
                day, s.plants, s.plant_biomass, s.land_plants, s.land_plant_biomass, s.vegetated_land, max_h, s.plant_lineages, s.animals, s.herbivores, s.carnivores, s.detritivores, s.aquatic, s.land_animals, s.land_herbivores, s.inland_animals, s.inland_mass, s.best_land_speed, s.fliers, s.powered, s.animal_lineages, s.omnivores, s.mean_carn, s.mean_detr, s.mean_gut, s.mean_weapon, s.mean_armour, s.mean_mass, s.mean_parts, s.plant_to_animal, s.animal_to_plant, s.primary_production, s.grazing_intake, s.mat_grazing, s.predation_intake, s.scavenging_intake, s.metabolic_cost, s.deaths_starved, s.deaths_old, s.deaths_suffocated, s.deaths_dried, if s.kills > 0 { s.ppmr_log_sum / s.kills as f32 } else { 0.0 }, if s.kills > 0 { 100.0 * s.kills_by_larger as f32 / s.kills as f32 } else { 0.0 }, s.satiated_frac * 100.0, s.mean_toughness,
                s.dead_matter, s.nutrient, s.births, s.deaths, s.kills, s.trait_evals, s.pending_evals, s.arena_pairs, s.arena_pending, s.arena_eaten * 100.0, s.arena_fled * 100.0
            );
        }
    }
    let el = start.elapsed().as_secs_f32();
    println!("{} days in {:.1} s ({:.0} days/s)", days, el, days as f32 / el);
    println!("\nland lineages by gait (speed x clearance x stepping limbs):");
    for l in w.lineages_filtered("animal", "land", "gait", 6) {
        println!(
            "  lineage {:>5}: n={:>4} mass {:>6.2} kg | land {:.2} m/s | limbs {:.1} clearance {:.2} m COT {:.1} | upright {:.2} parts {:.1}",
            l.id, l.count, l.mean_mass, l.land_speed, l.limbs, l.clearance, l.cot, l.upright, l.parts
        );
        // Write a representative genome of each so it can be replayed in the page.
        if let Some(dir) = dump_dir {
            if let Some((g, _)) = w.representative_in(l.id, "land") {
                let path = format!("{}/lineage_{}.json", dir, l.id);
                if let Ok(s) = serde_json::to_string(&g) {
                    let _ = std::fs::create_dir_all(dir);
                    let _ = std::fs::write(&path, s);
                    println!("      genome written to {path}");
                }
            }
        }
    }
    println!();
    // Fought pairs, by outcome, for the audit.
    let outs = w.arena_outcomes();
    if !outs.is_empty() {
        let n = outs.len() as f32;
        println!(
            "arena: {} pairs | eaten {:.0}% fled {:.0}% survived {:.0}% | pairs with any kill: {}",
            outs.len(),
            100.0 * outs.iter().map(|o| o.3.eaten).sum::<f32>() / n,
            100.0 * outs.iter().map(|o| o.3.fled).sum::<f32>() / n,
            100.0 * outs.iter().map(|o| o.3.survived).sum::<f32>() / n,
            outs.iter().filter(|o| o.3.eaten > 0.0).count()
        );
        // The pairs themselves: who tried to eat whom, and how it went.
        let all = w.lineages(100000);
        let by_lineage: std::collections::HashMap<u32, &evo_core::eco::LineageSummary> = all.iter().map(|l| (l.id, l)).collect();
        for (pred, prey, env, o) in outs.iter().take(12) {
            let d = |id: &u32| by_lineage.get(id).map(|l| format!("{} ({:.3} kg, {:.1} parts, mouth {:.4} m², land {:.2} water {:.2} m/s)", l.id, l.mean_mass, l.parts, l.mouth, l.land_speed, l.water_speed)).unwrap_or_else(|| format!("{} (gone)", id));
            println!("  {:?}: {} vs {}: eaten {:.0}% fled {:.0}% survived {:.0}%", env, d(pred), d(prey), o.eaten * 100.0, o.fled * 100.0, o.survived * 100.0);
        }
    }
    for l in w.lineages(6) {
        if l.kind == "animal" {
            if let Some(dir) = dump_dir {
                if let Some((g, _)) = w.representative(l.id) {
                    let path = format!("{}/top_{}.json", dir, l.id);
                    if let Ok(s) = serde_json::to_string(&g) {
                        let _ = std::fs::create_dir_all(dir);
                        let _ = std::fs::write(&path, s);
                    }
                }
            }
            println!(
                "  animal lineage {:>4}: n={:>3} mass {:>6.2} kg | ate plant {:.2} meat {:.2} carrion {:.2} | gut {:.2} claws {:.2} plating {:.2} reach {:.2} m upright {:.2} | land {:.2} m/s water {:.2} m/s air {:.2} m/s glide {:.1} m lift {:.2} m limbs {:.1} clear {:.2} m COT {:.1} turn {:.2} rad/s climb {:.2} m/s shore {:.2} m lungs {:.2} | {:.2} BL/s | on shore {:.0}% | parts {:.1} | membranes {:.2} m² | aquatic {:.0}%",
                l.id, l.count, l.mean_mass, l.herb, l.carn, l.detr, l.gut, l.weapon, l.armour, l.reach, l.upright, l.land_speed, l.water_speed, l.air_speed, l.glide, l.lift, l.limbs, l.clearance, l.cot, l.turn, l.climb, l.shore, l.breathing, l.speed_bl, l.shore_share * 100.0, l.parts, l.membrane_area, l.aquatic_share * 100.0
            );
        } else {
            println!(
                "  plant lineage {:>4}: n={:>4} mass {:>6.2} kg | height {:>5.2} m | SLA {:>5.1} | root {:.2} | parts {:.1} | leaf {:.2} m² | land tol {:.2} | on land {:.0}%",
                l.id, l.count, l.mean_mass, l.height, l.sla, l.root_frac, l.parts, l.membrane_area, l.lung_frac, (1.0 - l.aquatic_share) * 100.0
            );
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut days = 1500u64;
    let mut grid = 1500usize;
    let mut evals_per_day = 96usize;
    let mut patch_area = evo_core::eco::PATCH_AREA;
    let mut arena_per_day = 1.0f32;
    let mut planet_sel = "0".to_string();
    let mut seed = 1u64;
    let mut dump_dir: Option<String> = None;
    let mut save: Option<String> = None;
    let mut water: Option<f32> = None;
    let mut load: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--water" => {
                water = args[i + 1].parse().ok();
                i += 1;
            }
            "--save" => {
                save = Some(args[i + 1].clone());
                i += 1;
            }
            "--load" => {
                load = Some(args[i + 1].clone());
                i += 1;
            }
            "--dump-gaits" => {
                dump_dir = Some(args[i + 1].clone());
                i += 1;
            }
            "--planet" => {
                planet_sel = args[i + 1].clone();
                i += 1;
            }
            "--seed" => {
                seed = args[i + 1].parse().unwrap_or(seed);
                i += 1;
            }
            "--eco" => {}
            "--days" => {
                days = args[i + 1].parse().unwrap_or(days);
                i += 1;
            }
            "--grid" => {
                grid = args[i + 1].parse().unwrap_or(grid);
                i += 1;
            }
            "--patch-area" => {
                patch_area = args[i + 1].parse().unwrap_or(patch_area);
                i += 1;
            }
            "--arena-per-day" => {
                arena_per_day = args[i + 1].parse().unwrap_or(arena_per_day);
                i += 1;
            }
            "--evals-per-day" => {
                evals_per_day = args[i + 1].parse().unwrap_or(evals_per_day);
                i += 1;
            }
            "--list-planets" => {
                for (k, p) in PlanetParams::presets().iter().enumerate() {
                    let d = p.derive();
                    println!(
                        "{k}: {:<28} g={:>5.2}  rho_air={:>6.3}  pO2={:>5.1}kPa  plant:animal={:>6.0}:1  lung={:.0}%  flight_idx={:.2}",
                        p.name, p.gravity, d.air_density, d.po2_kpa, d.plant_animal_ratio, d.lung_frac * 100.0, d.flight_index
                    );
                }
                return;
            }
            _ => {}
        }
        i += 1;
    }

    let presets = PlanetParams::presets();
    let mut planet = presets
        .iter()
        .enumerate()
        .find(|(k, p)| k.to_string() == planet_sel || p.name.to_lowercase().contains(&planet_sel.to_lowercase()))
        .map(|(_, p)| p.clone())
        .unwrap_or_else(PlanetParams::earth);
    if let Some(w) = water {
        // The planet's water-coverage dial, from the command line: a world
        // with more land than Earth's has more interior to walk on.
        planet.water_coverage = w.clamp(0.05, 0.97);
    }
    run_eco(planet, days, grid, seed, evals_per_day, patch_area, arena_per_day, dump_dir.as_deref(), save.as_deref(), load.as_deref());
}
