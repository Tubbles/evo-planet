//! Planet-scale ecology with implicit fitness on a spherical world.
//!
//! No score. Plants capture light, draw water and nutrients, pay respiration
//! and structural cost, and seed when they have surplus energy. Animals eat
//! plants, algae, dead matter or each other, pay basal and locomotion costs,
//! and reproduce when energy crosses a threshold. Dead things feed a detritus
//! pool that decomposers return to the nutrient pool. Whatever persists is
//! fit by definition.
//!
//! The world is a set of uniform patches on a Fibonacci sphere with a noise
//! elevation field. Sea level is the elevation quantile matching the planet's
//! water coverage. Each patch has a local temperature from latitude and
//! altitude, so peaks and poles are cold and barren.
//!
//! Life starts in the sea as floating algae sheets (one-membrane plant genomes)
//! and animals that can only graze them: carnivory, detritivory, stems, roots
//! and land tolerance all have to evolve. Plants reach land through a
//! land-tolerance gene and the wet shore band. Animals reach land because shore-dwelling herbivores
//! can graze adjacent land plants from the water, better the more they can
//! crawl, so any body that moves on land at all is rewarded before it ever
//! leaves the sea. Every new body is measured in both water and on land.
//!
//! Plants and animals share the graph genome in `genome`; plants are decoded
//! by `plant::decode_plant`, animals by `decode` plus a short rapier episode
//! per environment whose result is cached per genome.
//!
//! Units: one tick is one day; energy is kilograms of dry biomass
//! equivalent; a patch is roughly a 10 m square.

use crate::decode::decode;
use crate::genome::Genome;
use crate::planet::{Derived, PlanetParams};
use crate::plant::{decode_plant, PlantBody};
use crate::sim::{fatigue_factor, CreatureSim, Environment, SimConfig};
use crate::terrain;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const PATCH_AREA: f32 = 100.0;
pub const MAX_PLANTS_PER_PATCH: usize = 8;
/// Algae are small; the sea can hold more individuals per patch.
pub const MAX_PLANTS_PER_WATER_PATCH: usize = 16;
/// Global animal cap. High enough that predator and prey numbers can cycle
/// without the cap itself deciding who lives.
pub const MAX_ANIMALS: usize = 500;
pub const TRAIT_EPISODE_S: f32 = 3.0;
pub const NEIGHBOURS: usize = 6;

// ---------------------------------------------------------------------------
// Plants
// ---------------------------------------------------------------------------

/// A plant is a graph genome decoded as stems, leaves and roots (see
/// `plant::decode_plant`). The decoded body is cached on the individual.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plant {
    pub genome: Genome,
    #[serde(skip)]
    pub body: Option<PlantBody>,
    pub lineage: u32,
    pub patch: usize,
    pub biomass: f32,
    pub energy: f32,
    pub age: f32,
    /// Realised height at current size, metres.
    pub height: f32,
    pub light_frac: f32,
    pub water_frac: f32,
}

impl Plant {
    fn new(genome: Genome, lineage: u32, patch: usize, biomass: f32, energy: f32, gravity: f32) -> Self {
        let body = decode_plant(&genome, gravity);
        Self { genome, body: Some(body), lineage, patch, biomass, energy, age: 0.0, height: 0.02, light_frac: 0.0, water_frac: 0.0 }
    }
    pub fn body(&self) -> &PlantBody {
        self.body.as_ref().expect("plant body decoded")
    }
    /// Linear growth fraction at the current biomass.
    pub fn grown(&self) -> f32 {
        (self.biomass / self.body().target_mass).clamp(0.0, 1.0).powf(1.0 / 3.0)
    }
    pub fn leaf_area(&self) -> f32 {
        let g = self.grown();
        self.body().leaf_area * g * g
    }
    pub fn leaf_mass(&self) -> f32 {
        self.body().leaf_mass / self.body().target_mass * self.biomass
    }
    pub fn root_mass(&self) -> f32 {
        self.body().root_mass / self.body().target_mass * self.biomass
    }
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
    /// Adult body measured by physics; `mass` grows toward `traits.mass`.
    pub traits: Traits,
    pub patch: usize,
    pub lineage: u32,
    /// Current body mass, kg.
    pub mass: f32,
    /// Stored energy (fat), kg biomass equivalent. Death below zero.
    pub reserve: f32,
    /// Gut contents awaiting digestion, kg.
    pub gut_plant: f32,
    pub gut_meat: f32,
    pub age: f32,
    pub kills: u32,
    pub offspring: u32,
    /// Lifetime intake by source, for the realised diet class.
    pub ate_plant: f32,
    pub ate_meat: f32,
    pub ate_carrion: f32,
    /// Yesterday's energy gain, used as the foraging decision baseline.
    pub intake_rate: f32,
}

impl Animal {
    fn new(genome: Genome, key: String, traits: Traits, patch: usize, lineage: u32, mass: f32, reserve: f32) -> Self {
        Self {
            genome,
            key,
            traits,
            patch,
            lineage,
            mass: mass.max(0.001),
            reserve,
            gut_plant: 0.0,
            gut_meat: 0.0,
            age: 0.0,
            kills: 0,
            offspring: 0,
            ate_plant: 0.0,
            ate_meat: 0.0,
            ate_carrion: 0.0,
            intake_rate: 0.0,
        }
    }

    /// Realised diet fractions (plant, meat from kills, carrion). An animal
    /// that has not eaten yet is read from its gut type.
    pub fn diet(&self) -> (f32, f32, f32) {
        let total = self.ate_plant + self.ate_meat + self.ate_carrion;
        if total <= 1e-6 {
            let g = self.genome.gut_type.clamp(0.0, 1.0);
            return (1.0 - g, g * 0.5, g * 0.5);
        }
        (self.ate_plant / total, self.ate_meat / total, self.ate_carrion / total)
    }

    /// Plant and meat assimilation efficiencies from gut type.
    pub fn efficiencies(&self) -> (f32, f32) {
        let g = self.genome.gut_type.clamp(0.0, 1.0);
        (0.5 - 0.35 * g, 0.45 + 0.45 * g)
    }

    pub fn lifespan(&self) -> f32 {
        300.0 * self.traits.mass.max(0.01).powf(0.25) * self.genome.longevity.max(0.1)
    }
}

// ---------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Patch {
    /// Unit direction of the patch centre.
    pub pos: [f32; 3],
    /// Metres above sea level (negative is seabed).
    pub elevation: f32,
    /// Local mean temperature, C.
    pub temp_c: f32,
    pub water: bool,
    /// Land patch touching water.
    pub shore: bool,
    pub moisture: f32,
    pub nutrient: f32,
    /// Plant detritus, kg.
    pub dead_matter: f32,
    /// Dead animals, kg: richer than plant detritus and the bridge from
    /// scavenging to predation.
    pub carrion: f32,
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
    /// Animals with carnivory above 0.15 (counted in whichever class dominates).
    pub omnivores: usize,
    /// Realised diet means: fraction of lifetime intake from kills and from carrion.
    pub mean_carn: f32,
    pub mean_detr: f32,
    pub mean_mass: f32,
    pub mean_parts: f32,
    pub mean_gut: f32,
    pub mean_weapon: f32,
    pub mean_armour: f32,
    pub aquatic: usize,
    pub land_plants: usize,
    pub land_animals: usize,
    pub land_herbivores: usize,
    pub land_patches: usize,
    pub vegetated_land: usize,
    /// Fastest land speed among living animals, m/s.
    pub best_land_speed: f32,
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
    /// Plant lineages whose offspring were born motile, and the reverse.
    pub plant_to_animal: u64,
    pub animal_to_plant: u64,
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
    #[serde(default)]
    pub gut: f32,
    #[serde(default)]
    pub weapon: f32,
    #[serde(default)]
    pub armour: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub n_patches: usize,
    pub day: u64,
    pub water: Vec<u8>,
    pub vegetation: Vec<u8>,
    pub dead: Vec<u8>,
    pub canopy_height: Vec<u8>,
    /// patch, lineage, mass, diet class (0 herb 1 carn 2 detr), aquatic
    pub animals: Vec<f32>,
    pub stats: EcoStats,
    pub lineages: Vec<LineageSummary>,
}

/// Static render geometry: a displaced icosphere whose vertices know their
/// patch, plus the patch centres.
#[derive(Clone, Debug, Serialize)]
pub struct TerrainMesh {
    /// Unit directions, 3 per vertex (displacement is applied by the renderer).
    pub positions: Vec<f32>,
    pub indices: Vec<u32>,
    pub vertex_patch: Vec<u32>,
    /// Metres above sea level per vertex.
    pub vertex_elevation: Vec<f32>,
    pub vertex_temp: Vec<f32>,
    pub patch_positions: Vec<f32>,
    pub patch_elevation: Vec<f32>,
    pub patch_temp: Vec<f32>,
    pub patch_water: Vec<u8>,
}

pub struct EcoWorld {
    pub planet: PlanetParams,
    pub derived: Derived,
    pub patches: Vec<Patch>,
    pub neighbours: Vec<Vec<usize>>,
    pub plants: Vec<Plant>,
    pub animals: Vec<Animal>,
    pub day: u64,
    pub terrain_seed: u32,
    rng: StdRng,
    next_lineage: u32,
    trait_cache: HashMap<String, Traits>,
    pending: Vec<(usize, Environment)>,
    pub stats: EcoStats,
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
    /// `n_patches` uniform patches on the sphere (clamped to 200..6000).
    pub fn new(planet: PlanetParams, n_patches: usize, seed: u64) -> Self {
        let derived = planet.derive();
        let rng = StdRng::seed_from_u64(seed);
        let n = n_patches.clamp(200, 6000);
        let terrain_seed = (seed as u32) ^ 0xA5A5_5A5A;
        let positions = terrain::fibonacci_sphere(n);
        let neighbours = terrain::nearest_neighbours(&positions, NEIGHBOURS);

        // Elevation, then sea level from the water-coverage quantile.
        let raw: Vec<f32> = positions.iter().map(|p| terrain::elevation_at(*p, terrain_seed)).collect();
        let mut sorted = raw.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let k = (planet.water_coverage.clamp(0.0, 1.0) * n as f32) as usize;
        let sea_level = if k == 0 { f32::MIN } else { sorted[(k - 1).min(n - 1)] };

        let mut patches: Vec<Patch> = (0..n)
            .map(|i| {
                let elevation = raw[i] - sea_level;
                let water = elevation <= 0.0;
                let temp_c = terrain::local_temperature(planet.temp_c, positions[i], elevation);
                // Land moisture: humidity, drier with altitude.
                let moisture = if water {
                    1.0
                } else {
                    (planet.humidity * 0.8 * (1.0 - 0.45 * (elevation / 4000.0).clamp(0.0, 1.0))).clamp(0.02, 1.0)
                };
                Patch { pos: positions[i], elevation, temp_c, water, shore: false, moisture, nutrient: 2.0, dead_matter: 1.0, carrion: 0.0 }
            })
            .collect();
        // Shore band: land patches touching water stay wet, the first foothold for plants.
        for i in 0..n {
            if patches[i].water {
                continue;
            }
            if neighbours[i].iter().any(|&j| patches[j].water) {
                patches[i].shore = true;
                patches[i].moisture = patches[i].moisture.max(0.85);
            }
        }

        let mut world = Self {
            planet,
            derived,
            patches,
            neighbours,
            plants: Vec::new(),
            animals: Vec::new(),
            day: 0,
            terrain_seed,
            rng,
            next_lineage: 1,
            trait_cache: HashMap::new(),
            pending: Vec::new(),
            stats: EcoStats::default(),
            history: Vec::new(),
        };
        // Two algae per water patch on average, half grown: a sea worth grazing.
        let water_n = world.patches.iter().filter(|p| p.water).count().max(1);
        world.seed_plants(water_n * 2);
        // Founders scale with the sea so small worlds are not overgrazed on day one.
        world.immigrate((water_n / 12).clamp(12, 80));
        world
    }

    fn new_lineage(&mut self) -> u32 {
        let id = self.next_lineage;
        self.next_lineage += 1;
        id
    }

    fn water_patches(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.patches.len()).filter(|&i| self.patches[i].water).collect();
        if v.is_empty() {
            v = (0..self.patches.len()).collect();
        }
        v
    }

    fn seed_plants(&mut self, count: usize) {
        let pool = self.water_patches();
        for _ in 0..count {
            let lineage = self.new_lineage();
            let g = Genome::random_alga(&mut self.rng);
            // Life starts in the oceans as floating algae.
            let patch = pool[self.rng.gen_range(0..pool.len())];
            let gravity = self.planet.gravity;
            let mut plant = Plant::new(g, lineage, patch, 0.0, 0.0, gravity);
            plant.biomass = plant.body().target_mass * 0.5;
            plant.energy = plant.body().target_mass * 0.1;
            self.plants.push(plant);
        }
    }

    /// Add fresh random animals to the sea.
    pub fn immigrate(&mut self, count: usize) {
        let pool = self.water_patches();
        for _ in 0..count {
            if self.animals.len() >= MAX_ANIMALS {
                return;
            }
            let mut genome = Genome::random_with(&mut self.rng);
            // Everyone starts as a small algae grazer; predation, scavenging
            // and body size evolve from there.
            genome.diet_carn = 0.0;
            genome.diet_detr = 0.0;
            for node in genome.nodes.iter_mut() {
                node.len *= 0.45;
                node.r0 *= 0.45;
                node.r1 *= 0.45;
            }
            let lineage = self.new_lineage();
            let patch = pool[self.rng.gen_range(0..pool.len())];
            let traits = self.lookup_traits(&genome);
            let mass = traits.mass;
            let key = hash_genome(&genome);
            let reserve = (0.05 + genome.fat_frac) * mass * 0.8;
            self.animals.push(Animal::new(genome, key, traits, patch, lineage, mass, reserve));
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

    /// Queue physics measurements in both environments, so a sea creature's
    /// ability to crawl is visible to selection before it ever leaves the sea.
    fn request_traits(&mut self, idx: usize) {
        let t = self.animals[idx].traits;
        // Measure the current habitat first (popped last-in-first-out).
        let in_water = self.patches[self.animals[idx].patch].water;
        let order = if in_water { [Environment::Land, Environment::Water] } else { [Environment::Water, Environment::Land] };
        for env in order {
            let done = match env {
                Environment::Water => t.measured_water,
                _ => t.measured_land,
            };
            if !done && !self.pending.iter().any(|(i, e)| *i == idx && *e == env) {
                self.pending.push((idx, env));
            }
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
            for a in self.animals.iter_mut() {
                if a.key == key {
                    a.traits = traits;
                }
            }
        }
        self.stats.pending_evals = self.pending.len();
        done
    }

    /// Random walk of `hops` steps over the neighbour graph.
    fn disperse(&mut self, from: usize, reach: f32) -> usize {
        let mut p = from;
        let mut hops = reach.floor() as usize;
        if self.rng.gen_bool((reach.fract()) as f64) {
            hops += 1;
        }
        for _ in 0..hops {
            let nb = &self.neighbours[p];
            p = nb[self.rng.gen_range(0..nb.len())];
        }
        p
    }

    pub fn tick(&mut self) {
        self.day += 1;
        let g = self.planet.gravity;
        let cf = co2_factor(self.planet.co2_frac);
        let light = self.planet.insolation;
        let aridity = self.derived.aridity;
        let n_patches = self.patches.len();

        // ---------------- Plants: light competition per patch ----------------
        let mut by_patch: Vec<Vec<usize>> = vec![Vec::new(); n_patches];
        for (i, p) in self.plants.iter().enumerate() {
            by_patch[p.patch].push(i);
        }
        let mut seeds: Vec<(Genome, u32, usize, f32)> = Vec::new();
        let mut dead_plants: Vec<usize> = Vec::new();
        for (pi, list) in by_patch.iter().enumerate() {
            if list.is_empty() {
                continue;
            }
            let patch = self.patches[pi].clone();
            let is_water = patch.water;
            let tf = temp_factor(patch.temp_c);
            // Realised height at current size. Water supports the plant, so
            // submerged plants ignore the stem check but stay near the surface.
            for &i in list {
                let p = &mut self.plants[i];
                let want = p.body().height * p.grown().max(0.05);
                p.height = if is_water { want.min(1.5) } else { want }.max(0.02);
            }
            let mut order = list.clone();
            order.sort_by(|a, b| self.plants[*b].height.partial_cmp(&self.plants[*a].height).unwrap());
            // Light arrives per square metre; `remaining` is in m^2 of full sun.
            // Water attenuates it, which is the pull toward land.
            let mut remaining = light * PATCH_AREA * if is_water { 0.3 } else { 1.0 };
            let mut nutrient_demand = 0.0;
            let mut intercepts = Vec::with_capacity(order.len());
            for &i in &order {
                let p = &self.plants[i];
                let leaf_area = p.leaf_area();
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
                let root = p.root_mass();
                let tol = p.genome.land_tolerance;
                let sla = p.body().sla;
                // Water: submerged plants are never water-limited; on land,
                // uptake needs land-adapted roots against transpiration demand.
                // Thin leaves (high SLA) transpire more per area.
                let water_frac = if is_water {
                    1.0
                } else {
                    let uptake = root * patch.moisture * 0.6 * (0.3 + 0.7 * tol);
                    let demand = leaf_area * (0.002 + 0.015 * aridity) * (0.5 + tf) * (0.5 + (sla / 20.0).min(2.0));
                    if demand > 0.0 { (uptake / demand).min(1.0) } else { 1.0 }
                };
                // Unadapted tissue desiccates in air; adaptation costs upkeep.
                let habitat_eff = if is_water { 1.0 } else { 0.25 + 0.75 * tol };
                // 0.02 kg dry mass per m^2 of fully lit leaf per day; the patch's
                // Beer-Lambert light budget caps canopy production at ~2 kg/day.
                let photo = got * 0.02 * cf * tf * water_frac * habitat_eff * (0.3 + 0.7 * nutrient_share);
                // Leaves respire fast, woody and root tissue slowly.
                let leaf_m = p.leaf_mass();
                let respiration = (leaf_m * 0.004 + (p.biomass - leaf_m).max(0.0) * 0.0005) * (0.3 + tf) * (1.0 + 0.3 * tol);
                p.energy += photo - respiration;
                p.light_frac = if light > 0.0 { got / (light * PATCH_AREA) } else { 0.0 };
                p.water_frac = water_frac;
                p.age += 1.0;
                nutrient_used += p.biomass * 0.002 * nutrient_share * 0.3;
                // Growth up to the structural cap (water lifts the cap), then seeds.
                if p.energy > 0.0 {
                    let cap = p.body().target_mass * if is_water { 1.0 } else { p.body().growth_cap.powi(3) };
                    if p.biomass < cap {
                        let grow = p.energy.min(p.biomass * 0.05 + 0.01).min(cap - p.biomass);
                        p.biomass += grow;
                        p.energy -= grow;
                    }
                    let seed_cost = p.genome.seed_mass * 1.5;
                    if p.biomass >= cap * p.genome.repro_frac && p.energy > seed_cost * 1.5 {
                        let count = ((p.energy / seed_cost) as usize).min(3);
                        for _ in 0..count {
                            p.energy -= seed_cost;
                            let (child, _body_changed) = p.genome.mutate(&mut self.rng);
                            let reach = p.genome.dispersal * (1.0 + 0.3 * p.height * (self.derived.air_density / 1.225).sqrt());
                            seeds.push((child, p.lineage, p.patch, reach));
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
        // Seeds hop over the neighbour graph and land where there is room.
        let mut counts: Vec<usize> = vec![0; n_patches];
        for p in &self.plants {
            counts[p.patch] += 1;
        }
        let gravity = self.planet.gravity;
        for (child, parent_lineage, from, reach) in seeds {
            let target = self.disperse(from, reach);
            let cap = if self.patches[target].water { MAX_PLANTS_PER_WATER_PATCH } else { MAX_PLANTS_PER_PATCH };
            if counts[target] >= cap {
                self.patches[target].dead_matter += child.seed_mass;
                continue;
            }
            let lineage = if self.rng.gen_bool(0.03) { self.new_lineage() } else { parent_lineage };
            let seed_mass = child.seed_mass;
            if child.motility >= 0.5 {
                // A seed that moves: born as an animal, grazing from day one.
                if self.animals.len() < MAX_ANIMALS {
                    let traits = self.lookup_traits(&child);
                    let key = hash_genome(&child);
                    let mass0 = (child.offspring_frac * traits.mass).max(seed_mass);
                    self.animals.push(Animal::new(child, key, traits, target, lineage, mass0, mass0 * 0.3));
                    let idx = self.animals.len() - 1;
                    self.request_traits(idx);
                    self.stats.plant_to_animal += 1;
                    self.stats.births += 1;
                }
                continue;
            }
            counts[target] += 1;
            self.plants.push(Plant::new(child, lineage, target, seed_mass, seed_mass * 0.8, gravity));
        }
        // Remove dead plants (descending index order).
        dead_plants.sort_unstable();
        dead_plants.dedup();
        for &i in dead_plants.iter().rev() {
            let p = self.plants.swap_remove(i);
            self.patches[p.patch].dead_matter += p.biomass;
        }

        // ---------------- Animals -------------------------------------------
        // Mechanistic: allometric metabolism, gut and digestion, Holling
        // functional response, attack-versus-defence predation, profitability
        // decisions, growth and life history. See docs/MODEL.md.
        let o2 = self.derived.o2_rel.clamp(0.05, 3.0);
        let mut deaths: Vec<usize> = Vec::new();
        struct Birth {
            genome: Genome,
            patch: usize,
            lineage: u32,
            body_changed: bool,
            parent_traits: Traits,
            mass0: f32,
            reserve0: f32,
        }
        let mut births: Vec<Birth> = Vec::new();
        let mut killed: Vec<usize> = Vec::new();
        let n_animals = self.animals.len();
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
            // --- read state -------------------------------------------------
            let (patch, in_water, adult_mass, mass, motility, section, freq, activity, _gut_type, weapon, armour, fat_frac, gut_frac, membrane_area, water_speed_raw, land_speed_raw, measured_land, measured_water, eff_plant, eff_meat, prev_intake) = {
                let a = &self.animals[ai];
                let (ep, em) = a.efficiencies();
                (
                    a.patch,
                    self.patches[a.patch].water,
                    a.traits.mass.max(0.001),
                    a.mass.max(0.001),
                    a.genome.motility.clamp(0.0, 1.0),
                    a.traits.section_area,
                    a.traits.base_freq,
                    a.traits.activity,
                    a.genome.gut_type.clamp(0.0, 1.0),
                    a.genome.weapon.clamp(0.0, 1.0),
                    a.genome.armour.clamp(0.0, 1.0),
                    a.genome.fat_frac.clamp(0.02, 0.6),
                    a.genome.gut_frac.clamp(0.02, 0.4),
                    a.traits.membrane_area,
                    a.traits.water_speed,
                    a.traits.land_speed,
                    a.traits.measured_land,
                    a.traits.measured_water,
                    ep,
                    em,
                    a.intake_rate,
                )
            };
            let temp = self.patches[patch].temp_c;
            // Ectotherm: body temperature is patch temperature; Q10 = 2 around 20 C.
            let q = 2f32.powf((temp - 20.0) / 10.0).clamp(0.15, 3.0);
            let size_f = (mass / adult_mass).clamp(0.01, 1.0); // growth fraction
            let lin_f = size_f.powf(1.0 / 3.0); // juvenile linear scale
            let m34 = mass.powf(0.75);
            // Armour slows: a shell is dead weight for the muscles.
            let speed_here = (if in_water { water_speed_raw } else { land_speed_raw }) * motility * lin_f * (1.0 - 0.4 * armour);
            let land_speed = land_speed_raw * motility * lin_f * (1.0 - 0.4 * armour);

            // --- costs -------------------------------------------------------
            // Field metabolic rate: Kleiber exponent, ectotherm anchor, oxygen limit.
            let fmr = 0.006 * m34 * q / o2.min(1.0).max(0.3);
            let upkeep = fmr * 0.3 * (weapon + armour);
            let activity_cost = 0.004 * section * freq * 20.0 * activity * motility * size_f.powf(2.0 / 3.0);
            let mut delta = -(fmr + upkeep + activity_cost);
            if membrane_area > 0.0 && motility < 1.0 {
                let light_here = light * if in_water { 0.3 } else { 1.0 };
                delta += membrane_area * lin_f * lin_f * light_here * 0.02 * cf * temp_factor(temp) * 0.5 * (1.0 - motility);
            }

            // --- digestion of yesterday's gut contents -------------------------
            let gut_cap = gut_frac * mass;
            let digest_rate = 0.12 * m34 * q;
            {
                let a = &mut self.animals[ai];
                let total = a.gut_plant + a.gut_meat;
                if total > 0.0 {
                    let frac = (digest_rate / total).min(1.0);
                    let dp = a.gut_plant * frac;
                    let dm = a.gut_meat * frac;
                    a.gut_plant -= dp;
                    a.gut_meat -= dm;
                    delta += dp * eff_plant + dm * eff_meat;
                    // Undigested remainder returns to the patch as dung.
                    self.patches[patch].dead_matter += dp * (1.0 - eff_plant) + dm * (1.0 - eff_meat);
                }
            }
            let mut space = (gut_cap - (self.animals[ai].gut_plant + self.animals[ai].gut_meat)).max(0.0);

            // --- foraging options, each as a Holling type II intake rate ---------
            // Maximum intake (kg/day) scales with M^0.75: roughly a tenth of body
            // mass per day for a kilogram animal; handling time is its inverse.
            let bite_rate = (0.1 * m34 * q).max(1e-6);
            let handling = 1.0 / bite_rate;
            // Browsing reach: about half a metre for a 1 kg body, scaling with length.
            let reach_h = 0.5 * mass.powf(1.0 / 3.0);
            // Fraction of the patch's accessible food encountered per day: a
            // 10 m patch is covered in a day by anything that moves at all.
            let search = (if in_water { 0.5 } else { 0.2 }) + 2.0 * speed_here;

            // Plants here (accessible leaf mass), plus shore plants reachable from the water.
            let mut plant_targets: Vec<(usize, f32)> = Vec::new();
            let mut plant_b = 0.0f32;
            for &pi in &plants_in[patch] {
                let p = &self.plants[pi];
                if p.biomass < 0.004 {
                    continue;
                }
                let lf = p.body().leaf_mass / p.body().target_mass;
                // Browsing height matters on land; floating algae are all within reach.
                let browse = if in_water { 1.0 } else { (reach_h / p.height.max(0.02)).clamp(0.1, 1.0) };
                // Half the leaf tissue is takeable in a day; the plant lives on
                // through regrowth and seeding. (A defence gene belongs here.)
                let acc = (p.biomass - 0.004) * lf * 0.5 * browse;
                if acc > 0.0 {
                    plant_targets.push((pi, acc));
                    plant_b += acc;
                }
            }
            if in_water {
                // Shore grazing: reach plants on adjacent land, better the more one can crawl.
                let amph = 0.8 * (0.1 + 4.0 * land_speed).min(1.0);
                for &j in &self.neighbours[patch] {
                    if self.patches[j].water {
                        continue;
                    }
                    for &pi in &plants_in[j] {
                        let p = &self.plants[pi];
                        if p.biomass < 0.004 {
                            continue;
                        }
                        let lf = p.body().leaf_mass / p.body().target_mass;
                        let acc = (p.biomass - 0.004) * lf * 0.5 * amph;
                        if acc > 0.0 {
                            plant_targets.push((pi, acc));
                            plant_b += acc;
                        }
                    }
                }
            }
            let plant_rate = search * plant_b / (1.0 + search * handling * plant_b); // kg/day
            let carrion_here = self.patches[patch].carrion;
            let carrion_search = 0.5 + speed_here * 0.5; // carrion is found by smell
            let carrion_rate = carrion_search * carrion_here / (1.0 + carrion_search * handling * carrion_here);
            // Energy value per kg eaten, by gut.
            let plant_value = plant_rate * eff_plant;
            let carrion_value = carrion_rate * eff_meat;
            let alternative_rate = plant_value.max(carrion_value).max(prev_intake * 0.5);

            let mut food_plant = 0.0f32;
            let mut food_carrion = 0.0f32;
            let mut food_meat = 0.0f32;

            // --- predation: attack versus defence, decided by profitability -----
            if space > 0.0 && motility > 0.0 && speed_here > 0.0 {
                let candidates: Vec<usize> = animals_in[patch].iter().copied().filter(|&j| j != ai && !killed.contains(&j)).collect();
                let mut best: Option<(usize, f32, f32)> = None; // (idx, p_success, profitability)
                for _ in 0..candidates.len().min(3) {
                    let j = candidates[self.rng.gen_range(0..candidates.len())];
                    let prey = &self.animals[j];
                    let pm = prey.mass.max(0.001);
                    let p_in_water = self.patches[prey.patch].water;
                    let p_lin = (pm / prey.traits.mass.max(0.001)).clamp(0.01, 1.0).powf(1.0 / 3.0);
                    let prey_speed = (if p_in_water { prey.traits.water_speed } else { prey.traits.land_speed }) * prey.genome.motility.clamp(0.0, 1.0) * p_lin * (1.0 - 0.4 * prey.genome.armour.clamp(0.0, 1.0));
                    let attack = m34 * (0.5 + speed_here) * (0.5 + 1.5 * weapon);
                    let defence = pm.powf(0.75) * (0.5 + prey_speed) * (0.5 + 1.5 * prey.genome.armour.clamp(0.0, 1.0));
                    let scale = 0.25 * (attack + defence) + 1e-6;
                    let p_success = 1.0 / (1.0 + (-(attack - defence) / scale).exp());
                    let edible = pm * 0.8;
                    let meal_now = edible.min(space);
                    // Handling: a day to subdue plus eating time; big carcasses take days.
                    let handle = (0.2 + edible * handling).min(5.0);
                    let gain = meal_now * eff_meat + (edible - meal_now) * eff_meat * 0.3; // leftovers partly recoverable
                    let attempt_cost = 0.1 * fmr;
                    let profitability = (p_success * gain - attempt_cost) / handle;
                    if profitability > 0.0 && best.map(|(_, _, pr)| profitability > pr).unwrap_or(true) {
                        best = Some((j, p_success, profitability));
                    }
                }
                if let Some((j, p_success, profitability)) = best {
                    if profitability > alternative_rate {
                        delta -= 0.1 * fmr; // the attempt costs energy either way
                        if self.rng.gen_bool(p_success.clamp(0.0, 1.0) as f64) {
                            let prey_mass = self.animals[j].mass;
                            let prey_reserve = self.animals[j].reserve.max(0.0);
                            let edible = prey_mass * 0.8 + prey_reserve;
                            let meal = edible.min(space);
                            space -= meal;
                            food_meat += meal;
                            self.patches[patch].carrion += edible - meal + prey_mass * 0.2;
                            killed.push(j);
                            self.animals[ai].kills += 1;
                            self.stats.kills += 1;
                        }
                    }
                }
            }

            // --- eat: higher-value source first, limited by stomach space -------
            if space > 0.0 {
                let mut order2 = [(0u8, carrion_value), (1u8, plant_value)];
                if order2[1].1 > order2[0].1 {
                    order2.swap(0, 1);
                }
                for (kind, _) in order2 {
                    if space <= 0.0 {
                        break;
                    }
                    if kind == 0 && carrion_rate > 0.0 {
                        let take = carrion_rate.min(space).min(carrion_here);
                        self.patches[patch].carrion -= take;
                        space -= take;
                        food_carrion += take;
                    } else if kind == 1 && plant_rate > 0.0 && plant_b > 0.0 {
                        let mut take = plant_rate.min(space);
                        let scale_take = take / plant_b;
                        for (pi, acc) in &plant_targets {
                            let bite = acc * scale_take;
                            let p = &mut self.plants[*pi];
                            let bite = bite.min(p.biomass * 0.9);
                            p.biomass -= bite;
                            take -= bite;
                            food_plant += bite;
                        }
                        space -= food_plant;
                    }
                }
            }
            {
                let a = &mut self.animals[ai];
                a.gut_plant += food_plant;
                a.gut_meat += food_carrion + food_meat;
                a.ate_plant += food_plant;
                a.ate_meat += food_meat;
                a.ate_carrion += food_carrion;
            }

            // --- movement: when hungry, toward the best reachable neighbour --------
            let hungry = delta < 0.0 && self.animals[ai].reserve < 0.5 * (0.05 + fat_frac) * mass;
            if hungry && self.rng.gen_bool(0.6) {
                let directed = self.rng.gen_bool(0.5);
                let mut best: Option<(usize, f32)> = None;
                for &cand in &self.neighbours[patch] {
                    let cand_water = self.patches[cand].water;
                    let can = if cand_water { water_speed_raw * motility > 0.005 || !measured_water } else { land_speed_raw * motility > 0.005 || !measured_land };
                    if !can {
                        continue;
                    }
                    let value = if directed {
                        let plants_v: f32 = plants_in[cand].iter().map(|&pi| self.plants[pi].biomass).sum::<f32>() * eff_plant;
                        let carrion_v = self.patches[cand].carrion * eff_meat;
                        let prey_v: f32 = animals_in[cand].iter().map(|&j| self.animals[j].mass).sum::<f32>() * eff_meat * 0.2 * (0.5 + 1.5 * weapon);
                        plants_v + carrion_v + prey_v + self.rng.gen_range(0.0..0.02)
                    } else {
                        self.rng.gen_range(0.0..1.0)
                    };
                    if best.map(|(_, v)| value > v).unwrap_or(true) {
                        best = Some((cand, value));
                    }
                }
                if let Some((target, _)) = best {
                    let target_water = self.patches[target].water;
                    let use_speed = (if target_water { water_speed_raw } else { land_speed_raw }) * motility * lin_f;
                    // Cost of transport ~ M^-0.3 per kg per metre (Tucker), 10 m per patch.
                    delta -= 0.00001 * mass.powf(0.7) * (g / 9.81) * 10.0 / (0.3 + use_speed).min(3.0);
                    self.animals[ai].patch = target;
                }
            }

            // --- reserve, growth, reproduction, death -----------------------------
            let a = &mut self.animals[ai];
            a.intake_rate = (delta + fmr + upkeep + activity_cost).max(0.0); // gross gain today
            a.reserve += delta;
            a.age += 1.0;
            let cap = (0.05 + fat_frac) * a.mass;
            if a.reserve > cap {
                let surplus = a.reserve - cap;
                if a.mass < adult_mass {
                    // Ontogenetic growth: anabolism limited by M^0.75, 70% efficient.
                    let grow = surplus.min(0.05 * m34 * q).min(adult_mass - a.mass);
                    a.mass += grow * 0.7;
                    a.reserve -= grow;
                } else {
                    let mass0 = (a.genome.offspring_frac.clamp(0.01, 0.5) * adult_mass).max(0.001);
                    let cost = mass0 * 1.3; // body plus a starter reserve
                    if surplus >= cost && self.rng.gen_bool(0.5) {
                        a.reserve -= cost;
                        a.offspring += 1;
                        let (child, body_changed) = if self.rng.gen_bool(0.75) { a.genome.mutate(&mut self.rng) } else { (a.genome.clone(), false) };
                        births.push(Birth { genome: child, patch: a.patch, lineage: a.lineage, body_changed, parent_traits: a.traits, mass0, reserve0: mass0 * 0.3 });
                    }
                    // Beyond two offspring's worth of store, surplus is wasted as heat.
                    let max_store = cap + 2.0 * cost;
                    if a.reserve > max_store {
                        a.reserve = max_store;
                    }
                }
            }
            if a.reserve < 0.0 || a.age > a.lifespan() {
                deaths.push(ai);
            }
        }
        // Resolve deaths and kills.
        let mut gone: Vec<usize> = deaths.into_iter().chain(killed.into_iter()).collect();
        gone.sort_unstable();
        gone.dedup();
        for &i in gone.iter().rev() {
            let a = self.animals.swap_remove(i);
            self.patches[a.patch].carrion += a.mass * 0.8 + a.reserve.max(0.0);
            self.stats.deaths += 1;
        }
        // Indices changed: rebuild pending list.
        self.pending.clear();
        for i in 0..self.animals.len() {
            self.request_traits(i);
        }
        // Births.
        let mut plant_counts: Vec<usize> = vec![0; n_patches];
        for p in &self.plants {
            plant_counts[p.patch] += 1;
        }
        for b in births {
            let lineage = if b.body_changed && self.rng.gen_bool(0.2) { self.new_lineage() } else { b.lineage };
            if b.genome.motility < 0.5 {
                // A sessile child: settles where it was born and lives as a plant.
                let cap = if self.patches[b.patch].water { MAX_PLANTS_PER_WATER_PATCH } else { MAX_PLANTS_PER_PATCH };
                if plant_counts[b.patch] < cap {
                    plant_counts[b.patch] += 1;
                    let mut plant = Plant::new(b.genome, lineage, b.patch, 0.0, 0.0, self.planet.gravity);
                    plant.biomass = b.mass0.min(plant.body().target_mass).max(0.002);
                    plant.energy = b.reserve0;
                    self.plants.push(plant);
                    self.stats.animal_to_plant += 1;
                    self.stats.births += 1;
                } else {
                    self.patches[b.patch].carrion += b.mass0;
                }
                continue;
            }
            if self.animals.len() >= MAX_ANIMALS {
                self.patches[b.patch].carrion += b.mass0;
                continue;
            }
            // Same body as the parent: inherit its measured speeds instead of
            // spending physics on it (controller-only mutations are approximated).
            let key = hash_genome(&b.genome);
            if !b.body_changed {
                self.trait_cache.entry(key.clone()).or_insert(b.parent_traits);
            }
            let traits = self.lookup_traits(&b.genome);
            self.animals.push(Animal::new(b.genome, key, traits, b.patch, lineage, b.mass0, b.reserve0));
            let idx = self.animals.len() - 1;
            self.request_traits(idx);
            self.stats.births += 1;
        }

        // ---------------- Decomposition and nutrient return ------------------
        for pt in self.patches.iter_mut() {
            let tf = temp_factor(pt.temp_c);
            let rate = 0.03 * (0.2 + tf) * (0.3 + 0.7 * pt.moisture);
            let dec = pt.dead_matter * rate;
            pt.dead_matter -= dec;
            pt.nutrient += dec * 0.8;
            let rot = pt.carrion * (rate * 3.0).min(0.5);
            pt.carrion -= rot;
            pt.nutrient += rot * 0.8;
            // Slow weathering input and leaching.
            pt.nutrient = (pt.nutrient + 0.002 - pt.nutrient * 0.001).max(0.0);
        }

        // ---------------- Immigration keeps the sea from emptying ------------
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
        s.land_herbivores = 0;
        s.best_land_speed = 0.0;
        s.omnivores = 0;
        s.mean_carn = 0.0;
        s.mean_detr = 0.0;
        s.mean_mass = 0.0;
        s.mean_parts = 0.0;
        let na = self.animals.len().max(1) as f32;
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
        s.land_animals = 0;
        s.mean_gut = 0.0;
        s.mean_weapon = 0.0;
        s.mean_armour = 0.0;
        for a in &self.animals {
            let (h, c, d) = a.diet();
            s.mean_gut += a.genome.gut_type / na;
            s.mean_weapon += a.genome.weapon / na;
            s.mean_armour += a.genome.armour / na;
            let on_land = !self.patches[a.patch].water;
            if c >= h && c >= d {
                s.carnivores += 1;
            } else if d >= h {
                s.detritivores += 1;
            } else {
                s.herbivores += 1;
                if on_land {
                    s.land_herbivores += 1;
                }
            }
            if on_land {
                s.land_animals += 1;
            } else {
                s.aquatic += 1;
            }
            s.best_land_speed = s.best_land_speed.max(a.traits.land_speed);
            {
                let mut v = [h, c, d];
                v.sort_by(|x, y| y.partial_cmp(x).unwrap());
                if v[1] >= 0.25 {
                    s.omnivores += 1;
                }
            }
            s.mean_carn += c / na;
            s.mean_detr += d / na;
            s.mean_mass += a.mass / na;
            s.mean_parts += a.genome.structural_size() as f32 / na;
        }
        let mut pl: Vec<u32> = self.plants.iter().map(|p| p.lineage).collect();
        pl.sort_unstable();
        pl.dedup();
        s.plant_lineages = pl.len();
        let mut al: Vec<u32> = self.animals.iter().map(|a| a.lineage).collect();
        al.sort_unstable();
        al.dedup();
        s.animal_lineages = al.len();
        s.dead_matter = self.patches.iter().map(|p| p.dead_matter + p.carrion).sum();
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

    fn empty_summary(id: u32, kind: &str, count: usize) -> LineageSummary {
        LineageSummary {
            id,
            kind: kind.to_string(),
            count,
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
            gut: 0.0,
            weapon: 0.0,
            armour: 0.0,
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
                let mut s = Self::empty_summary(id, "animal", idx.len());
                for &i in &idx {
                    let a = &self.animals[i];
                    let (h, ca, d) = a.diet();
                    s.mean_mass += a.mass / c;
                    s.gut += a.genome.gut_type / c;
                    s.weapon += a.genome.weapon / c;
                    s.armour += a.genome.armour / c;
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
            pmap.entry(p.lineage).or_default().push(i);
        }
        let mut plants: Vec<LineageSummary> = pmap
            .into_iter()
            .map(|(id, idx)| {
                let c = idx.len() as f32;
                let mut s = Self::empty_summary(id, "plant", idx.len());
                for &i in &idx {
                    let p = &self.plants[i];
                    s.mean_mass += p.biomass / c;
                    s.height += p.height / c;
                    s.sla += p.body().sla / c;
                    s.root_frac += p.body().root_mass / p.body().target_mass / c;
                    s.aquatic_share += if self.patches[p.patch].water { 1.0 / c } else { 0.0 };
                    s.lung_frac += p.genome.land_tolerance / c; // reused field: land tolerance for plants
                    s.parts += p.body().part_count as f32 / c;
                    s.membrane_area += p.body().leaf_area / c; // reused field: full-grown leaf area
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
            .max_by_key(|a| (a.offspring, (a.reserve * 100.0) as i64))
            .map(|a| (a.genome.clone(), if self.patches[a.patch].water { Environment::Water } else { Environment::Land }))
    }

    /// Largest living plant of a lineage, as its genome.
    pub fn plant_representative(&self, lineage: u32) -> Option<Genome> {
        self.plants
            .iter()
            .filter(|p| p.lineage == lineage)
            .max_by(|a, b| a.biomass.partial_cmp(&b.biomass).unwrap())
            .map(|p| p.genome.clone())
    }

    pub fn snapshot(&self) -> Snapshot {
        let n = self.patches.len();
        let mut veg = vec![0.0f32; n];
        let mut canopy = vec![0.0f32; n];
        for p in &self.plants {
            veg[p.patch] += p.biomass;
            canopy[p.patch] = canopy[p.patch].max(p.height);
        }
        let vegetation = veg
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let p = &self.patches[i];
                let x = if p.water { v / 2.0 } else { v / 8.0 };
                (x.min(1.0) * 255.0) as u8
            })
            .collect();
        let canopy_height = canopy.iter().map(|h| ((h / 15.0).min(1.0) * 255.0) as u8).collect();
        let dead = self.patches.iter().map(|p| (((p.dead_matter + p.carrion) / 20.0).min(1.0) * 255.0) as u8).collect();
        let water = self.patches.iter().map(|p| if p.water { 1 } else { 0 }).collect();
        let mut animals = Vec::with_capacity(self.animals.len() * 5);
        for a in &self.animals {
            let (h, c, d) = a.diet();
            let class = if c >= h && c >= d { 1.0 } else if d >= h { 2.0 } else { 0.0 };
            animals.extend_from_slice(&[a.patch as f32, a.lineage as f32, a.mass, class, if self.patches[a.patch].water { 1.0 } else { 0.0 }]);
        }
        Snapshot { n_patches: n, day: self.day, water, vegetation, dead, canopy_height, animals, stats: self.stats.clone(), lineages: self.lineages(8) }
    }

    /// Render geometry at the given icosphere detail (5 = 10k vertices, 6 = 41k).
    pub fn terrain_mesh(&self, detail: u32) -> TerrainMesh {
        let ico = terrain::icosphere(detail.clamp(2, 7));
        let centres: Vec<[f32; 3]> = self.patches.iter().map(|p| p.pos).collect();
        let sea_level_raw = {
            // Recover raw sea level from any patch: raw = elevation + sea_level.
            let p = &self.patches[0];
            terrain::elevation_at(p.pos, self.terrain_seed) - p.elevation
        };
        let mut positions = Vec::with_capacity(ico.positions.len() * 3);
        let mut vertex_patch = Vec::with_capacity(ico.positions.len());
        let mut vertex_elevation = Vec::with_capacity(ico.positions.len());
        let mut vertex_temp = Vec::with_capacity(ico.positions.len());
        for v in &ico.positions {
            positions.extend_from_slice(v);
            let patch = terrain::nearest_patch(&centres, *v);
            vertex_patch.push(patch as u32);
            let e = terrain::elevation_at(*v, self.terrain_seed) - sea_level_raw;
            vertex_elevation.push(e);
            vertex_temp.push(terrain::local_temperature(self.planet.temp_c, *v, e));
        }
        TerrainMesh {
            positions,
            indices: ico.indices,
            vertex_patch,
            vertex_elevation,
            vertex_temp,
            patch_positions: self.patches.iter().flat_map(|p| p.pos.iter().copied()).collect(),
            patch_elevation: self.patches.iter().map(|p| p.elevation).collect(),
            patch_temp: self.patches.iter().map(|p| p.temp_c).collect(),
            patch_water: self.patches.iter().map(|p| if p.water { 1 } else { 0 }).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecology_runs_and_plants_persist() {
        let mut w = EcoWorld::new(PlanetParams::earth(), 300, 3);
        for _ in 0..200 {
            w.measure_pending(2);
            w.tick();
        }
        assert!(w.stats.plants > 0, "plants died out: {:?}", w.stats);
        assert!(w.stats.plant_biomass.is_finite());
        assert!(w.stats.trait_evals > 0);
        let snap = w.snapshot();
        assert_eq!(snap.vegetation.len(), 300);
        let mesh = w.terrain_mesh(3);
        assert_eq!(mesh.vertex_patch.len(), 642);
    }

    #[test]
    fn sea_level_matches_water_coverage_and_peaks_are_cold() {
        let w = EcoWorld::new(PlanetParams::earth(), 1000, 11);
        let water = w.patches.iter().filter(|p| p.water).count();
        assert!((water as f32 / 1000.0 - 0.71).abs() < 0.03, "{water}");
        let highest = w.patches.iter().max_by(|a, b| a.elevation.partial_cmp(&b.elevation).unwrap()).unwrap();
        let shore = w.patches.iter().filter(|p| p.shore).map(|p| p.temp_c).sum::<f32>() / w.patches.iter().filter(|p| p.shore).count().max(1) as f32;
        assert!(highest.temp_c < shore, "peak {} shore {}", highest.temp_c, shore);
    }

    #[test]
    fn plants_colonise_land_from_the_sea() {
        // A mid-sized world: tiny seas get overgrazed by founders before algae establish.
        let mut w = EcoWorld::new(PlanetParams::earth(), 800, 5);
        for _ in 0..2500 {
            w.tick();
        }
        assert!(w.stats.plants > 0);
        assert!(w.stats.land_plants > 0, "no land plants after 2500 days: {:?}", w.stats);
    }
}
