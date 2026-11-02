//! Planet-scale ecology with implicit fitness.
//!
//! No score. Plants capture light, draw water and nutrients, pay respiration
//! and structural cost, and seed when they have surplus energy. Animals eat
//! plants, dead matter or each other, pay basal and locomotion costs, and
//! reproduce when energy crosses a threshold. Dead things feed a detritus
//! pool that decomposers return to the nutrient pool. Whatever persists is
//! fit by definition.
//!
//! Animal bodies are the Sims-style genomes from `genome`. Their movement
//! traits (speed on land, in water, oxygen-limited activity) are measured
//! once per genome with a short rapier episode and cached, so physics cost
//! scales with the number of new body plans, not with population.
//!
//! Units are abstract but consistent: one tick is one "day"; energy is in
//! kilograms of dry biomass equivalent; patches are 10 m squares.

use crate::decode::decode;
use crate::genome::Genome;
use crate::planet::{Derived, PlanetParams};
use crate::sim::{fatigue_factor, CreatureSim, Environment, SimConfig};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const PATCH_AREA: f32 = 100.0;
pub const MAX_PLANTS_PER_PATCH: usize = 8;
pub const MAX_ANIMALS: usize = 260;
pub const TRAIT_EPISODE_S: f32 = 3.0;

// ---------------------------------------------------------------------------
// Plants
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlantGenome {
    /// Target height in metres.
    pub height: f32,
    /// Target total biomass, kg.
    pub target_mass: f32,
    /// Allocation fractions of biomass (normalised on use).
    pub leaf_frac: f32,
    pub stem_frac: f32,
    pub root_frac: f32,
    /// Specific leaf area, m^2 per kg of leaf. Thin leaves capture more
    /// light per kg but transpire more.
    pub sla: f32,
    /// Seed mass, kg. Bigger seeds survive longer before paying off.
    pub seed_mass: f32,
    /// Dispersal distance in patches at zero height.
    pub dispersal: f32,
    /// Lifespan in days.
    pub lifespan: f32,
    /// 0 = fully aquatic, 1 = fully adapted to land (cuticle, vascular
    /// tissue, roots that draw soil water). Costs respiration everywhere.
    #[serde(default)]
    pub land_tolerance: f32,
    pub lineage: u32,
}

impl PlantGenome {
    pub fn random(rng: &mut StdRng, lineage: u32) -> Self {
        Self {
            height: rng.gen_range(0.2..3.0),
            target_mass: rng.gen_range(0.5..20.0),
            leaf_frac: rng.gen_range(0.2..0.6),
            stem_frac: rng.gen_range(0.1..0.5),
            root_frac: rng.gen_range(0.1..0.5),
            sla: rng.gen_range(4.0..25.0),
            seed_mass: rng.gen_range(0.002..0.08),
            dispersal: rng.gen_range(0.5..3.0),
            lifespan: rng.gen_range(60.0..1200.0),
            land_tolerance: rng.gen_range(0.0..0.2),
            lineage,
        }
    }

    /// Returns (child, big_change).
    pub fn mutate(&self, rng: &mut StdRng) -> (Self, bool) {
        let mut g = self.clone();
        let mut big = false;
        let mut n = |v: &mut f32, rel: f32, lo: f32, hi: f32, big_flag: &mut bool| {
            if rng.gen_bool(0.35) {
                let u: f32 = rng.gen_range(-1.0..1.0);
                let delta = u * u * u * rel * 3.0; // heavy-tailed
                *v = (*v * (1.0 + delta)).clamp(lo, hi);
                if delta.abs() > 0.25 {
                    *big_flag = true;
                }
            }
        };
        n(&mut g.height, 0.2, 0.05, 40.0, &mut big);
        n(&mut g.target_mass, 0.2, 0.05, 2000.0, &mut big);
        n(&mut g.leaf_frac, 0.2, 0.05, 0.9, &mut big);
        n(&mut g.stem_frac, 0.2, 0.02, 0.9, &mut big);
        n(&mut g.root_frac, 0.2, 0.02, 0.9, &mut big);
        n(&mut g.sla, 0.2, 1.0, 40.0, &mut big);
        n(&mut g.seed_mass, 0.25, 0.0005, 2.0, &mut big);
        n(&mut g.dispersal, 0.25, 0.1, 12.0, &mut big);
        n(&mut g.lifespan, 0.2, 10.0, 5000.0, &mut big);
        if rng.gen_bool(0.35) {
            let d: f32 = rng.gen_range(-0.12..0.12);
            g.land_tolerance = (g.land_tolerance + d).clamp(0.0, 1.0);
            if d.abs() > 0.08 {
                big = true;
            }
        }
        (g, big)
    }

    pub fn fractions(&self) -> (f32, f32, f32) {
        let s = (self.leaf_frac + self.stem_frac + self.root_frac).max(1e-6);
        (self.leaf_frac / s, self.stem_frac / s, self.root_frac / s)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plant {
    pub genome: PlantGenome,
    pub patch: usize,
    pub biomass: f32,
    pub energy: f32,
    pub age: f32,
    /// Realised height after the structural check, metres.
    pub height: f32,
    pub light_frac: f32,
    pub water_frac: f32,
}

// ---------------------------------------------------------------------------
// Animals
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Traits {
    pub mass: f32,
    pub land_speed: f32,
    pub water_speed: f32,
    pub glide_distance: f32,
    pub activity: f32,
    pub section_area: f32,
    pub base_freq: f32,
    pub membrane_area: f32,
    pub measured_land: bool,
    pub measured_water: bool,
    pub measured_air: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Animal {
    pub genome: Genome,
    #[serde(default)]
    pub key: String,
    pub traits: Traits,
    pub patch: usize,
    pub energy: f32,
    pub age: f32,
    pub lineage: u32,
    pub kills: u32,
    pub offspring: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Patch {
    pub water: bool,
    pub moisture: f32,
    pub nutrient: f32,
    pub dead_matter: f32,
    /// Standing algae in water patches, kg. Grows with light, grazed by aquatic herbivores.
    pub algae: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EcoStats {
    pub day: u64,
    pub plants: usize,
    pub plant_biomass: f32,
    pub animals: usize,
    pub herbivores: usize,
    pub carnivores: usize,
    pub detritivores: usize,
    pub aquatic: usize,
    pub land_plants: usize,
    pub land_animals: usize,
    pub land_patches: usize,
    pub vegetated_land: usize,
    pub plant_lineages: usize,
    pub animal_lineages: usize,
    pub dead_matter: f32,
    pub nutrient: f32,
    pub births: u64,
    pub deaths: u64,
    pub kills: u64,
    pub immigrants: u64,
    pub trait_evals: u64,
    pub pending_evals: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LineageSummary {
    pub id: u32,
    pub kind: String,
    pub count: usize,
    pub mean_mass: f32,
    pub herb: f32,
    pub carn: f32,
    pub detr: f32,
    pub aquatic_share: f32,
    pub land_speed: f32,
    pub water_speed: f32,
    pub parts: f32,
    pub membrane_area: f32,
    pub lung_frac: f32,
    pub height: f32,
    pub sla: f32,
    pub root_frac: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub n: usize,
    pub day: u64,
    pub water: Vec<u8>,
    pub vegetation: Vec<u8>,
    pub dead: Vec<u8>,
    pub canopy_height: Vec<u8>,
    /// x, y, lineage, mass, diet class (0 herb 1 carn 2 detr), aquatic
    pub animals: Vec<f32>,
    pub stats: EcoStats,
    pub lineages: Vec<LineageSummary>,
}

pub struct EcoWorld {
    pub n: usize,
    pub planet: PlanetParams,
    pub derived: Derived,
    pub patches: Vec<Patch>,
    pub plants: Vec<Plant>,
    pub animals: Vec<Animal>,
    pub day: u64,
    rng: StdRng,
    next_lineage: u32,
    trait_cache: HashMap<String, Traits>,
    pending: Vec<(usize, Environment)>,
    pub stats: EcoStats,
    pub seeds: Vec<Genome>,
    pub history: Vec<[f32; 7]>,
}

fn temp_factor(temp_c: f32) -> f32 {
    (-((temp_c - 22.0) / 16.0).powi(2)).exp()
}

fn co2_factor(co2_frac: f32) -> f32 {
    let ppm = co2_frac * 1.0e6;
    (ppm / (ppm + 250.0)) / (400.0 / 650.0)
}

fn hash_genome(g: &Genome) -> String {
    serde_json::to_string(g).unwrap_or_default()
}

impl EcoWorld {
    pub fn new(planet: PlanetParams, n: usize, seed: u64) -> Self {
        let derived = planet.derive();
        let mut rng = StdRng::seed_from_u64(seed);
        let n = n.clamp(8, 96);
        // Water mask from smoothed noise thresholded at water coverage.
        let mut noise: Vec<f32> = (0..n * n).map(|_| rng.gen_range(0.0..1.0)).collect();
        for _ in 0..3 {
            let mut out = noise.clone();
            for y in 0..n {
                for x in 0..n {
                    let mut s = 0.0;
                    let mut c = 0.0;
                    for dy in -2i32..=2 {
                        for dx in -2i32..=2 {
                            let xx = (x as i32 + dx).rem_euclid(n as i32) as usize;
                            let yy = (y as i32 + dy).rem_euclid(n as i32) as usize;
                            s += noise[yy * n + xx];
                            c += 1.0;
                        }
                    }
                    out[y * n + x] = s / c;
                }
            }
            noise = out;
        }
        let mut sorted = noise.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let k = ((planet.water_coverage.clamp(0.0, 1.0)) * (n * n) as f32) as usize;
        let thr = if k == 0 { -1.0 } else { sorted[(k - 1).min(n * n - 1)] };
        let patches: Vec<Patch> = (0..n * n)
            .map(|i| {
                let water = noise[i] <= thr;
                // Land moisture: humidity plus a wetness gradient toward low ground.
                let moisture = if water {
                    1.0
                } else {
                    (planet.humidity * 0.7 + 0.5 * (thr.max(0.0) + 0.15 - noise[i]).clamp(0.0, 0.3) * 2.0).clamp(0.02, 1.0)
                };
                Patch { water, moisture, nutrient: 2.0, dead_matter: 1.0, algae: if water { 5.0 } else { 0.0 } }
            })
            .collect();

        // Shore band: land patches touching water stay wet, the first foothold for plants.
        let mut patches = patches;
        for i in 0..n * n {
            if patches[i].water {
                continue;
            }
            let x = i % n;
            let y = i / n;
            let nb = [y * n + (x + 1) % n, y * n + (x + n - 1) % n, ((y + 1) % n) * n + x, ((y + n - 1) % n) * n + x];
            if nb.iter().any(|&j| patches[j].water) {
                patches[i].moisture = patches[i].moisture.max(0.85);
            }
        }
        let mut world = Self {
            n,
            planet,
            derived,
            patches,
            plants: Vec::new(),
            animals: Vec::new(),
            day: 0,
            rng,
            next_lineage: 1,
            trait_cache: HashMap::new(),
            pending: Vec::new(),
            stats: EcoStats::default(),
            seeds: Vec::new(),
            history: Vec::new(),
        };
        world.seed_plants(n * n / 3);
        world.immigrate(60);
        world
    }

    fn new_lineage(&mut self) -> u32 {
        let id = self.next_lineage;
        self.next_lineage += 1;
        id
    }

    fn seed_plants(&mut self, count: usize) {
        for _ in 0..count {
            let lineage = self.new_lineage();
            let g = PlantGenome::random(&mut self.rng, lineage);
            // Life starts in the oceans: seed water patches when there are any.
            let mut pool: Vec<usize> = (0..self.patches.len()).filter(|&i| self.patches[i].water).collect();
            if pool.is_empty() {
                pool = (0..self.patches.len()).collect();
            }
            let patch = pool[self.rng.gen_range(0..pool.len())];
            self.plants.push(Plant {
                biomass: g.target_mass * 0.2,
                energy: g.target_mass * 0.1,
                age: 0.0,
                height: 0.1,
                light_frac: 0.0,
                water_frac: 0.0,
                genome: g,
                patch,
            });
        }
    }

    /// Add fresh animals: from the seed stock if any, else random genomes.
    pub fn immigrate(&mut self, count: usize) {
        for _ in 0..count {
            if self.animals.len() >= MAX_ANIMALS {
                return;
            }
            let genome = if !self.seeds.is_empty() && self.rng.gen_bool(0.6) {
                let i = self.rng.gen_range(0..self.seeds.len());
                let (g, _) = self.seeds[i].mutate(&mut self.rng);
                g
            } else {
                Genome::random_with(&mut self.rng)
            };
            let lineage = self.new_lineage();
            let water: Vec<usize> = (0..self.patches.len()).filter(|&i| self.patches[i].water).collect();
            let patch = if water.is_empty() { self.rng.gen_range(0..self.patches.len()) } else { water[self.rng.gen_range(0..water.len())] };
            let traits = self.lookup_traits(&genome);
            let mass = traits.mass;
            let key = hash_genome(&genome);
            self.animals.push(Animal { genome, key, traits, patch, energy: mass * 0.5 + 0.2, age: 0.0, lineage, kills: 0, offspring: 0 });
            let idx = self.animals.len() - 1;
            self.request_traits(idx);
            self.stats.immigrants += 1;
        }
    }

    fn lookup_traits(&mut self, g: &Genome) -> Traits {
        let key = hash_genome(g);
        if let Some(t) = self.trait_cache.get(&key) {
            return *t;
        }
        let pheno = decode(g);
        let t = Traits {
            mass: pheno.total_mass,
            activity: fatigue_factor(&pheno, &self.derived),
            section_area: pheno.total_section_area,
            base_freq: pheno.base_freq,
            membrane_area: pheno.membrane_area,
            ..Default::default()
        };
        self.trait_cache.insert(key, t);
        t
    }

    fn request_traits(&mut self, idx: usize) {
        let env = if self.patches[self.animals[idx].patch].water { Environment::Water } else { Environment::Land };
        let t = &self.animals[idx].traits;
        let done = match env {
            Environment::Water => t.measured_water,
            _ => t.measured_land,
        };
        if !done && !self.pending.iter().any(|(i, e)| *i == idx && *e == env) {
            self.pending.push((idx, env));
        }
    }

    /// Run up to `budget` physics trait measurements for pending animals.
    pub fn measure_pending(&mut self, budget: usize) -> usize {
        let mut done = 0;
        while done < budget {
            let Some((idx, env)) = self.pending.pop() else { break };
            if idx >= self.animals.len() {
                continue;
            }
            let genome = self.animals[idx].genome.clone();
            let key = self.animals[idx].key.clone();
            let mut traits = self.trait_cache.get(&key).copied().unwrap_or(self.animals[idx].traits);
            let already = match env {
                Environment::Water => traits.measured_water,
                _ => traits.measured_land,
            };
            if !already {
                let cfg = SimConfig { episode_time: TRAIT_EPISODE_S, ..Default::default() };
                let r = CreatureSim::run_episode(decode(&genome), &self.planet, env, cfg);
                let speed = if r.exploded { 0.0 } else { r.distance / TRAIT_EPISODE_S };
                match env {
                    Environment::Water => {
                        traits.water_speed = speed;
                        traits.measured_water = true;
                    }
                    _ => {
                        traits.land_speed = speed;
                        traits.measured_land = true;
                    }
                }
                self.stats.trait_evals += 1;
                done += 1;
                self.trait_cache.insert(key.clone(), traits);
            }
            // Apply to every animal sharing the genome.
            for a in self.animals.iter_mut() {
                if a.key == key {
                    a.traits = traits;
                }
            }
        }
        self.stats.pending_evals = self.pending.len();
        done
    }

    fn neighbours(&self, p: usize) -> [usize; 4] {
        let n = self.n;
        let x = p % n;
        let y = p / n;
        [
            y * n + (x + 1) % n,
            y * n + (x + n - 1) % n,
            ((y + 1) % n) * n + x,
            ((y + n - 1) % n) * n + x,
        ]
    }

    pub fn tick(&mut self) {
        self.day += 1;
        let g = self.planet.gravity;
        let tf = temp_factor(self.planet.temp_c);
        let cf = co2_factor(self.planet.co2_frac);
        let light = self.planet.insolation;
        let aridity = self.derived.aridity;
        let n_patches = self.patches.len();

        // ---------------- Plants: light competition per patch ----------------
        let mut by_patch: Vec<Vec<usize>> = vec![Vec::new(); n_patches];
        for (i, p) in self.plants.iter().enumerate() {
            by_patch[p.patch].push(i);
        }
        let mut seeds: Vec<(PlantGenome, usize, f32)> = Vec::new();
        let mut dead_plants: Vec<usize> = Vec::new();
        for (pi, list) in by_patch.iter().enumerate() {
            if list.is_empty() {
                continue;
            }
            let is_water = self.patches[pi].water;
            // Structural check and realised height. Water supports the plant,
            // so submerged plants skip the stem check but stay near the surface.
            for &i in list {
                let p = &mut self.plants[i];
                let (lf, sf, _) = p.genome.fractions();
                let stem = p.biomass * sf;
                let above = p.biomass * (lf + sf);
                let grown = (p.biomass / p.genome.target_mass).clamp(0.0, 1.0).powf(1.0 / 3.0);
                let want = p.genome.height * grown.max(0.05);
                // Allowable height: stem strength over weight above, scaled by gravity.
                let allowed = if is_water { 1.5 } else { 60.0 * stem / (above * g + 1e-6) };
                p.height = want.min(allowed).max(0.02);
            }
            let mut order = list.clone();
            order.sort_by(|a, b| self.plants[*b].height.partial_cmp(&self.plants[*a].height).unwrap());
            let patch = self.patches[pi].clone();
            // Light arrives per square metre; `remaining` is in m^2 of full sun.
            // Water attenuates it, which is the pull toward land.
            let mut remaining = light * PATCH_AREA * if is_water { 0.25 } else { 1.0 };
            let mut nutrient_demand = 0.0;
            let mut intercepts = Vec::with_capacity(order.len());
            for &i in &order {
                let p = &self.plants[i];
                let (lf, _, _) = p.genome.fractions();
                let leaf_area = p.biomass * lf * p.genome.sla;
                let frac = 1.0 - (-0.5 * leaf_area / PATCH_AREA).exp();
                let got = remaining * frac;
                remaining -= got;
                intercepts.push((i, got, leaf_area));
                nutrient_demand += p.biomass * 0.002;
            }
            let nutrient_share = if nutrient_demand > 0.0 { (patch.nutrient / nutrient_demand).min(1.0) } else { 1.0 };
            let mut nutrient_used = 0.0;
            for (i, got, leaf_area) in intercepts {
                let p = &mut self.plants[i];
                let (_, _, rf) = p.genome.fractions();
                let root = p.biomass * rf;
                let tol = p.genome.land_tolerance;
                // Water: submerged plants are never water-limited; on land,
                // uptake needs land-adapted roots against transpiration demand.
                let water_frac = if patch.water {
                    1.0
                } else {
                    let uptake = root * patch.moisture * 0.6 * (0.3 + 0.7 * tol);
                    let demand = leaf_area * (0.002 + 0.015 * aridity) * (0.5 + tf);
                    if demand > 0.0 { (uptake / demand).min(1.0) } else { 1.0 }
                };
                // Unadapted tissue desiccates in air; adaptation costs upkeep.
                let habitat_eff = if patch.water { 1.0 } else { 0.25 + 0.75 * tol };
                // 0.008 kg dry mass per m^2 of full sun per day (~3 kg/m^2/yr).
                let photo = got * 0.008 * cf * tf * water_frac * habitat_eff * (0.3 + 0.7 * nutrient_share);
                let respiration = p.biomass * 0.004 * (0.3 + tf) * (1.0 + 0.3 * tol);
                p.energy += photo - respiration;
                p.light_frac = if light > 0.0 { got / (light * PATCH_AREA) } else { 0.0 };
                p.water_frac = water_frac;
                p.age += 1.0;
                nutrient_used += p.biomass * 0.002 * nutrient_share * 0.3;
                // Growth then seeds.
                if p.energy > 0.0 {
                    if p.biomass < p.genome.target_mass {
                        let grow = p.energy.min(p.biomass * 0.05 + 0.01);
                        p.biomass += grow;
                        p.energy -= grow;
                    }
                    let seed_cost = p.genome.seed_mass * 1.5;
                    if p.biomass >= p.genome.target_mass * 0.5 && p.energy > seed_cost * 3.0 {
                        let count = ((p.energy / seed_cost) as usize).min(6);
                        for _ in 0..count {
                            p.energy -= seed_cost;
                            let (child, _big) = p.genome.mutate(&mut self.rng);
                            let reach = p.genome.dispersal * (1.0 + 0.3 * p.height * (self.derived.air_density / 1.225).sqrt());
                            seeds.push((child, p.patch, reach));
                        }
                    }
                }
                if p.energy < -p.biomass * 0.3 || p.age > p.genome.lifespan {
                    dead_plants.push(i);
                }
            }
            let pt = &mut self.patches[pi];
            pt.nutrient = (pt.nutrient - nutrient_used).max(0.0);
        }
        // Seeds land within reach on land patches that have room.
        let mut counts: Vec<usize> = vec![0; n_patches];
        for p in &self.plants {
            counts[p.patch] += 1;
        }
        for (child, from, reach) in seeds {
            let n = self.n as i32;
            let dx = (self.rng.gen_range(-reach..=reach)).round() as i32;
            let dy = (self.rng.gen_range(-reach..=reach)).round() as i32;
            let x = ((from % self.n) as i32 + dx).rem_euclid(n) as usize;
            let y = ((from / self.n) as i32 + dy).rem_euclid(n) as usize;
            let target = y * self.n + x;
            if counts[target] >= MAX_PLANTS_PER_PATCH {
                self.patches[target].dead_matter += child.seed_mass;
                continue;
            }
            counts[target] += 1;
            let lineage = if self.rng.gen_bool(0.03) { self.new_lineage() } else { child.lineage };
            let mut child = child;
            child.lineage = lineage;
            self.plants.push(Plant {
                biomass: child.seed_mass,
                energy: child.seed_mass * 0.8,
                age: 0.0,
                height: 0.02,
                light_frac: 0.0,
                water_frac: 0.0,
                genome: child,
                patch: target,
            });
        }
        // Remove dead plants (descending index order).
        dead_plants.sort_unstable();
        dead_plants.dedup();
        for &i in dead_plants.iter().rev() {
            let p = self.plants.swap_remove(i);
            self.patches[p.patch].dead_matter += p.biomass;
        }

        // ---------------- Animals -------------------------------------------
        let o2 = self.derived.o2_rel.clamp(0.05, 3.0);
        let mut deaths: Vec<usize> = Vec::new();
        let mut births: Vec<(Genome, usize, f32, u32, usize)> = Vec::new();
        let mut killed: Vec<usize> = Vec::new();
        let n_animals = self.animals.len();
        // Animals per patch for predation lookups.
        let mut animals_in: Vec<Vec<usize>> = vec![Vec::new(); n_patches];
        for (i, a) in self.animals.iter().enumerate() {
            animals_in[a.patch].push(i);
        }
        let mut plants_in: Vec<Vec<usize>> = vec![Vec::new(); n_patches];
        for (i, p) in self.plants.iter().enumerate() {
            plants_in[p.patch].push(i);
        }
        let order: Vec<usize> = {
            let mut v: Vec<usize> = (0..n_animals).collect();
            for i in (1..v.len()).rev() {
                let j = self.rng.gen_range(0..=i);
                v.swap(i, j);
            }
            v
        };
        for &ai in &order {
            if killed.contains(&ai) {
                continue;
            }
            let (patch, mass, herb, carn, detr, speed, in_water, activity, section, freq) = {
                let a = &self.animals[ai];
                let (h, c, d) = a.genome.diet();
                let in_water = self.patches[a.patch].water;
                let speed = if in_water { a.traits.water_speed } else { a.traits.land_speed };
                (a.patch, a.traits.mass.max(0.05), h, c, d, speed, in_water, a.traits.activity, a.traits.section_area, a.traits.base_freq)
            };
            let m34 = mass.powf(0.75);
            // Costs: basal + activity (muscle section x frequency) + travel.
            let basal = 0.006 * m34 * (0.4 + tf) / o2.min(1.0).max(0.3);
            let activity_cost = 0.004 * section * freq * 50.0 * activity;
            let mut energy_delta = -(basal + activity_cost);

            // Foraging: search rate grows with speed. Drifting food in water
            // reaches a still animal; on land it has to walk to the plants.
            let search = (if in_water { 0.3 } else { 0.1 } + speed).min(4.0) * m34;
            let mut food = 0.0;
            if herb > 0.05 {
                let mut want = herb * search * 0.08;
                if in_water {
                    // Aquatic herbivores graze algae first; it regrows fastest.
                    let pt = &mut self.patches[patch];
                    let bite = want.min(pt.algae * 0.5);
                    pt.algae -= bite;
                    want -= bite;
                    food += bite * 0.6;
                }
                for &pi in &plants_in[patch] {
                    if want <= 0.0 {
                        break;
                    }
                    let p = &mut self.plants[pi];
                    if p.biomass < 0.05 {
                        continue; // seedlings are too small to find
                    }
                    let (lf, _, _) = p.genome.fractions();
                    let edible = (p.biomass - 0.05) * lf * 0.5;
                    let bite = want.min(edible);
                    p.biomass -= bite;
                    want -= bite;
                    food += bite * 0.6;
                }
            }
            if detr > 0.05 {
                let pt = &mut self.patches[patch];
                let bite = (detr * search * 0.12).min(pt.dead_matter * 0.5);
                pt.dead_matter -= bite;
                food += bite * 0.6;
            }
            if carn > 0.05 {
                // Attack one random neighbour in the patch.
                let candidates: Vec<usize> = animals_in[patch].iter().copied().filter(|&j| j != ai && !killed.contains(&j)).collect();
                if !candidates.is_empty() && self.rng.gen_bool((carn * 0.25) as f64) {
                    let j = candidates[self.rng.gen_range(0..candidates.len())];
                    let prey = &self.animals[j];
                    let prey_speed = if in_water { prey.traits.water_speed } else { prey.traits.land_speed };
                    let size_ok = prey.traits.mass < mass * 2.5;
                    let p_catch = 1.0 / (1.0 + (-(speed - prey_speed) * 2.0).exp());
                    if size_ok && self.rng.gen_bool((p_catch * 0.6) as f64) {
                        let meal = (prey.energy.max(0.0) + prey.traits.mass * 0.5) * 0.7;
                        food += meal.min(mass * 1.5);
                        self.patches[patch].dead_matter += prey.traits.mass * 0.5;
                        killed.push(j);
                        self.animals[ai].kills += 1;
                        self.stats.kills += 1;
                    }
                }
            }
            energy_delta += food;

            // Movement: leave if hungry, toward a random neighbour the body can use.
            let hungry = food < basal;
            if hungry && self.rng.gen_bool(0.6) {
                let nb = self.neighbours(patch);
                let target = nb[self.rng.gen_range(0..4)];
                let target_water = self.patches[target].water;
                let a = &self.animals[ai];
                let can = if target_water { a.traits.water_speed > 0.05 || !a.traits.measured_water } else { a.traits.land_speed > 0.05 || !a.traits.measured_land };
                if can {
                    // Travel cost: weight times distance over efficiency.
                    // ~10 J per kg per metre, in biomass units (17 MJ/kg).
                    energy_delta -= 0.00001 * mass * (g / 9.81) * 10.0 / (0.3 + speed).min(3.0);
                    self.animals[ai].patch = target;
                }
            }

            let a = &mut self.animals[ai];
            a.energy += energy_delta;
            a.age += 1.0;
            let threshold = 0.6 * a.traits.mass + 0.2;
            if a.energy > threshold && self.rng.gen_bool(0.5) {
                let (child, body_changed) = if self.rng.gen_bool(0.75) { a.genome.mutate(&mut self.rng) } else { (a.genome.clone(), false) };
                let share = a.energy * 0.35;
                a.energy -= share;
                a.offspring += 1;
                births.push((child, a.patch, share, a.lineage, body_changed as usize));
            }
            if a.energy < 0.0 || a.age > 400.0 + 200.0 * (a.traits.mass.ln().max(0.0)) {
                deaths.push(ai);
            }
        }
        // Resolve deaths and kills.
        let mut gone: Vec<usize> = deaths.into_iter().chain(killed.into_iter()).collect();
        gone.sort_unstable();
        gone.dedup();
        for &i in gone.iter().rev() {
            let a = self.animals.swap_remove(i);
            self.patches[a.patch].dead_matter += a.traits.mass * 0.5;
            self.stats.deaths += 1;
        }
        // Indices changed: rebuild pending list.
        self.pending.clear();
        for i in 0..self.animals.len() {
            self.request_traits(i);
        }
        // Births.
        for (genome, patch, energy, lineage, body_changed) in births {
            if self.animals.len() >= MAX_ANIMALS {
                self.patches[patch].dead_matter += energy;
                continue;
            }
            let lineage = if body_changed == 1 && self.rng.gen_bool(0.2) { self.new_lineage() } else { lineage };
            let traits = self.lookup_traits(&genome);
            let key = hash_genome(&genome);
            self.animals.push(Animal { genome, key, traits, patch, energy, age: 0.0, lineage, kills: 0, offspring: 0 });
            let idx = self.animals.len() - 1;
            self.request_traits(idx);
            self.stats.births += 1;
        }

        // ---------------- Decomposition and nutrient return ------------------
        for pt in self.patches.iter_mut() {
            let rate = 0.03 * (0.2 + tf) * (0.3 + 0.7 * pt.moisture);
            let dec = pt.dead_matter * rate;
            pt.dead_matter -= dec;
            pt.nutrient += dec * 0.8;
            // Slow weathering input and leaching.
            pt.nutrient = (pt.nutrient + 0.002 - pt.nutrient * 0.001).max(0.0);
            if pt.water {
                // Logistic algae growth driven by light, CO2 and temperature, fed by nutrient.
                let cap = 25.0 * (0.2 + (pt.nutrient / (pt.nutrient + 5.0)));
                let r = 0.12 * light * cf * tf;
                let growth = r * pt.algae * (1.0 - pt.algae / cap.max(0.1));
                pt.algae = (pt.algae + growth).max(0.05);
                pt.nutrient = (pt.nutrient - growth.max(0.0) * 0.02).max(0.0);
            }
        }

        // ---------------- Immigration keeps the world from emptying ----------
        if self.day % 60 == 0 && self.animals.len() < 40 {
            self.immigrate(8);
        }
        if self.day % 120 == 0 && self.plants.len() < self.patches.len() / 10 {
            self.seed_plants(self.patches.len() / 8);
        }

        self.update_stats();
    }

    fn update_stats(&mut self) {
        let mut s = EcoStats { day: self.day, ..self.stats.clone() };
        s.plants = self.plants.len();
        s.plant_biomass = self.plants.iter().map(|p| p.biomass).sum();
        s.animals = self.animals.len();
        s.herbivores = 0;
        s.carnivores = 0;
        s.detritivores = 0;
        s.aquatic = 0;
        s.land_plants = self.plants.iter().filter(|p| !self.patches[p.patch].water).count();
        s.land_patches = self.patches.iter().filter(|p| !p.water).count();
        {
            let mut veg = vec![false; self.patches.len()];
            for p in &self.plants {
                if !self.patches[p.patch].water && p.biomass > 0.05 {
                    veg[p.patch] = true;
                }
            }
            s.vegetated_land = veg.iter().filter(|v| **v).count();
        }
        s.land_animals = self.animals.iter().filter(|a| !self.patches[a.patch].water).count();
        for a in &self.animals {
            let (h, c, d) = a.genome.diet();
            if c >= h && c >= d {
                s.carnivores += 1;
            } else if d >= h {
                s.detritivores += 1;
            } else {
                s.herbivores += 1;
            }
            if self.patches[a.patch].water {
                s.aquatic += 1;
            }
        }
        let mut pl: Vec<u32> = self.plants.iter().map(|p| p.genome.lineage).collect();
        pl.sort_unstable();
        pl.dedup();
        s.plant_lineages = pl.len();
        let mut al: Vec<u32> = self.animals.iter().map(|a| a.lineage).collect();
        al.sort_unstable();
        al.dedup();
        s.animal_lineages = al.len();
        s.dead_matter = self.patches.iter().map(|p| p.dead_matter).sum();
        s.nutrient = self.patches.iter().map(|p| p.nutrient).sum();
        s.pending_evals = self.pending.len();
        self.stats = s;
        if self.day % 5 == 0 {
            self.history.push([
                self.stats.plant_biomass,
                self.stats.herbivores as f32,
                self.stats.carnivores as f32,
                self.stats.detritivores as f32,
                self.stats.animal_lineages as f32,
                self.stats.plant_lineages as f32,
                if self.stats.land_patches > 0 { self.stats.vegetated_land as f32 / self.stats.land_patches as f32 } else { 0.0 },
            ]);
            if self.history.len() > 600 {
                self.history.remove(0);
            }
        }
    }

    pub fn lineages(&self, max: usize) -> Vec<LineageSummary> {
        let mut map: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, a) in self.animals.iter().enumerate() {
            map.entry(a.lineage).or_default().push(i);
        }
        let mut out: Vec<LineageSummary> = map
            .into_iter()
            .map(|(id, idx)| {
                let c = idx.len() as f32;
                let mut s = LineageSummary {
                    id,
                    kind: "animal".to_string(),
                    count: idx.len(),
                    mean_mass: 0.0,
                    herb: 0.0,
                    carn: 0.0,
                    detr: 0.0,
                    aquatic_share: 0.0,
                    land_speed: 0.0,
                    water_speed: 0.0,
                    parts: 0.0,
                    membrane_area: 0.0,
                    lung_frac: 0.0,
                    height: 0.0,
                    sla: 0.0,
                    root_frac: 0.0,
                };
                for &i in &idx {
                    let a = &self.animals[i];
                    let (h, ca, d) = a.genome.diet();
                    s.mean_mass += a.traits.mass / c;
                    s.herb += h / c;
                    s.carn += ca / c;
                    s.detr += d / c;
                    s.aquatic_share += if self.patches[a.patch].water { 1.0 / c } else { 0.0 };
                    s.land_speed += a.traits.land_speed / c;
                    s.water_speed += a.traits.water_speed / c;
                    s.parts += decode(&a.genome).part_count() as f32 / c;
                    s.membrane_area += a.traits.membrane_area / c;
                    s.lung_frac += a.genome.lung_frac / c;
                }
                s
            })
            .collect();
        let mut pmap: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, p) in self.plants.iter().enumerate() {
            pmap.entry(p.genome.lineage).or_default().push(i);
        }
        let mut plants: Vec<LineageSummary> = pmap
            .into_iter()
            .map(|(id, idx)| {
                let c = idx.len() as f32;
                let mut s = LineageSummary {
                    id,
                    kind: "plant".to_string(),
                    count: idx.len(),
                    mean_mass: 0.0,
                    herb: 0.0,
                    carn: 0.0,
                    detr: 0.0,
                    aquatic_share: 0.0,
                    land_speed: 0.0,
                    water_speed: 0.0,
                    parts: 0.0,
                    membrane_area: 0.0,
                    lung_frac: 0.0,
                    height: 0.0,
                    sla: 0.0,
                    root_frac: 0.0,
                };
                for &i in &idx {
                    let p = &self.plants[i];
                    s.mean_mass += p.biomass / c;
                    s.height += p.height / c;
                    s.sla += p.genome.sla / c;
                    s.root_frac += p.genome.fractions().2 / c;
                    s.aquatic_share += if self.patches[p.patch].water { 1.0 / c } else { 0.0 };
                    s.lung_frac += p.genome.land_tolerance / c; // reused field: land tolerance for plants
                }
                s
            })
            .collect();
        out.sort_by(|a, b| b.count.cmp(&a.count));
        plants.sort_by(|a, b| b.count.cmp(&a.count));
        out.truncate(max);
        plants.truncate(max);
        out.extend(plants);
        out
    }

    /// Representative animal genome of a lineage (most offspring), with habitat.
    pub fn representative(&self, lineage: u32) -> Option<(Genome, Environment)> {
        self.animals
            .iter()
            .filter(|a| a.lineage == lineage)
            .max_by_key(|a| (a.offspring, (a.energy * 100.0) as i64))
            .map(|a| (a.genome.clone(), if self.patches[a.patch].water { Environment::Water } else { Environment::Land }))
    }

    pub fn snapshot(&self) -> Snapshot {
        let n = self.n;
        let mut veg = vec![0.0f32; n * n];
        let mut canopy = vec![0.0f32; n * n];
        for p in &self.plants {
            veg[p.patch] += p.biomass;
            canopy[p.patch] = canopy[p.patch].max(p.height);
        }
        let vegetation = veg
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let p = &self.patches[i];
                let x = if p.water { p.algae / 25.0 } else { v / 40.0 };
                (x.min(1.0) * 255.0) as u8
            })
            .collect();
        let canopy_height = canopy.iter().map(|h| ((h / 15.0).min(1.0) * 255.0) as u8).collect();
        let dead = self.patches.iter().map(|p| ((p.dead_matter / 20.0).min(1.0) * 255.0) as u8).collect();
        let water = self.patches.iter().map(|p| if p.water { 1 } else { 0 }).collect();
        let mut animals = Vec::with_capacity(self.animals.len() * 6);
        for a in &self.animals {
            let (h, c, d) = a.genome.diet();
            let class = if c >= h && c >= d { 1.0 } else if d >= h { 2.0 } else { 0.0 };
            animals.extend_from_slice(&[
                (a.patch % n) as f32,
                (a.patch / n) as f32,
                a.lineage as f32,
                a.traits.mass,
                class,
                if self.patches[a.patch].water { 1.0 } else { 0.0 },
            ]);
        }
        Snapshot { n, day: self.day, water, vegetation, dead, canopy_height, animals, stats: self.stats.clone(), lineages: self.lineages(8) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecology_runs_and_plants_persist() {
        let mut w = EcoWorld::new(PlanetParams::earth(), 16, 3);
        for _ in 0..200 {
            w.measure_pending(2);
            w.tick();
        }
        assert!(w.stats.plants > 0, "plants died out: {:?}", w.stats);
        assert!(w.stats.plant_biomass.is_finite());
        assert!(w.stats.trait_evals > 0);
        let snap = w.snapshot();
        assert_eq!(snap.vegetation.len(), 256);
    }

    #[test]
    fn gravity_limits_plant_height() {
        let mut earth = EcoWorld::new(PlanetParams::earth(), 12, 5);
        let mut heavy = PlanetParams::earth();
        heavy.gravity = 25.0;
        let mut big = EcoWorld::new(heavy, 12, 5);
        for _ in 0..300 {
            earth.tick();
            big.tick();
        }
        let h = |w: &EcoWorld| w.plants.iter().map(|p| p.height).fold(0.0f32, f32::max);
        assert!(h(&earth) > h(&big), "earth {} heavy {}", h(&earth), h(&big));
    }
}
