//! Background evolution: a small MAP-Elites archive per environment with
//! age-based morphological innovation protection.
//!
//! Archive axes: part count (6 bins) x membrane area per kilogram (6 bins).
//! Keeping one elite per cell preserves winged, legged and limbless lineages
//! side by side instead of letting the fastest runner take everything.
//!
//! Innovation protection: a child whose body changed and that lost to the
//! cell occupant is kept in a nursery for a few generations and used as a
//! parent, so its controller has time to re-adapt to the new body
//! (Cheney et al. 2016/2018, simplified).

use crate::decode::decode;
use crate::genome::Genome;
use crate::planet::PlanetParams;
use crate::sim::{CreatureSim, Environment, EpisodeResult, SimConfig};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

pub const GRID: usize = 6;
pub const NURSERY_MAX: usize = 12;
pub const NURSERY_AGE_MAX: u32 = 6;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Elite {
    pub genome: Genome,
    pub result: EpisodeResult,
    pub generation_born: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Protected {
    pub genome: Genome,
    pub result: EpisodeResult,
    pub age: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvolveConfig {
    pub planet: PlanetParams,
    pub env: Environment,
    pub sim: SimConfig,
    pub batch: usize,
    pub threads: usize,
    pub seed: u64,
    pub fresh_fraction: f32,
}

impl Default for EvolveConfig {
    fn default() -> Self {
        Self {
            planet: PlanetParams::earth(),
            env: Environment::Land,
            sim: SimConfig::default(),
            batch: 24,
            threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).max(1),
            seed: 1,
            fresh_fraction: 0.1,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GenSummary {
    pub generation: u32,
    pub evaluations: u64,
    pub best_fitness: f32,
    pub batch_best: f32,
    pub occupied_cells: usize,
    pub nursery_size: usize,
    pub evals_per_second: f32,
    pub exploded_in_batch: usize,
}

struct Pending {
    children: Vec<(Genome, bool)>,
    results: Vec<EpisodeResult>,
}

pub struct Evolver {
    pub cfg: EvolveConfig,
    pub archive: Vec<Option<Elite>>,
    pub nursery: Vec<Protected>,
    pub generation: u32,
    pub evaluations: u64,
    pub best: Option<Elite>,
    rng: StdRng,
    pub last_summary: GenSummary,
    pending: Option<Pending>,
}

pub fn descriptor_bins(part_count: usize, area_per_mass: f32) -> (usize, usize) {
    let a = ((part_count.saturating_sub(1)) / 4).min(GRID - 1);
    let thresholds = [0.0005f32, 0.003, 0.008, 0.02, 0.05];
    let mut b = 0;
    for t in thresholds {
        if area_per_mass > t {
            b += 1;
        }
    }
    (a, b.min(GRID - 1))
}

pub fn cell_index(part_count: usize, area_per_mass: f32) -> usize {
    let (a, b) = descriptor_bins(part_count, area_per_mass);
    a * GRID + b
}

impl Evolver {
    pub fn new(cfg: EvolveConfig) -> Self {
        Self {
            rng: StdRng::seed_from_u64(cfg.seed),
            cfg,
            archive: vec![None; GRID * GRID],
            nursery: Vec::new(),
            generation: 0,
            evaluations: 0,
            best: None,
            last_summary: GenSummary::default(),
            pending: None,
        }
    }

    pub fn reset(&mut self, cfg: EvolveConfig) {
        *self = Self::new(cfg);
    }

    fn occupied(&self) -> Vec<usize> {
        self.archive.iter().enumerate().filter(|(_, e)| e.is_some()).map(|(i, _)| i).collect()
    }

    fn pick_parent(&mut self) -> Option<(Genome, bool)> {
        let occ = self.occupied();
        let use_nursery = !self.nursery.is_empty() && (occ.is_empty() || self.rng.gen_bool(0.4));
        if use_nursery {
            let i = self.rng.gen_range(0..self.nursery.len());
            return Some((self.nursery[i].genome.clone(), true));
        }
        if occ.is_empty() {
            return None;
        }
        let i = occ[self.rng.gen_range(0..occ.len())];
        Some((self.archive[i].as_ref().unwrap().genome.clone(), false))
    }

    /// Produce one batch of children, evaluate them in parallel, insert.
    pub fn step_generation(&mut self) -> GenSummary {
        #[cfg(not(target_arch = "wasm32"))]
        let start = Instant::now();
        let children = self.make_children();
        let results = evaluate_batch(&children, &self.cfg);
        #[cfg(not(target_arch = "wasm32"))]
        let elapsed = start.elapsed().as_secs_f32().max(1e-6);
        #[cfg(target_arch = "wasm32")]
        let elapsed = 1.0;
        self.insert_results(children, results, elapsed)
    }

    fn make_children(&mut self) -> Vec<(Genome, bool)> {
        let mut children: Vec<(Genome, bool)> = Vec::with_capacity(self.cfg.batch);
        for _ in 0..self.cfg.batch {
            let fresh = self.rng.gen_bool(self.cfg.fresh_fraction as f64);
            match (fresh, self.pick_parent()) {
                (false, Some((parent, _))) => {
                    let (child, body_changed) = parent.mutate(&mut self.rng);
                    children.push((child, body_changed));
                }
                _ => {
                    children.push((Genome::random_with(&mut self.rng), true));
                }
            }
        }
        children
    }

    // --- Incremental, single-threaded path (used on wasm) ----------------

    /// Start a generation without evaluating anything yet.
    pub fn begin_generation(&mut self) {
        if self.pending.is_none() {
            let children = self.make_children();
            self.pending = Some(Pending { children, results: Vec::new() });
        }
    }

    /// Evaluate up to `max_evals` children of the pending generation.
    /// Returns true when the generation is complete and ready to finish.
    pub fn pump(&mut self, max_evals: usize) -> bool {
        self.begin_generation();
        let cfg = self.cfg.clone();
        let pending = self.pending.as_mut().unwrap();
        let mut n = 0;
        while pending.results.len() < pending.children.len() && n < max_evals {
            let (g, _) = &pending.children[pending.results.len()];
            pending.results.push(evaluate_one(g, &cfg));
            n += 1;
        }
        pending.results.len() == pending.children.len()
    }

    /// Insert the pending generation's results. `elapsed_s` is wall time
    /// spent, supplied by the caller (wasm has no monotonic clock in std).
    pub fn finish_generation(&mut self, elapsed_s: f32) -> Option<GenSummary> {
        let pending = self.pending.take()?;
        if pending.results.len() != pending.children.len() {
            self.pending = Some(pending);
            return None;
        }
        Some(self.insert_results(pending.children, pending.results, elapsed_s.max(1e-6)))
    }

    fn insert_results(&mut self, children: Vec<(Genome, bool)>, results: Vec<EpisodeResult>, elapsed: f32) -> GenSummary {
        self.evaluations += results.len() as u64;
        self.generation += 1;

        let mut batch_best = 0.0f32;
        let mut exploded = 0;
        for ((genome, body_changed), result) in children.into_iter().zip(results.into_iter()) {
            if result.exploded {
                exploded += 1;
            }
            batch_best = batch_best.max(result.fitness);
            let idx = cell_index(result.part_count, result.area_per_mass);
            let beats = match &self.archive[idx] {
                None => true,
                Some(e) => result.fitness > e.result.fitness,
            };
            if beats {
                self.archive[idx] = Some(Elite { genome: genome.clone(), result: result.clone(), generation_born: self.generation });
            } else if body_changed && result.fitness > 0.0 {
                // Innovation protection: new body, not yet competitive.
                let occupant = self.archive[idx].as_ref().map(|e| e.result.fitness).unwrap_or(0.0);
                if result.fitness > 0.25 * occupant && self.nursery.len() < NURSERY_MAX {
                    self.nursery.push(Protected { genome: genome.clone(), result: result.clone(), age: 0 });
                }
            }
            let is_best = self.best.as_ref().map(|b| result.fitness > b.result.fitness).unwrap_or(true);
            if is_best && !result.exploded {
                self.best = Some(Elite { genome, result, generation_born: self.generation });
            }
        }

        for p in self.nursery.iter_mut() {
            p.age += 1;
        }
        self.nursery.retain(|p| p.age <= NURSERY_AGE_MAX);

        let summary = GenSummary {
            generation: self.generation,
            evaluations: self.evaluations,
            best_fitness: self.best.as_ref().map(|b| b.result.fitness).unwrap_or(0.0),
            batch_best,
            occupied_cells: self.occupied().len(),
            nursery_size: self.nursery.len(),
            evals_per_second: self.cfg.batch as f32 / elapsed,
            exploded_in_batch: exploded,
        };
        self.last_summary = summary.clone();
        summary
    }

    pub fn champion(&self) -> Option<&Elite> {
        self.best.as_ref()
    }

    /// Fitness grid for display, row = part-count bin, col = area/mass bin.
    pub fn grid_fitness(&self) -> Vec<Option<f32>> {
        self.archive.iter().map(|e| e.as_ref().map(|e| e.result.fitness)).collect()
    }
}

pub fn evaluate_one(genome: &Genome, cfg: &EvolveConfig) -> EpisodeResult {
    let pheno = decode(genome);
    CreatureSim::run_episode(pheno, &cfg.planet, cfg.env, cfg.sim)
}

pub fn evaluate_batch(children: &[(Genome, bool)], cfg: &EvolveConfig) -> Vec<EpisodeResult> {
    let threads = cfg.threads.max(1).min(children.len().max(1));
    let mut results: Vec<Option<EpisodeResult>> = vec![None; children.len()];
    std::thread::scope(|s| {
        let chunk = (children.len() + threads - 1) / threads;
        let mut handles = Vec::new();
        for (ti, slice) in children.chunks(chunk).enumerate() {
            let cfg = cfg.clone();
            handles.push(s.spawn(move || {
                let mut out = Vec::with_capacity(slice.len());
                for (g, _) in slice {
                    out.push(evaluate_one(g, &cfg));
                }
                (ti, out)
            }));
        }
        for h in handles {
            let (ti, out) = h.join().expect("evaluation thread panicked");
            for (k, r) in out.into_iter().enumerate() {
                results[ti * chunk + k] = Some(r);
            }
        }
    });
    results.into_iter().map(|r| r.unwrap_or_default()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_path_matches_batch_shape() {
        let cfg = EvolveConfig {
            batch: 6,
            threads: 1,
            sim: SimConfig { episode_time: 1.0, ..Default::default() },
            ..Default::default()
        };
        let mut ev = Evolver::new(cfg);
        let mut done = false;
        let mut pumps = 0;
        while !done {
            done = ev.pump(2);
            pumps += 1;
        }
        assert_eq!(pumps, 3);
        let s = ev.finish_generation(0.5).unwrap();
        assert_eq!(s.generation, 1);
        assert_eq!(s.evaluations, 6);
        assert!(ev.finish_generation(0.1).is_none());
    }

    #[test]
    fn evolver_runs_a_few_generations() {
        let cfg = EvolveConfig {
            batch: 8,
            threads: 2,
            sim: SimConfig { episode_time: 2.0, ..Default::default() },
            ..Default::default()
        };
        let mut ev = Evolver::new(cfg);
        for _ in 0..3 {
            let s = ev.step_generation();
            assert!(s.best_fitness.is_finite());
        }
        assert!(ev.champion().is_some());
        assert!(ev.last_summary.occupied_cells >= 1);
    }
}
