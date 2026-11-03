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

use crate::decode::{body_metrics, decode};
use crate::genome::Genome;
use std::f32::consts::PI;
use crate::planet::{Derived, PlanetParams};
use crate::plant::{decode_plant, PlantBody};
use crate::sim::{fatigue_factor, CreatureSim, Environment, SimConfig};
use crate::terrain;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A patch is 100 m², the scale the individual-plant model is sized for. A
/// body whose daily path sweeps more than its patch ranges into the
/// neighbours (see the home-range spill in the animal tick), so speed keeps
/// paying beyond the patch instead of saturating at two centimetres a second.
pub const PATCH_AREA: f32 = 100.0;
/// Everything below is per square metre or per metre; the cell size only
/// decides how finely the planet is cut (see EcoWorld::patch_area).
pub const NUTRIENT0_PER_M2: f32 = 0.02;
pub const DEAD0_PER_M2: f32 = 0.01;
/// Individual-plant density caps (performance guards), per m^2.
pub const PLANT_CAP_PER_M2_WATER: f32 = 0.48;
pub const PLANT_CAP_PER_M2_LAND: f32 = 0.08;
/// Beddington-DeAngelis interference per competitor per 100 m^2.
pub const INTERFERENCE_PER_100M2: f32 = 0.15;
/// Founding grazers per square metre of sea.
pub const FOUNDERS_PER_M2: f32 = 1.0 / 1200.0;
/// Farthest a hungry walker or swimmer relocates in a day, metres.
pub const RELOCATE_MAX_M: f32 = 50.0;
/// Osmotrophy: kg absorbed per m^2 of surface per day per kg/m^2 of dead matter.
pub const OSMOTROPHY_RATE: f32 = 0.3;
/// Larval drift from the birth site, metres.
pub const LARVAL_DRIFT_M: f32 = 15.0;
/// The seed dispersal gene is in tens of metres.
pub const SEED_DISPERSAL_UNIT_M: f32 = 10.0;
/// Share of a water patch's plant mass that is suspended (drift, plankton)
/// and so reachable by filtering a current rather than by travelling.
pub const SUSPENDED_FRACTION: f32 = 0.2;
/// Water cleared by a filter feeder, m^3 per kg of body per day.
pub const CLEARANCE_M3_PER_KG_DAY: f32 = 1.2;
/// Global animal cap. High enough that predator and prey numbers can cycle
/// without the cap itself deciding who lives.
/// Performance guard only. Ecological limits come from food, crowding and
/// predation (Beddington-DeAngelis interference), not from this number.
pub const MAX_ANIMALS: usize = 5000;
pub const TRAIT_EPISODE_S: f32 = 4.0;
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
        Self::new_with(genome, lineage, patch, biomass, energy, gravity, None)
    }
    fn new_with(genome: Genome, lineage: u32, patch: usize, biomass: f32, energy: f32, gravity: f32, body: Option<PlantBody>) -> Self {
        let body = body.unwrap_or_else(|| decode_plant(&genome, gravity));
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
    /// Sustained flight speed, m/s: set only when the body is still aloft at
    /// the end of the air-drop episode (powered or very efficient flight).
    pub air_speed: f32,
    /// Nimbleness: yaw rate gained from asymmetric drive (rad/s) and vertical
    /// speed gained from a posture shift (m/s), per environment. Zero unless
    /// the body locomotes at all.
    pub turn_land: f32,
    pub turn_water: f32,
    pub climb_water: f32,
    pub turn_air: f32,
    pub climb_air: f32,
    /// Beach climb probe (not a gate any more): metres up a dry beach under
    /// its own power. Only measured by audits.
    pub shore: f32,
    pub measured_shore: bool,
    /// Geometry-derived function (see decode::body_metrics).
    pub reach: f32,
    pub mouth_area: f32,
    pub claws: f32,
    pub plating: f32,
    /// Mean centre-of-mass height on land relative to half the reach, 0..1.
    pub upright: f32,
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
        150.0 * self.traits.mass.max(0.01).powf(0.25) * self.genome.longevity.max(0.1)
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
    /// Tallest plant in the patch, metres: the launch height for gliders.
    #[serde(default)]
    pub canopy: f32,
    /// Fraction of the day the patch is under water: 1 at sea, 0.5 on the
    /// intertidal shore (waves and tides), 0 inland. Breathing, light, water
    /// supply and locomotion all blend on it instead of switching.
    #[serde(default)]
    pub submersion: f32,
    /// Fraction of the day's light left under the plant canopy (what a
    /// swimming photosynthesiser gets), set by the plant tick.
    #[serde(default)]
    pub light_left: f32,
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
    /// Animals with a measured sustained air speed.
    #[serde(default)]
    pub fliers: usize,
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
    /// Daily energy audit (kg biomass equivalent per day, last tick).
    pub primary_production: f32,
    pub grazing_intake: f32,
    pub predation_intake: f32,
    pub scavenging_intake: f32,
    pub metabolic_cost: f32,
    pub deaths_starved: u64,
    pub deaths_old: u64,
    #[serde(default)]
    pub deaths_suffocated: u64,
    /// Sum of log10(predator mass / prey mass) over all kills; mean = sum / kills.
    pub ppmr_log_sum: f32,
    pub kills_by_larger: u64,
    /// Fraction of animals with a gut at least 80% full (Jeschke: ~88% satiated in the field).
    pub satiated_frac: f32,
    pub mean_toughness: f32,
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
    /// Claws (pad section relative to trunk) and plating (rigid mass fraction).
    #[serde(default)]
    pub weapon: f32,
    #[serde(default)]
    pub armour: f32,
    #[serde(default)]
    pub reach: f32,
    #[serde(default)]
    pub upright: f32,
    #[serde(default)]
    pub air_speed: f32,
    /// Mean glide distance beyond a thrown stone from a 10 m launch, metres.
    #[serde(default)]
    pub glide: f32,
    /// Nimbleness means: turn rate (rad/s) in the lineage's main habitat, climb speed (m/s) in water.
    #[serde(default)]
    pub turn: f32,
    #[serde(default)]
    pub climb: f32,
    /// Shore test mean (m up the beach) and breathing gene mean (0 gills, 1 lungs).
    #[serde(default)]
    pub shore: f32,
    #[serde(default)]
    pub breathing: f32,
    /// Share of members on intertidal shore patches.
    #[serde(default)]
    pub shore_share: f32,
    /// Speed in body lengths per second (length ~ 2 x reach), main habitat.
    #[serde(default)]
    pub speed_bl: f32,
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
    /// Area of one patch, m^2: a discretisation choice, not a model parameter.
    pub patch_area: f32,
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
        Self::new_with_area(planet, n_patches, seed, PATCH_AREA)
    }

    /// Build a world with a chosen cell size (m^2). Results should not depend
    /// on it beyond discretisation noise; `evo_headless --patch-area` tests that.
    pub fn new_with_area(planet: PlanetParams, n_patches: usize, seed: u64, patch_area: f32) -> Self {
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
                Patch { pos: positions[i], elevation, temp_c, water, shore: false, moisture, nutrient: NUTRIENT0_PER_M2 * patch_area, dead_matter: DEAD0_PER_M2 * patch_area, carrion: 0.0, canopy: 0.0, submersion: if water { 1.0 } else { 0.0 }, light_left: 1.0 }
            })
            .collect();
        // Shore band: land patches touching water stay wet, the first foothold for plants.
        for i in 0..n {
            if patches[i].water {
                continue;
            }
            if neighbours[i].iter().any(|&j| patches[j].water) {
                patches[i].shore = true;
                patches[i].submersion = 0.5;
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
            patch_area,
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
        // Founders per area of sea (about one per 1,200 m^2), not per cell.
        let sea_area = water_n as f32 * patch_area;
        // No founding animals: life starts as algae and heterotrophy has to
        // arise by itself (osmotrophy, then motility, then mouths).
        let _ = sea_area;
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

    /// Warm shallow seas: water patches next to land with a mild climate.
    /// Life starts where production is highest; nothing is seeded on ice.
    /// Individual-plant cap of a patch from the density cap and cell size.
    fn plant_cap(&self, patch: usize) -> usize {
        let d = if self.patches[patch].submersion >= 0.5 { PLANT_CAP_PER_M2_WATER } else { PLANT_CAP_PER_M2_LAND };
        ((d * self.patch_area).round() as usize).max(1)
    }

    fn cradle_patches(&self) -> Vec<usize> {
        let shallow: Vec<usize> = (0..self.patches.len())
            .filter(|&i| self.patches[i].water && self.patches[i].temp_c > 12.0 && self.neighbours[i].iter().any(|&j| !self.patches[j].water))
            .collect();
        if !shallow.is_empty() {
            return shallow;
        }
        let warm: Vec<usize> = (0..self.patches.len()).filter(|&i| self.patches[i].water && self.patches[i].temp_c > 5.0).collect();
        if !warm.is_empty() {
            return warm;
        }
        self.water_patches()
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

    /// Add fresh random animals to the warm shallow sea.
    pub fn immigrate(&mut self, count: usize) {
        let pool = self.cradle_patches();
        for _ in 0..count {
            if self.animals.len() >= MAX_ANIMALS {
                return;
            }
            let mut genome = Genome::random_with(&mut self.rng);
            // Everyone starts as a small algae grazer; predation, scavenging
            // and body size evolve from there.
            genome.diet_carn = 0.0;
            genome.diet_detr = 0.0;
            // The first grazers were tiny next to the producers they ate: scale
            // every dimension to about a fifth, a few hundred grams of animal.
            for node in genome.nodes.iter_mut() {
                node.len *= 0.2;
                node.r0 *= 0.2;
                node.r1 *= 0.2;
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
        let bm = body_metrics(&pheno);
        let t = Traits {
            mass: pheno.total_mass,
            activity: fatigue_factor(&pheno, &self.derived),
            // Muscle work: cross-section of driven joints weighted by how far
            // and how often they are swung. A rigid strut or a limp limb costs
            // only its mass.
            section_area: pheno
                .parts
                .iter()
                .skip(1)
                .filter(|p| p.joint != crate::genome::JointKind::Rigid)
                .map(|p| p.section_area * p.cpg.freq_mult * (p.cpg.amp.abs().min(p.limit) / 0.8).min(1.5))
                .sum::<f32>(),
            base_freq: pheno.base_freq,
            membrane_area: pheno.membrane_area,
            reach: bm.reach,
            mouth_area: bm.mouth_area,
            claws: bm.claws,
            plating: bm.plating,
            upright: 0.3,
            ..Default::default()
        };
        self.trait_cache.insert(key, t);
        t
    }

    /// Change the planet under a running world. Sea level follows water
    /// coverage (flooded land drowns its plants, drained seabed strands its
    /// animals), temperature and moisture follow the new climate, plant
    /// structural caps follow gravity, and every body's physics traits are
    /// queued for re-measurement when gravity, pressure or oxygen changed.
    pub fn set_planet(&mut self, planet: PlanetParams) {
        let old = std::mem::replace(&mut self.planet, planet);
        self.derived = self.planet.derive();
        let n = self.patches.len();
        // Raw elevation recovered from any patch, then the new sea level.
        let old_sea = terrain::elevation_at(self.patches[0].pos, self.terrain_seed) - self.patches[0].elevation;
        let raw: Vec<f32> = self.patches.iter().map(|p| p.elevation + old_sea).collect();
        let mut sorted = raw.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let k = (self.planet.water_coverage.clamp(0.0, 1.0) * n as f32) as usize;
        let sea_level = if k == 0 { f32::MIN } else { sorted[(k - 1).min(n - 1)] };
        let mut flooded: Vec<bool> = vec![false; n];
        for i in 0..n {
            let elevation = raw[i] - sea_level;
            let water = elevation <= 0.0;
            flooded[i] = water && !self.patches[i].water;
            let p = &mut self.patches[i];
            p.elevation = elevation;
            p.water = water;
            p.temp_c = terrain::local_temperature(self.planet.temp_c, p.pos, elevation);
            p.moisture = if water {
                1.0
            } else {
                (self.planet.humidity * 0.8 * (1.0 - 0.45 * (elevation / 4000.0).clamp(0.0, 1.0))).clamp(0.02, 1.0)
            };
            p.shore = false;
            p.submersion = if water { 1.0 } else { 0.0 };
        }
        for i in 0..n {
            if !self.patches[i].water && self.neighbours[i].iter().any(|&j| self.patches[j].water) {
                self.patches[i].shore = true;
                self.patches[i].submersion = 0.5;
                self.patches[i].moisture = self.patches[i].moisture.max(0.85);
            }
        }
        // Land plants under new sea die; everything else adjusts its cap.
        let g = self.planet.gravity;
        let mut i = 0;
        while i < self.plants.len() {
            if flooded[self.plants[i].patch] {
                let p = self.plants.swap_remove(i);
                self.patches[p.patch].dead_matter += p.biomass;
                continue;
            }
            if let Some(b) = self.plants[i].body.as_mut() {
                b.set_gravity(g);
            }
            i += 1;
        }
        let physics_changed = (old.gravity - self.planet.gravity).abs() > 0.01 * old.gravity
            || (old.pressure_atm - self.planet.pressure_atm).abs() > 0.01 * old.pressure_atm
            || (old.o2_frac - self.planet.o2_frac).abs() > 0.01 * old.o2_frac.max(0.01);
        if physics_changed {
            // Keep the old numbers as priors; re-measure every living body.
            for t in self.trait_cache.values_mut() {
                t.measured_land = false;
                t.measured_water = false;
                t.measured_air = false;
                t.measured_shore = false;
            }
            let derived = self.derived;
            for a in self.animals.iter_mut() {
                a.traits.measured_land = false;
                a.traits.measured_water = false;
                a.traits.measured_air = false;
                a.traits.measured_shore = false;
                a.traits.activity = fatigue_factor(&decode(&a.genome), &derived);
            }
            self.pending.clear();
            for idx in 0..self.animals.len() {
                self.pending.push((idx, Environment::Land));
                self.pending.push((idx, Environment::Water));
            }
        }
        self.stats.pending_evals = self.pending.len();
    }

    /// Queue physics measurements in both environments, so a sea creature's
    /// ability to crawl is visible to selection before it ever leaves the sea.
    fn request_traits(&mut self, idx: usize) {
        let t = self.animals[idx].traits;
        // Most young die before their speed matters: measure at half adult mass.
        if self.animals[idx].mass < 0.5 * t.mass && (t.measured_land || t.measured_water || self.animals[idx].age > 0.0) {
            return;
        }
        // Measure the current habitat first (popped last-in-first-out).
        let in_water = self.patches[self.animals[idx].patch].water;
        let order = if in_water { [Environment::Land, Environment::Water] } else { [Environment::Water, Environment::Land] };
        // Wings are only worth testing on a body light enough to be lifted:
        // wing loading below 50 kg/m^2 (bats and birds span 1-20).
        let winged = t.membrane_area > 0.0 && t.membrane_area / t.mass.max(1e-3) >= 0.02;
        if winged && !t.measured_air && !self.pending.iter().any(|(i, e)| *i == idx && *e == Environment::Air) {
            self.pending.push((idx, Environment::Air));
        }
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
        if budget > 0 && self.pending.len() > 1 {
            // Measure the bodies that matter most first: one request per
            // (body, environment), ordered by how many living animals share it,
            // so a body that is spreading on inherited speeds is checked soon.
            let mut copies: HashMap<String, usize> = HashMap::new();
            for a in &self.animals {
                *copies.entry(a.key.clone()).or_insert(0) += 1;
            }
            let mut seen: HashSet<(String, Environment)> = HashSet::new();
            let mut keep: Vec<(usize, Environment)> = Vec::with_capacity(self.pending.len());
            for &(idx, env) in self.pending.iter().rev() {
                if idx >= self.animals.len() {
                    continue;
                }
                if seen.insert((self.animals[idx].key.clone(), env)) {
                    keep.push((idx, env));
                }
            }
            // Most-copied bodies (an unmeasured body that is spreading) and the
            // newest bodies (where selection information lives) take turns, so a
            // fresh body plan is measured soon instead of last.
            keep.sort_by_key(|&(idx, _)| copies.get(&self.animals[idx].key).copied().unwrap_or(0));
            let mut order: Vec<(usize, Environment)> = Vec::with_capacity(keep.len());
            let (mut lo, mut hi) = (0usize, keep.len());
            let mut take_new = false;
            while lo < hi {
                if take_new {
                    order.push(keep[lo]);
                    lo += 1;
                } else {
                    hi -= 1;
                    order.push(keep[hi]);
                }
                take_new = !take_new;
            }
            // Popped from the back: reverse so the first chosen is popped first.
            order.reverse();
            self.pending = order;
        }
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
                Environment::Air => traits.measured_air,
                Environment::Shore => traits.measured_shore,
                _ => traits.measured_land,
            };
            if !already {
                let pheno = decode(&genome);
                let cfg = SimConfig { episode_time: TRAIT_EPISODE_S, ..Default::default() };
                let r = CreatureSim::run_episode(pheno.clone(), &self.planet, env, cfg);
                // Sustained progress only, and only what the muscles add: the same
                // body is run again with its motors off (sham control) and that
                // passive drift, a buoyant rise with a tilted fin or a slide after
                // a topple, is subtracted. The air drop has its own control (a stone).
                let mut passive_shore = 0.0;
                let passive = if env == Environment::Air || r.exploded {
                    0.0
                } else {
                    let sham = SimConfig { episode_time: TRAIT_EPISODE_S, motors: false, ..Default::default() };
                    let c = CreatureSim::run_episode(pheno, &self.planet, env, sham);
                    passive_shore = if c.exploded { 0.0 } else { c.shore_progress };
                    if c.exploded { 0.0 } else { c.sustained_speed }
                };
                let speed = if r.exploded { 0.0 } else { (r.sustained_speed - passive).max(0.0) };
                // Nimbleness, only for bodies that locomote: the same body driven
                // asymmetrically (turning) and with a shifted posture (climb/dive).
                let (turn, climb) = if speed > 0.01 && !r.exploded && env != Environment::Shore {
                    let pheno2 = decode(&genome);
                    let st = SimConfig { episode_time: TRAIT_EPISODE_S, steer: 0.6, ..Default::default() };
                    let rs = CreatureSim::run_episode(pheno2.clone(), &self.planet, env, st);
                    let turn = if rs.exploded { 0.0 } else { (rs.yaw_rate - r.yaw_rate).abs() };
                    let climb = if env == Environment::Land {
                        0.0
                    } else {
                        let bc = SimConfig { episode_time: TRAIT_EPISODE_S, bias_shift: 0.4, ..Default::default() };
                        let rc = CreatureSim::run_episode(pheno2, &self.planet, env, bc);
                        if rc.exploded { 0.0 } else { (rc.climb_speed - r.climb_speed).abs() }
                    };
                    (turn, climb)
                } else {
                    (0.0, 0.0)
                };
                match env {
                    Environment::Shore => {
                        traits.shore = if r.exploded { 0.0 } else { (r.shore_progress - passive_shore).max(0.0) };
                        traits.measured_shore = true;
                    }
                    Environment::Water => {
                        traits.water_speed = speed;
                        traits.turn_water = turn;
                        traits.climb_water = climb;
                        traits.measured_water = true;
                    }
                    Environment::Air => {
                        // Dropped from 10 m: a body still aloft for most of the episode
                        // and moving forward is flying or gliding, not falling.
                        // How far the body gets through the air in the episode from a
                        // 10 m launch: a brick scores nothing, a parachuter little, a glider
                        // metres per second, a flapper more. Continuous, so wings can grow.
                        // Launched at 3 m/s from 10 m: metres beyond where a stone lands are
                        // the wings' doing (glide ratio = that / 10). Still aloft at the
                        // end of the episode means sustained flight at the measured speed.
                        let beyond = (r.air_distance - crate::sim::ballistic_distance(self.planet.gravity)).max(0.0);
                        traits.glide_distance = if r.exploded { 0.0 } else { beyond };
                        traits.air_speed = if !r.exploded && r.airborne_frac >= 0.95 && r.distance > 5.0 { r.distance / TRAIT_EPISODE_S } else { 0.0 };
                        traits.turn_air = turn;
                        traits.climb_air = climb;
                        traits.measured_air = true;
                    }
                    _ => {
                        traits.land_speed = speed;
                        traits.turn_land = turn;
                        traits.measured_land = true;
                        traits.upright = (r.mean_height / (0.5 * traits.reach).max(0.01)).clamp(0.0, 1.0);
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

    /// Larval drift: a random walk that stays in the birth patch's medium
    /// (water larvae drift through water, land young stay on land).
    fn disperse_same_habitat(&mut self, from: usize, reach: f32) -> usize {
        let wet = self.patches[from].submersion > 0.0;
        let mut p = from;
        let mut hops = reach.floor() as usize;
        if self.rng.gen_bool((reach.fract()) as f64) {
            hops += 1;
        }
        for _ in 0..hops {
            let nb: Vec<usize> = self.neighbours[p].iter().copied().filter(|&k| (self.patches[k].submersion > 0.0) == wet).collect();
            if nb.is_empty() {
                break;
            }
            p = nb[self.rng.gen_range(0..nb.len())];
        }
        p
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
        let a = self.patch_area;
        let a_area = a;
        let patch_w = a.sqrt();
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
        let mut seeds: Vec<(Genome, u32, usize, f32, Option<PlantBody>)> = Vec::new();
        let mut dead_plants: Vec<usize> = Vec::new();
        let mut production_today = 0.0f32;
        for (pi, list) in by_patch.iter().enumerate() {
            if list.is_empty() {
                continue;
            }
            let patch = self.patches[pi].clone();
            let is_water = patch.water;
            let sub = patch.submersion;
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
            let mut remaining = light * a * (0.3 * sub + (1.0 - sub));
            let mut nutrient_demand = 0.0;
            let mut intercepts = Vec::with_capacity(order.len());
            for &i in &order {
                let p = &self.plants[i];
                let leaf_area = p.leaf_area();
                let frac = 1.0 - (-0.5 * leaf_area / a).exp();
                let got = remaining * frac;
                remaining -= got;
                intercepts.push((i, got, leaf_area));
                nutrient_demand += p.biomass * 0.002;
            }
            let light_left = if light > 0.0 { (remaining / (light * a * (0.3 * sub + (1.0 - sub)))).clamp(0.0, 1.0) } else { 0.0 };
            let nutrient_share = if nutrient_demand > 0.0 { (patch.nutrient / nutrient_demand).min(1.0) } else { 1.0 };
            let mut nutrient_used = 0.0;
            let mut litter_today = 0.0;
            // Osmotrophy: organisms absorb dead organic matter through their surface
            // in proportion to their heterotrophy gene; the pool is shared.
            let dom_density = patch.dead_matter / a; // kg/m^2
            let mut dom_demand = 0.0f32;
            for &i in &order {
                let p = &self.plants[i];
                dom_demand += p.genome.heterotrophy.clamp(0.0, 1.0) * p.leaf_area() * dom_density * OSMOTROPHY_RATE;
            }
            let dom_share = if dom_demand > 0.0 { (patch.dead_matter * 0.5 / dom_demand).min(1.0) } else { 1.0 };
            let mut dom_used = 0.0f32;
            for (i, got, leaf_area) in intercepts {
                let p = &mut self.plants[i];
                let root = p.root_mass();
                let tol = p.genome.land_tolerance;
                let sla = p.body().sla;
                // Water: submerged plants are never water-limited; on land,
                // uptake needs land-adapted roots against transpiration demand.
                // Thin leaves (high SLA) transpire more per area.
                let water_frac = if sub >= 1.0 {
                    1.0
                } else {
                    let uptake = root * patch.moisture * 0.6 * (0.3 + 0.7 * tol);
                    let demand = leaf_area * (0.002 + 0.015 * aridity) * (0.5 + tf) * (0.5 + (sla / 20.0).min(2.0));
                    let dry = if demand > 0.0 { (uptake / demand).min(1.0) } else { 1.0 };
                    sub + (1.0 - sub) * dry
                };
                // Unadapted tissue desiccates in air; adaptation costs upkeep.
                let habitat_eff = sub + (1.0 - sub) * (0.25 + 0.75 * tol);
                // 0.02 kg dry mass per m^2 of fully lit leaf per day; the patch's
                // Beer-Lambert light budget caps canopy production at ~2 kg/day.
                // Tough, long-lived leaves photosynthesise slower per area (leaf economics).
                let tough = p.genome.leaf_toughness.clamp(0.0, 1.0);
                let het = p.genome.heterotrophy.clamp(0.0, 1.0);
                // Mixotroph's trade-off: absorption machinery displaces chloroplasts.
                let photo = got * 0.02 * cf * tf * water_frac * habitat_eff * (0.3 + 0.7 * nutrient_share) * (1.0 - 0.35 * tough) * (1.0 - 0.6 * het);
                let absorbed = het * p.leaf_area() * dom_density * OSMOTROPHY_RATE * dom_share * (0.3 + tf);
                dom_used += absorbed;
                // Leaves respire fast, woody and root tissue slowly.
                let leaf_m = p.leaf_mass();
                // Transport: feeding leaves through long stems costs upkeep per kg*m.
                let transport = p.body().transport * (p.biomass / p.body().target_mass).clamp(0.0, 1.0);
                let respiration = (leaf_m * 0.004 + (p.biomass - leaf_m).max(0.0) * 0.0005 + transport * 0.002) * (0.3 + tf) * (1.0 + 0.3 * tol);
                production_today += photo;
                p.energy += photo + absorbed * 0.6 - respiration;
                // Leaf economics: thin leaves live weeks, thick ones years
                // (Wright et al. 2004: lifespan ~ LMA^1.5, a year at 0.1 kg/m^2).
                // Shed leaves are litter and must be regrown from energy.
                if leaf_m > 0.0 && sla > 0.0 {
                    let lma = 1.0 / sla;
                    let leaf_lifespan = (365.0 * (lma / 0.1).powf(1.5)).clamp(20.0, 2000.0);
                    let shed = leaf_m / leaf_lifespan;
                    p.biomass = (p.biomass - shed).max(1e-5);
                    litter_today += shed;
                }
                p.light_frac = if light > 0.0 { got / (light * a) } else { 0.0 };
                p.water_frac = water_frac;
                p.age += 1.0;
                nutrient_used += p.biomass * 0.002 * nutrient_share * 0.3;
                // Growth up to the structural cap (water lifts the cap), then seeds.
                if p.energy > 0.0 {
                    let cap = p.body().target_mass * (sub + (1.0 - sub) * p.body().growth_cap.powi(3));
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
                            let (child, body_changed) = p.genome.mutate(&mut self.rng);
                            let reach = p.genome.dispersal * SEED_DISPERSAL_UNIT_M / patch_w * (1.0 + 0.3 * p.height * (self.derived.air_density / 1.225).sqrt());
                            // Same body as the parent: reuse its decoded structure.
                            let body = if body_changed { None } else { Some(p.body().clone()) };
                            seeds.push((child, p.lineage, p.patch, reach, body));
                        }
                    }
                }
                if p.energy < -p.biomass * 0.3 || p.age > p.genome.lifespan {
                    dead_plants.push(i);
                }
            }
            let pt = &mut self.patches[pi];
            pt.nutrient = (pt.nutrient - nutrient_used).max(0.0);
            pt.dead_matter = (pt.dead_matter - dom_used).max(0.0) + litter_today;
            pt.light_left = light_left;
        }
        // Seeds hop over the neighbour graph and land where there is room.
        let mut counts: Vec<usize> = vec![0; n_patches];
        for p in &self.plants {
            counts[p.patch] += 1;
        }
        let gravity = self.planet.gravity;
        for (child, parent_lineage, from, reach, body) in seeds {
            let target = self.disperse(from, reach);
            let cap = self.plant_cap(target);
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
            self.plants.push(Plant::new_with(child, lineage, target, seed_mass, seed_mass * 0.8, gravity, body));
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
        let (mut graze_today, mut prey_today, mut carrion_today, mut cost_today) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let n_animals = self.animals.len();
        let mut animals_in: Vec<Vec<usize>> = vec![Vec::new(); n_patches];
        for (i, a) in self.animals.iter().enumerate() {
            animals_in[a.patch].push(i);
        }
        for p in self.patches.iter_mut() {
            p.canopy = 0.0;
        }
        for p in &self.plants {
            let pt = &mut self.patches[p.patch];
            pt.canopy = pt.canopy.max(p.height);
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
            let (patch, in_water, adult_mass, mass, motility, section, freq, activity, _gut_type, _weapon, _armour, fat_frac, gut_frac, membrane_area, water_speed_raw, land_speed_raw, measured_land, measured_water, eff_plant, eff_meat, prev_intake) = {
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
            let (reach_adult, mouth_area, claws, plating, upright, air_speed_raw, measured_air, glide_ratio) = {
                let t = &self.animals[ai].traits;
                let sb = self.patches[patch].submersion;
                (t.reach.max(0.02), t.mouth_area.max(1e-5), t.claws, t.plating, sb + (1.0 - sb) * t.upright, t.air_speed, t.measured_air, t.glide_distance / crate::sim::AIR_DROP_HEIGHT)
            };
            // Launch height here: the canopy plus the drop to the lowest neighbour.
            let launch_h = {
                let e = self.patches[patch].elevation;
                let drop = self.neighbours[patch].iter().map(|&k| e - self.patches[k].elevation).fold(0.0f32, f32::max);
                (self.patches[patch].canopy + drop.min(50.0)).max(0.0)
            };
            // Metres a glider covers in a day from about twenty launches.
            let glide_day = if in_water { 0.0 } else { 20.0 * glide_ratio * launch_h };
            // Gas exchange: gills work in water, lungs in air; halfway is poor in
            // both. On the intertidal shore the animal is under water half the
            // day, so a gill-breather lives there while lungs slowly pay.
            let sub = self.patches[patch].submersion;
            let breathing = self.animals[ai].genome.breathing.clamp(0.0, 1.0);
            let resp_eff = sub * (1.0 - 0.9 * breathing) + (1.0 - sub) * (0.1 + 0.9 * breathing);
            let temp = self.patches[patch].temp_c;
            // Ectotherm: body temperature is patch temperature; Q10 = 2 around 20 C.
            let q = 2f32.powf((temp - 20.0) / 10.0).clamp(0.15, 3.0);
            let size_f = (mass / adult_mass).clamp(0.01, 1.0); // growth fraction
            let lin_f = size_f.powf(1.0 / 3.0); // juvenile linear scale
            let m34 = mass.powf(0.75);
            // Plating slows: rigid mass is dead weight for the muscles.
            // On the shore the day is part swum, part crawled.
            let speed_here = (water_speed_raw * sub + land_speed_raw * (1.0 - sub)) * motility * lin_f * (1.0 - 0.3 * plating);
            let land_speed = land_speed_raw * motility * lin_f * (1.0 - 0.3 * plating);
            // Flight: over land a flier covers ground at its air speed (plating
            // is dead weight here too). It lands to eat, so bite and reach are unchanged.
            let air_speed = air_speed_raw * motility * lin_f * (1.0 - 0.3 * plating);
            let travel = if in_water { speed_here } else { speed_here.max(air_speed) };

            // --- costs -------------------------------------------------------
            // Field metabolic rate: Kleiber exponent, ectotherm anchor, oxygen limit.
            let fmr = 0.006 * m34 * q / o2.min(1.0).max(0.3);
            let upkeep = 0.0f32; // claws and plating are real parts and already cost mass
            // Short of oxygen, the muscles cannot be kept working all day.
            let activity = activity * resp_eff;
            let activity_cost = 0.004 * section * freq * 20.0 * activity * motility * size_f.powf(2.0 / 3.0);
            let mut delta = -(fmr + upkeep + activity_cost);
            // A motile organism with membranes still photosynthesises (Euglena,
            // not a sudden animal): the same leaf chemistry as a plant, halved for
            // a surface that is not held flat to the sun, and displaced by the
            // heterotrophy gene like a mixotroph's chloroplasts.
            {
                let het = self.animals[ai].genome.heterotrophy.clamp(0.0, 1.0);
                let leaf_now = membrane_area * lin_f * lin_f;
                if leaf_now > 0.0 && het < 1.0 {
                    let tf = temp_factor(temp);
                    // Under the plant canopy's leftover light, like any understorey.
                    let photo = (1.0 - 0.6 * het) * leaf_now * light * self.patches[patch].light_left * 0.02 * cf * tf * (0.3 * sub + (1.0 - sub)) * 0.5;
                    delta += photo;
                    production_today += photo;
                }
            }
            cost_today += fmr + upkeep + activity_cost;
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
            // Bite rate from the mouth: trunk cross-section (juveniles scaled),
            // about a tenth of body mass per day for a well-proportioned kilogram.
            let bite_rate = (12.0 * mouth_area * lin_f * lin_f * q * resp_eff).max(1e-6);
            let handling = 1.0 / bite_rate;
            // Browsing reach from the body's actual extent; on land only an
            // upright body gets its full reach.
            let reach_h = reach_adult * lin_f * (0.3 + 0.7 * upright);
            // Fraction of the patch's food encountered per day is the area swept:
            // a detection strip two reaches wide along the day's path (8 h of
            // foraging, cut by fatigue), plus the circle a still body can reach.
            let reach_now = reach_adult * lin_f;
            let path = (travel * 28800.0 * activity).max(glide_day);
            let range_area = PI * reach_now * reach_now + 2.0 * reach_now * path; // m^2 swept today
            let search = (range_area / a).min(1.0);
            // Filter feeding: a fifth of the sea's plant mass is drifting fragments
            // and plankton, and a body clears about a cubic metre of water per
            // kilogram per day (mussels and sponges: 1-10 L/h per 100 g), from a
            // patch two metres deep. Proportional to mass, so no scale advantage;
            // a supplement at fish-level metabolism, not a free living.
            let filtered = (CLEARANCE_M3_PER_KG_DAY * mass * sub / (a * 2.0)).min(1.0);
            let search = (search + SUSPENDED_FRACTION * filtered).min(1.0);

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
                // Tender leaves are half takeable in a day; tough ones resist grazing.
                let tough = p.genome.leaf_toughness.clamp(0.0, 1.0);
                let acc = (p.biomass - 0.004) * lf * 0.5 * (1.0 - 0.8 * tough) * browse;
                if acc > 0.0 {
                    plant_targets.push((pi, acc));
                    plant_b += acc;
                }
            }
            // Home range: once the day's path has swept this patch, the rest of
            // it sweeps the neighbours it could live in, so a faster body feeds
            // from a wider range (up to the six neighbours, a 700 m^2 range).
            let swept_patches = range_area / a;
            // Cells beyond this one that the range covers, ring by ring, each
            // ring taken in proportion to how much of it the range fills; up to
            // four rings so the reachable range does not depend on cell size.
            let mut extra = (swept_patches - 1.0).max(0.0);
            let mut ring_cells: Vec<(usize, f32)> = Vec::new(); // (cell, coverage)
            let mut seen = vec![patch];
            let mut frontier = vec![patch];
            for _ring in 0..4 {
                if extra <= 0.0 {
                    break;
                }
                let mut next = Vec::new();
                for &q in &frontier {
                    for &k in &self.neighbours[q] {
                        if !seen.contains(&k) {
                            seen.push(k);
                            next.push(k);
                        }
                    }
                }
                if next.is_empty() {
                    break;
                }
                let cover = (extra / next.len() as f32).min(1.0);
                for &k in &next {
                    ring_cells.push((k, cover));
                }
                extra -= next.len() as f32 * cover;
                frontier = next;
            }
            let spill = if ring_cells.is_empty() { 0.0 } else { 1.0 };
            for &(j, spill) in &ring_cells {
                {
                    let js = self.patches[j].submersion;
                    // Same medium or the shore in between; dry land needs legs.
                    let reachable = (js > 0.0) == (sub > 0.0) || js == 0.5 || sub == 0.5;
                    if !reachable || (js == 0.0 && land_speed_raw * motility <= 0.005) {
                        continue;
                    }
                    for &pi in &plants_in[j] {
                        let p = &self.plants[pi];
                        if p.biomass < 0.004 {
                            continue;
                        }
                        let lf = p.body().leaf_mass / p.body().target_mass;
                        let browse = if js >= 1.0 { 1.0 } else { (reach_h / p.height.max(0.02)).clamp(0.1, 1.0) };
                        let tough = p.genome.leaf_toughness.clamp(0.0, 1.0);
                        let acc = (p.biomass - 0.004) * lf * 0.5 * (1.0 - 0.8 * tough) * browse * spill;
                        if acc > 0.0 {
                            plant_targets.push((pi, acc));
                            plant_b += acc;
                        }
                    }
                }
            }
            if in_water && spill <= 0.0 {
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
                        let tough = p.genome.leaf_toughness.clamp(0.0, 1.0);
                        let acc = (p.biomass - 0.004) * lf * 0.5 * (1.0 - 0.8 * tough) * amph;
                        if acc > 0.0 {
                            plant_targets.push((pi, acc));
                            plant_b += acc;
                        }
                    }
                }
            }
            // Beddington-DeAngelis: conspecific crowding in the patch interferes with feeding.
            let crowd = (animals_in[patch].len().saturating_sub(1)) as f32;
            let interference = INTERFERENCE_PER_100M2 * crowd * 100.0 / a;
            let plant_rate = search * plant_b / (1.0 + search * handling * plant_b + interference); // kg/day
            let carrion_here = self.patches[patch].carrion;
            // Carrion is smelt from afar: the detection strip is three times wider.
            // A flier surveys from height: a far wider strip (vultures find carcasses from kilometres up).
            let strip = if sub < 1.0 && air_speed > 0.0 { 40.0 } else { 6.0 };
            let carrion_search = ((PI * reach_now * reach_now + strip * reach_now * path) / a).min(1.0);
            let carrion_rate = carrion_search * carrion_here / (1.0 + carrion_search * handling * carrion_here + interference);
            // Energy value per kg eaten, by gut.
            let plant_value = plant_rate * eff_plant;
            let carrion_value = carrion_rate * eff_meat;
            let alternative_rate = plant_value.max(carrion_value).max(prev_intake * 0.5);

            let mut food_plant = 0.0f32;
            let mut food_carrion = 0.0f32;
            let mut food_meat = 0.0f32;

            // --- predation: attack versus defence, decided by profitability -----
            if space > 0.0 && motility > 0.0 && speed_here > 0.0 {
                // Prey in this cell, plus prey in the ring cells the day's range covers
                // (each kept with probability equal to that cell's coverage).
                let mut candidates: Vec<usize> = animals_in[patch].iter().copied().filter(|&j| j != ai && !killed.contains(&j)).collect();
                for &(j, cover) in &ring_cells {
                    for &k in &animals_in[j] {
                        if k != ai && !killed.contains(&k) && self.rng.gen_bool(cover as f64) {
                            candidates.push(k);
                        }
                    }
                }
                let mut best: Option<(usize, f32, f32)> = None; // (idx, p_success, profitability)
                for _ in 0..candidates.len().min(3) {
                    let j = candidates[self.rng.gen_range(0..candidates.len())];
                    let prey = &self.animals[j];
                    let pm = prey.mass.max(0.001);
                    let p_in_water = self.patches[prey.patch].water;
                    let p_sub = self.patches[prey.patch].submersion;
                    let p_lin = (pm / prey.traits.mass.max(0.001)).clamp(0.01, 1.0).powf(1.0 / 3.0);
                    let prey_speed = (prey.traits.water_speed * p_sub + prey.traits.land_speed.max(prey.traits.air_speed) * (1.0 - p_sub)) * prey.genome.motility.clamp(0.0, 1.0) * p_lin * (1.0 - 0.3 * prey.traits.plating);
                    // Manoeuvrability decides chases as much as speed (Howland 1974):
                    // a turn rate of about 1.5 rad/s doubles either side's term.
                    let my_turn = self.animals[ai].traits.turn_water * sub + self.animals[ai].traits.turn_land.max(self.animals[ai].traits.turn_air) * (1.0 - sub);
                    let prey_turn = prey.traits.turn_water * p_sub + prey.traits.turn_land.max(prey.traits.turn_air) * (1.0 - p_sub);
                    // Chases scale with body size (Howland 1974): speeds in body
                    // lengths per second, a sessile party at the floor of 0.2.
                    let my_bl = speed_here / (2.0 * reach_now).max(0.01);
                    let prey_bl = prey_speed / (2.0 * prey.traits.reach * p_lin).max(0.01);
                    let attack = m34 * (0.2 + my_bl.min(10.0)) * (0.5 + 3.0 * claws) * (0.5 + 0.5 * upright) * (0.5 + 0.5 * (my_turn / 1.5).min(1.0));
                    // Taking off is the escape a ground predator cannot follow: a glider
                    // needs a perch (its patch's launch height), a flier needs nothing.
                    let takeoff = if p_in_water {
                        0.0
                    } else if prey.traits.air_speed > 0.0 {
                        1.0
                    } else {
                        let e = self.patches[prey.patch].elevation;
                        let drop = self.neighbours[prey.patch].iter().map(|&k| e - self.patches[k].elevation).fold(0.0f32, f32::max);
                        (prey.traits.glide_distance / crate::sim::AIR_DROP_HEIGHT * (self.patches[prey.patch].canopy + drop.min(50.0)) / 10.0).clamp(0.0, 1.0)
                    };
                    let defence = pm.powf(0.75) * (0.2 + prey_bl.min(10.0)) * (0.5 + 2.0 * prey.traits.plating) * (1.0 + 3.0 * takeoff) * (0.5 + 0.5 * (prey_turn / 1.5).min(1.0));
                    let scale = 0.25 * (attack + defence) + 1e-6;
                    // Encounter: detection distance scales with prey body length, so
                    // small prey are found at a fraction of the rate of large prey.
                    // Encounter: the predator meets the prey that its day's range
                    // covers, detecting small ones at a fraction of the rate.
                    let encounter = (range_area / a).min(1.0) * (pm / mass).powf(1.0 / 3.0).min(1.0);
                    // Capture: attack versus defence, times a hump in log size ratio
                    // (Brose 2006: small ectotherm predators do best on prey 10-100x
                    // smaller; much larger prey is possible but rare).
                    let z = (mass / pm).log10();
                    let hump = (-(z - 1.0).powi(2) / 2.0).exp();
                    let p_success = encounter * hump / (1.0 + (-(attack - defence) / scale).exp());
                    // Fritsch et al.: energy gained per kill falls with relative prey
                    // size, to zero for prey ten times larger (subduing cost, waste).
                    let xi = (0.4 + 0.4 * z).clamp(0.0, 1.0);
                    let edible = pm * 0.8 * xi;
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
                            let z = (mass / prey_mass.max(0.001)).log10();
                            let xi = (0.4 + 0.4 * z).clamp(0.0, 1.0);
                            let body = prey_mass * 0.8 + prey_reserve;
                            let edible = body * xi;
                            let meal = edible.min(space);
                            space -= meal;
                            food_meat += meal;
                            self.patches[patch].carrion += body - meal + prey_mass * 0.2;
                            self.stats.ppmr_log_sum += z;
                            if z > 0.0 {
                                self.stats.kills_by_larger += 1;
                            }
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
                graze_today += food_plant;
                prey_today += food_meat;
                carrion_today += food_carrion;
                a.gut_plant += food_plant;
                a.gut_meat += food_carrion + food_meat;
                a.ate_plant += food_plant;
                a.ate_meat += food_meat;
                a.ate_carrion += food_carrion;
            }

            // --- movement: when hungry, toward the best reachable neighbour --------
            let hungry = delta < 0.0 && self.animals[ai].reserve < 0.5 * (0.05 * mass + fat_frac * mass.powf(1.19));
            if hungry && self.rng.gen_bool(0.6) {
                let directed = self.rng.gen_bool(0.5);
                let mut best: Option<(usize, f32)> = None;
                // A glider reaches the next patch when a launch carries it ten metres;
                // a flier ranges over everything within an hour's flight.
                let can_glide_over = measured_air && glide_ratio * launch_h >= patch_w;
                let flier = measured_air && air_speed_raw * motility > 0.5;
                // Relocation is a distance: a flier an hour's flight, a crawler or
                // swimmer up to RELOCATE_MAX_M of its day's path, in cells of this width.
                let reloc_m = if flier { air_speed_raw * motility * 3600.0 } else { path.min(RELOCATE_MAX_M) };
                let range_hops = ((reloc_m / patch_w).round() as usize).clamp(1, 12);
                let candidates: Vec<usize> = if range_hops > 1 {
                    // Breadth-first over the neighbour graph up to range_hops.
                    let mut seen = vec![patch];
                    let mut frontier = vec![patch];
                    for _ in 0..range_hops {
                        let mut next = Vec::new();
                        for &q in &frontier {
                            for &k in &self.neighbours[q] {
                                if !seen.contains(&k) {
                                    seen.push(k);
                                    next.push(k);
                                }
                            }
                        }
                        frontier = next;
                    }
                    seen.into_iter().filter(|&k| k != patch).collect()
                } else {
                    self.neighbours[patch].clone()
                };
                for cand in candidates {
                    let cand_sub = self.patches[cand].submersion;
                    // Entry needs demonstrated ability in that habitat; unmeasured
                    // young stay where they are until their body has been tested.
                    let swims = measured_water && water_speed_raw * motility > 0.005;
                    let crawls = measured_land && land_speed_raw * motility > 0.005;
                    let can = if cand_sub >= 1.0 {
                        swims || (sub > 0.0 && cand_sub >= sub) // drift back out with the tide
                    } else if cand_sub > 0.0 {
                        // The intertidal shore: reached by swimming in with the tide or crawling.
                        swims || crawls || flier
                    } else {
                        // Dry land, from the shore or from land: a crawl, a flight or a
                        // glide. Whether it survives there is breathing and food.
                        crawls || flier || can_glide_over
                    };
                    if !can {
                        continue;
                    }
                    let value = if directed {
                        let plants_v: f32 = plants_in[cand].iter().map(|&pi| self.plants[pi].biomass).sum::<f32>() * eff_plant;
                        let carrion_v = self.patches[cand].carrion * eff_meat;
                        let prey_v: f32 = animals_in[cand].iter().map(|&j| self.animals[j].mass).sum::<f32>() * eff_meat * 0.2 * (0.5 + 3.0 * claws);
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
                    let use_speed = (if target_water { water_speed_raw } else { land_speed_raw.max(air_speed_raw) }) * motility * lin_f;
                    // Cost of transport ~ M^-0.3 per kg per metre (Tucker), 10 m per patch.
                    delta -= 0.00001 * mass.powf(0.7) * (g / 9.81) * patch_w / (0.3 + use_speed).min(3.0);
                    self.animals[ai].patch = target;
                }
            }

            // --- reserve, growth, reproduction, death -----------------------------
            let a = &mut self.animals[ai];
            // Osmotrophy for animals too: a motile sheet absorbs dead matter through
            // its surface before it has any mouth worth the name.
            let het = a.genome.heterotrophy.clamp(0.0, 1.0);
            if het > 0.0 {
                let surface = (6.0 * (a.mass / 1000.0).powf(2.0 / 3.0)).max(2.0 * a.traits.membrane_area * (a.mass / a.traits.mass.max(1e-3)).powf(2.0 / 3.0));
                let pool = self.patches[a.patch].dead_matter;
                let absorbed = (het * surface * (pool / a_area) * OSMOTROPHY_RATE * q).min(pool * 0.2);
                self.patches[a.patch].dead_matter -= absorbed;
                delta += absorbed * 0.6;
                a.ate_plant += absorbed * 0.6;
            }
            a.intake_rate = (delta + fmr + upkeep + activity_cost).max(0.0); // gross gain today
            a.reserve += delta;
            a.age += 1.0;
            // Fat capacity is hyper-allometric (Lindstedt & Boyce: M^1.19), so big
            // animals ride out lean spells longer; the gene sets the coefficient.
            let cap = 0.05 * a.mass + fat_frac * a.mass.powf(1.19);
            if a.reserve > cap {
                let mut surplus = a.reserve - cap;
                if a.mass < adult_mass {
                    // West-Brown-Enquist growth: dm/dt = a m^0.75 (1 - (m/M)^0.25).
                    // Asymptotic, so maturity is a fraction of adult mass, not all of it.
                    let wbe = 0.12 * m34 * q * (1.0 - (a.mass / adult_mass).powf(0.25)).max(0.02);
                    let grow = surplus.min(wbe).min(adult_mass - a.mass);
                    a.mass += grow * 0.7;
                    a.reserve -= grow;
                    surplus -= grow;
                }
                let mature = a.mass >= a.genome.maturity_frac.clamp(0.2, 0.98) * adult_mass;
                if mature {
                    let mass0 = (a.genome.offspring_frac.clamp(0.01, 0.5) * adult_mass).max(0.001);
                    let cost = mass0 * 1.3; // body plus a starter reserve
                    if surplus >= cost && self.rng.gen_bool(0.8) {
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
            if resp_eff < 0.2 && self.rng.gen_bool(0.5) {
                // A gill-breather on dry land, or a lung-breather under water.
                self.stats.deaths_suffocated += 1;
                deaths.push(ai);
            } else if a.reserve < 0.0 {
                self.stats.deaths_starved += 1;
                deaths.push(ai);
            } else if a.age > a.lifespan() {
                self.stats.deaths_old += 1;
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
                let cap = self.plant_cap(b.patch);
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
            let mut traits = self.lookup_traits(&b.genome);
            // A changed body inherits its parent's measured speeds as a neutral
            // prior until its own physics runs: assumed no better and no worse.
            if b.body_changed && !traits.measured_water && !traits.measured_land {
                traits.water_speed = b.parent_traits.water_speed;
                traits.land_speed = b.parent_traits.land_speed;
                traits.turn_water = b.parent_traits.turn_water;
                traits.turn_land = b.parent_traits.turn_land;
            }
            // Larval dispersal: young drift one or two patches from the parent,
            // which is what keeps them off the parents' menu in the sea.
            let born_at = self.disperse_same_habitat(b.patch, LARVAL_DRIFT_M / patch_w);
            self.animals.push(Animal::new(b.genome, key, traits, born_at, lineage, b.mass0, b.reserve0));
            let idx = self.animals.len() - 1;
            self.request_traits(idx);
            self.stats.births += 1;
        }

        self.stats.primary_production = production_today;
        self.stats.grazing_intake = graze_today;
        self.stats.predation_intake = prey_today;
        self.stats.scavenging_intake = carrion_today;
        self.stats.metabolic_cost = cost_today;

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
        // (A rescue, not biology: a planet has no off-map. Kept per area so it
        // does not depend on the cell size; flagged in the model notes.)
        // No rescues: a planet has no off-map. Extinction is a result.

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
        s.fliers = 0;
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
        s.satiated_frac = 0.0;
        for a in &self.animals {
            if a.gut_plant + a.gut_meat >= 0.8 * a.genome.gut_frac.clamp(0.02, 0.4) * a.mass {
                s.satiated_frac += 1.0 / na;
            }
            let (h, c, d) = a.diet();
            s.mean_gut += a.genome.gut_type / na;
            s.mean_weapon += a.traits.claws / na;
            s.mean_armour += a.traits.plating / na;
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
            // Gliding means a glide angle under 45 degrees (ratio >= 1: ten metres
            // beyond the stone from ten metres up). Below that it is parachuting:
            // a tumbling plate drifts a metre or two and is not a glider.
            if a.traits.air_speed > 0.0 || a.traits.glide_distance >= crate::sim::AIR_DROP_HEIGHT {
                s.fliers += 1;
            }
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
        s.mean_toughness = self.plants.iter().map(|p| p.genome.leaf_toughness).sum::<f32>() / self.plants.len().max(1) as f32;
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
            reach: 0.0,
            upright: 0.0,
            air_speed: 0.0,
            glide: 0.0,
            turn: 0.0,
            climb: 0.0,
            shore: 0.0,
            breathing: 0.0,
            shore_share: 0.0,
            speed_bl: 0.0,
        }
    }

    pub fn lineages(&self, max: usize) -> Vec<LineageSummary> {
        self.lineages_filtered("both", "all", "count", max)
    }

    /// Lineage table with a filter. `kind`: both | animal | plant. `habitat`:
    /// all | land | water | air (majority habitat; air = lineage can fly).
    /// `sort`: count | mass | speed | height | parts | land (share of members on land). `max` per kind.
    pub fn lineages_filtered(&self, kind: &str, habitat: &str, sort: &str, max: usize) -> Vec<LineageSummary> {
        let (mut out, mut plants) = self.all_lineages();
        let keep = |l: &LineageSummary| match habitat {
            "land" => l.aquatic_share < 0.5 && l.shore_share < 0.5,
            "shore" => l.shore_share >= 0.5,
            "water" => l.aquatic_share >= 0.5,
            "air" => l.air_speed > 0.0 || l.glide >= crate::sim::AIR_DROP_HEIGHT,
            _ => true,
        };
        out.retain(|l| keep(l));
        plants.retain(|l| keep(l));
        let key = |l: &LineageSummary| -> f32 {
            match sort {
                "mass" => l.mean_mass,
                "speed" => l.land_speed.max(l.water_speed).max(l.air_speed),
                "height" => l.height,
                "parts" => l.parts,
                "land" => 1.0 - l.aquatic_share,
                "bl" => l.speed_bl,
                _ => l.count as f32,
            }
        };
        out.sort_by(|a, b| key(b).partial_cmp(&key(a)).unwrap_or(std::cmp::Ordering::Equal));
        plants.sort_by(|a, b| key(b).partial_cmp(&key(a)).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(max);
        plants.truncate(max);
        match kind {
            "animal" => out,
            "plant" => plants,
            _ => {
                out.extend(plants);
                out
            }
        }
    }

    fn all_lineages(&self) -> (Vec<LineageSummary>, Vec<LineageSummary>) {
        let mut map: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, a) in self.animals.iter().enumerate() {
            map.entry(a.lineage).or_default().push(i);
        }
        let out: Vec<LineageSummary> = map
            .into_iter()
            .map(|(id, idx)| {
                let c = idx.len() as f32;
                let mut s = Self::empty_summary(id, "animal", idx.len());
                for &i in &idx {
                    let a = &self.animals[i];
                    let (h, ca, d) = a.diet();
                    s.mean_mass += a.mass / c;
                    s.gut += a.genome.gut_type / c;
                    s.weapon += a.traits.claws / c;
                    s.armour += a.traits.plating / c;
                    s.reach += a.traits.reach / c;
                    s.upright += a.traits.upright / c;
                    s.herb += h / c;
                    s.carn += ca / c;
                    s.detr += d / c;
                    s.aquatic_share += if self.patches[a.patch].water { 1.0 / c } else { 0.0 };
                    s.land_speed += a.traits.land_speed / c;
                    s.water_speed += a.traits.water_speed / c;
                    s.air_speed += a.traits.air_speed / c;
                    s.glide += a.traits.glide_distance / c;
                    s.turn += (if self.patches[a.patch].water { a.traits.turn_water } else { a.traits.turn_land.max(a.traits.turn_air) }) / c;
                    s.climb += a.traits.climb_water.max(a.traits.climb_air) / c;
                    s.shore += a.traits.shore / c;
                    s.breathing += a.genome.breathing / c;
                    s.shore_share += if self.patches[a.patch].shore { 1.0 / c } else { 0.0 };
                    let sb = self.patches[a.patch].submersion;
                    let sp = a.traits.water_speed * sb + a.traits.land_speed * (1.0 - sb);
                    s.speed_bl += sp / (2.0 * a.traits.reach).max(0.01) / c;
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
        let plants: Vec<LineageSummary> = pmap
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
        (out, plants)
    }

    /// Representative animal genome of a lineage (most offspring), with habitat.
    pub fn representative(&self, lineage: u32) -> Option<(Genome, Environment)> {
        self.representative_in(lineage, "all")
    }

    /// Most prolific member, preferring those in `habitat` (land | water | air)
    /// when any live there; the returned environment is where to replay it.
    pub fn representative_in(&self, lineage: u32, habitat: &str) -> Option<(Genome, Environment)> {
        let members: Vec<&Animal> = self.animals.iter().filter(|a| a.lineage == lineage).collect();
        let fits = |a: &&Animal| match habitat {
            "land" => !self.patches[a.patch].water && !self.patches[a.patch].shore,
            "shore" => self.patches[a.patch].shore,
            "water" => self.patches[a.patch].water,
            "air" => a.traits.air_speed > 0.0 || a.traits.glide_distance >= crate::sim::AIR_DROP_HEIGHT,
            _ => true,
        };
        let pool: Vec<&Animal> = if members.iter().any(fits) { members.iter().copied().filter(fits).collect() } else { members };
        pool.into_iter().max_by_key(|a| (a.offspring, (a.reserve * 100.0) as i64)).map(|a| {
            let env = if habitat == "air" && (a.traits.air_speed > 0.0 || a.traits.glide_distance >= crate::sim::AIR_DROP_HEIGHT) {
                Environment::Air
            } else if self.patches[a.patch].water {
                Environment::Water
            } else {
                Environment::Land
            };
            (a.genome.clone(), env)
        })
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
        // The world starts with algae only; add grazers explicitly so the
        // physics measurement path is exercised.
        w.immigrate(20);
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
