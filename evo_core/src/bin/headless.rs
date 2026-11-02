//! Headless evolution runner.
//!
//! Usage: evo_headless [--planet N|name] [--env land|water|air] [--gens N]
//!        [--batch N] [--seed N] [--episode SECONDS] [--out champion.json]
//!        [--list-planets]
//!        evo_headless --eco [--planet N|name] [--days N] [--grid N] [--seed N]
//!        [--evals-per-day N]   (implicit-fitness planet ecology)

use evo_core::decode::decode;
use evo_core::evolve::{EvolveConfig, Evolver};
use evo_core::planet::PlanetParams;
use evo_core::sim::{Environment, SimConfig};

fn run_eco(planet: PlanetParams, days: u64, grid: usize, seed: u64, evals_per_day: usize) {
    use evo_core::eco::EcoWorld;
    let mut w = EcoWorld::new(planet.clone(), grid, seed);
    let d = planet.derive();
    println!(
        "Ecology on {} | grid {}x{} | g={:.1} | pO2 x{:.2} | NPP x{:.2} | aridity {:.2}",
        planet.name, grid, grid, planet.gravity, d.o2_rel, d.npp_rel, d.aridity
    );
    let start = std::time::Instant::now();
    for day in 1..=days {
        w.measure_pending(evals_per_day);
        w.tick();
        if day % (days / 10).max(1) == 0 || day == 1 {
            let s = &w.stats;
            let max_h = w.plants.iter().map(|p| p.height).fold(0.0f32, f32::max);
            println!(
                "day {:>5} | plants {:>5} ({:>8.1} kg, on land {:>4} / {:>3} patches green, tallest {:>4.1} m, {:>3} lin) | animals {:>3} (H {:>3} C {:>3} D {:>3}, aquatic {:>3}, {:>3} lin) | dead {:>7.1} nutr {:>7.1} | births {:>5} deaths {:>5} kills {:>4} | evals {} pending {}",
                day, s.plants, s.plant_biomass, s.land_plants, s.vegetated_land, max_h, s.plant_lineages, s.animals, s.herbivores, s.carnivores, s.detritivores, s.aquatic, s.animal_lineages,
                s.dead_matter, s.nutrient, s.births, s.deaths, s.kills, s.trait_evals, s.pending_evals
            );
        }
    }
    let el = start.elapsed().as_secs_f32();
    println!("{} days in {:.1} s ({:.0} days/s)", days, el, days as f32 / el);
    for l in w.lineages(6) {
        if l.kind == "animal" {
            println!(
                "  animal lineage {:>4}: n={:>3} mass {:>6.1} kg | diet H{:.2} C{:.2} D{:.2} | land {:.2} m/s water {:.2} m/s | parts {:.1} | membranes {:.2} m² | lung {:.0}% | aquatic {:.0}%",
                l.id, l.count, l.mean_mass, l.herb, l.carn, l.detr, l.land_speed, l.water_speed, l.parts, l.membrane_area, l.lung_frac * 100.0, l.aquatic_share * 100.0
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
    let mut eco = false;
    let mut days = 1500u64;
    let mut grid = 24usize;
    let mut evals_per_day = 3usize;
    let mut planet_sel = "0".to_string();
    let mut env = Environment::Land;
    let mut gens = 40u32;
    let mut batch = 24usize;
    let mut seed = 1u64;
    let mut episode = 8.0f32;
    let mut out: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--planet" => {
                planet_sel = args[i + 1].clone();
                i += 1;
            }
            "--env" => {
                env = match args[i + 1].to_lowercase().as_str() {
                    "water" => Environment::Water,
                    "air" => Environment::Air,
                    _ => Environment::Land,
                };
                i += 1;
            }
            "--gens" => {
                gens = args[i + 1].parse().unwrap_or(gens);
                i += 1;
            }
            "--batch" => {
                batch = args[i + 1].parse().unwrap_or(batch);
                i += 1;
            }
            "--seed" => {
                seed = args[i + 1].parse().unwrap_or(seed);
                i += 1;
            }
            "--episode" => {
                episode = args[i + 1].parse().unwrap_or(episode);
                i += 1;
            }
            "--out" => {
                out = Some(args[i + 1].clone());
                i += 1;
            }
            "--eco" => eco = true,
            "--days" => {
                days = args[i + 1].parse().unwrap_or(days);
                i += 1;
            }
            "--grid" => {
                grid = args[i + 1].parse().unwrap_or(grid);
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
    let planet = presets
        .iter()
        .enumerate()
        .find(|(k, p)| k.to_string() == planet_sel || p.name.to_lowercase().contains(&planet_sel.to_lowercase()))
        .map(|(_, p)| p.clone())
        .unwrap_or_else(PlanetParams::earth);
    if eco {
        run_eco(planet, days, grid, seed, evals_per_day);
        return;
    }
    let d = planet.derive();
    println!(
        "Planet: {} | g={:.2} m/s^2  air density={:.3} kg/m^3  pO2={:.1} kPa (x{:.2} Earth)",
        planet.name, planet.gravity, d.air_density, d.po2_kpa, d.o2_rel
    );
    println!(
        "Ecology: NPP x{:.2} Earth | plant:animal biomass {:.0}:1 | expected lung {:.0}% of torso | water storage {:.0}% | flight index {:.2} (stall speed {:.1} m/s at 50 N/m^2)",
        d.npp_rel,
        d.plant_animal_ratio,
        d.lung_frac * 100.0,
        d.water_storage_frac * 100.0,
        d.flight_index,
        d.stall_speed_ref
    );
    println!("Environment: {} | batch {} | {} generations | seed {}", env.name(), batch, gens, seed);

    let cfg = EvolveConfig {
        planet,
        env,
        sim: SimConfig { episode_time: episode, ..Default::default() },
        batch,
        seed,
        ..Default::default()
    };
    let mut ev = Evolver::new(cfg);
    for _ in 0..gens {
        let s = ev.step_generation();
        println!(
            "gen {:>3} | best {:>7.3} | batch best {:>7.3} | cells {:>2}/36 | nursery {:>2} | exploded {} | {:>5.1} evals/s",
            s.generation, s.best_fitness, s.batch_best, s.occupied_cells, s.nursery_size, s.exploded_in_batch, s.evals_per_second
        );
    }

    if let Some(ch) = ev.champion() {
        let pheno = decode(&ch.genome);
        println!("\nChampion (born gen {}):", ch.generation_born);
        println!(
            "  parts {} | membranes {} | mass {:.1} kg | membrane area {:.2} m^2 ({:.4} m^2/kg)",
            pheno.part_count(),
            pheno.membrane_count(),
            pheno.total_mass,
            pheno.membrane_area,
            pheno.area_per_mass()
        );
        println!(
            "  lung {:.1}% | water store {:.1}% | base freq {:.2} Hz | fatigue factor {:.2} | hydration {:.2}",
            pheno.lung_frac * 100.0,
            pheno.water_store * 100.0,
            pheno.base_freq,
            ch.result.fatigue,
            ch.result.hydration
        );
        println!(
            "  fitness {:.3} | distance {:.2} m | airborne {:.0}% | time aloft {:.2} s",
            ch.result.fitness,
            ch.result.distance,
            ch.result.airborne_frac * 100.0,
            ch.result.time_aloft
        );
        println!("\nArchive (rows: part-count bins 1-4,5-8,...; cols: membrane area/mass bins):");
        let grid = ev.grid_fitness();
        for r in 0..evo_core::evolve::GRID {
            let row: Vec<String> = (0..evo_core::evolve::GRID)
                .map(|c| match grid[r * evo_core::evolve::GRID + c] {
                    Some(f) => format!("{:>6.2}", f),
                    None => "     .".to_string(),
                })
                .collect();
            println!("  {}", row.join(" "));
        }
        if let Some(path) = out {
            std::fs::write(&path, ch.genome.to_json()).expect("write champion");
            println!("\nSaved champion genome to {path}");
        }
    }
}
