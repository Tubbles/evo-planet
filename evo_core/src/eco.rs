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

use crate::decode::{body_metrics, decode, Phenotype};
use crate::genome::Genome;
use std::f32::consts::PI;
use crate::planet::{Derived, PlanetParams};
use crate::plant::{decode_plant, PlantBody};
use crate::sim::{fatigue_factor, CreatureSim, Environment, EpisodeResult, SimConfig};
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
/// Mineral nutrient (the limiting element, nitrogen's part) in the soil or
/// water, kg per m². It is conserved: taken up as tissue is made, returned
/// as tissue decays or is metabolised. Three grams a square metre is a
/// productive sea's photic column or a lean soil; it caps standing biomass
/// at 150 g/m² and makes light's free lunch finite.
pub const NUTRIENT0_PER_M2: f32 = 0.003;
/// Kilograms of nutrient in a kilogram of dry tissue.
pub const NUTRIENT_FRACTION: f32 = 0.02;
pub const DEAD0_PER_M2: f32 = 0.01;
/// Individual-plant density caps (performance guards), per m^2. The same
/// on land as in water: at eight per hundred square metres the land cells
/// filled with stunted seedlings and no better-adapted seed could land.
pub const PLANT_CAP_PER_M2_WATER: f32 = 0.48;
pub const PLANT_CAP_PER_M2_LAND: f32 = 0.48;
/// Founding grazers per square metre of sea.
pub const FOUNDERS_PER_M2: f32 = 1.0 / 1200.0;
/// Evaporation from a wet body surface in dry air, kg of water per m² per
/// day (a few millimetres a day in still air; a frog's skin, a worm's).
/// Scaled by the air's dryness, by warmth and by the integument's
/// waterproofing (the land tolerance gene, the same gene a plant's cuticle
/// uses). A compact body carries little surface per kilogram; a sheet is
/// nearly all surface and dries out on land in a day.
pub const EVAPORATION_KG_PER_M2_DAY: f32 = 1.0;
/// Water deficit that kills, as a fraction of body mass (mammals die near
/// 15%; the water-store gene adds to this by carrying more).
pub const LETHAL_WATER_LOSS: f32 = 0.15;
/// Fresh plant and animal tissue is mostly water: kilograms of water per
/// kilogram of dry food eaten.
pub const WATER_PER_KG_PLANT: f32 = 4.0;
pub const WATER_PER_KG_MEAT: f32 = 2.5;
/// Dew per square metre of surface on a fully humid night, kg.
pub const DEW_KG_PER_M2: f32 = 0.2;
/// Growth yield of decomposers on dead matter (fraction becoming bacteria).
pub const BACTERIAL_YIELD: f32 = 0.4;
/// Phagocytosis: fraction of the suspended particles passing a surface that
/// are captured, per unit heterotrophy.
pub const CAPTURE_EFF: f32 = 0.05;
/// Ambient water movement past a body, m/s (waves, currents).
pub const CURRENT_M_S: f32 = 0.05;
/// Light extinction in water, per metre.
pub const WATER_EXTINCTION: f32 = 0.3;
/// Osmotrophy: kg absorbed per m^2 of surface per day per kg/m^2 of dead matter.
pub const OSMOTROPHY_RATE: f32 = 0.3;
/// Larval drift from the birth site, metres.
pub const LARVAL_DRIFT_M: f32 = 15.0;
/// The seed dispersal gene is in tens of metres.
pub const SEED_DISPERSAL_UNIT_M: f32 = 10.0;
/// Share of a water patch's plant mass that is suspended (drift, plankton)
/// and so reachable by filtering a current rather than by travelling.
pub const SUSPENDED_FRACTION: f32 = 0.2;
/// Water cleared by a filter feeder, m^3 per kg of body per day, at one
/// kilogram; per kilogram it falls as the square root of mass (a copepod
/// clears hundreds of cubic metres per kilogram a day, a mussel tens, a
/// kilogram fish one).
pub const CLEARANCE_M3_PER_KG_DAY: f32 = 0.5;
/// Clearance per kilogram for a body of `mass` kg.
pub fn clearance_per_kg(mass: f32) -> f32 {
    CLEARANCE_M3_PER_KG_DAY * mass.max(1e-6).powf(-0.5)
}
// No population guard: the world's limits (light, particles, food, crowding)
// are the only limits. Performance is a separate problem (cohorts, planned)
// and is never solved by discarding births.
pub const TRAIT_EPISODE_S: f32 = 6.0;
/// An animal's body genes mutate at this share of the rate of its other
/// genes (controller, life history, physiology). Morphology is conserved in
/// life, and each changed body is a physics measurement the planet must
/// afford: at the old rate a populated world bred a thousand new bodies a
/// day against a few dozen measured, and an unmeasured body pays for its
/// new part without ever being credited for it.
pub const BODY_MUTATION_SCALE: f32 = 0.2;
/// Foraging day, seconds (8 h), over which the gait's measured power is paid.
pub const FORAGE_S: f32 = 8.0 * 3600.0;
/// Muscle converts a quarter of its metabolic energy into mechanical work.
pub const MUSCLE_EFFICIENCY: f32 = 0.25;
/// Energy in a kilogram of dry biomass, joules (about 18 MJ).
pub const J_PER_KG_DRY: f32 = 18.0e6;
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

pub fn default_heading() -> [f32; 3] {
    [0.0, 1.0, 0.0]
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Traits {
    pub mass: f32,
    pub land_speed: f32,
    pub water_speed: f32,
    pub glide_distance: f32,
    /// Oxygen-limited share of full muscle output in water and in air.
    pub activity: f32,
    #[serde(default)]
    pub fatigue_water: f32,
    #[serde(default)]
    pub fatigue_land: f32,
    pub section_area: f32,
    pub base_freq: f32,
    pub membrane_area: f32,
    /// Mass held in sheets at adult size, kg.
    pub membrane_mass: f32,
    pub measured_land: bool,
    pub measured_water: bool,
    pub measured_air: bool,
    /// Sustained flight speed, m/s: set only when the body is still aloft at
    /// the end of the air-drop episode (powered or very efficient flight).
    pub air_speed: f32,
    /// Powered lift: metres of mean height the wing stroke adds over the same
    /// body holding its wings still. Zero for a pure glider.
    #[serde(default)]
    pub flap_lift: f32,
    /// Which way positive steer turns the body in each medium (+1 or -1, 0
    /// before the turn test), so the arena can steer it at a target.
    #[serde(default)]
    pub turn_sign_land: f32,
    #[serde(default)]
    pub turn_sign_water: f32,
    /// Which way the body travels in the trunk's own frame, per medium (from
    /// the gait episode), so the arena steers the gait, not the trunk axis.
    #[serde(default = "crate::eco::default_heading")]
    pub heading_land: [f32; 3],
    #[serde(default = "crate::eco::default_heading")]
    pub heading_water: [f32; 3],
    /// Measured mechanical power of the gait at full drive, watts, and its
    /// cost of transport, J/(kg m), per medium. What moving costs.
    #[serde(default)]
    pub power_land: f32,
    #[serde(default)]
    pub power_water: f32,
    #[serde(default)]
    pub cot_land: f32,
    #[serde(default)]
    pub cot_water: f32,
    /// The land gait's signature: how many limbs step, how high the trunk is
    /// carried (metres), and how much the trunk tumbles (rad/s).
    #[serde(default)]
    pub stepping_limbs: f32,
    #[serde(default)]
    pub trunk_clearance: f32,
    #[serde(default)]
    pub trunk_spin: f32,
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
    /// Water deficit, kg (zero when hydrated, negative when drying out).
    #[serde(default)]
    pub water: f32,
}

impl Animal {
    fn new(genome: Genome, key: String, traits: Traits, patch: usize, lineage: u32, mass: f32, reserve: f32) -> Self {
        Self {
            genome,
            key,
            traits,
            patch,
            lineage,
            mass: mass.max(1e-7),
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
            water: 0.0,
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

    /// Lifespan on the motility continuum: a sessile organism's lifespan gene
    /// (days) blending into the animal's 150·M^0.25·longevity as it moves.
    pub fn lifespan(&self) -> f32 {
        let m = self.genome.motility.clamp(0.0, 1.0);
        let animal = 150.0 * self.traits.mass.max(1e-7).powf(0.25) * self.genome.longevity.max(0.1);
        (1.0 - m) * self.genome.lifespan.max(1.0) + m * animal
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
    /// Where on the intertidal band a shore cell lies, 0 at the low-water
    /// mark (under water most of the day) to 1 at the high-water mark (dry
    /// most of the day), by elevation rank among the shore cells.
    #[serde(default)]
    pub shore_level: f32,
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
    /// Leaf area of motile sheets floating in the patch, m^2 (set by the plant tick).
    #[serde(default)]
    pub motile_area: f32,
    /// Bacterial biomass, kg: grown on dead matter, eaten by filter feeders
    /// and phagotrophs, respired back to nutrient (the microbial loop).
    #[serde(default)]
    pub bacteria: f32,
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
    /// Animals with a measured sustained air speed or a real glide.
    #[serde(default)]
    pub fliers: usize,
    /// Animals whose wing stroke measurably lifts them over holding still
    /// (powered flight, the flapping test with its controls).
    #[serde(default)]
    pub powered: usize,
    /// Pairs of body types fought out in the arena, and pairs waiting.
    #[serde(default)]
    pub arena_pairs: usize,
    /// Mean shares over the pairs fought so far: eaten, fled.
    #[serde(default)]
    pub arena_eaten: f32,
    #[serde(default)]
    pub arena_fled: f32,
    #[serde(default)]
    pub arena_pending: usize,
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
    /// Animals on dry land (not the intertidal shore), and their mean mass.
    #[serde(default)]
    pub inland_animals: usize,
    #[serde(default)]
    pub inland_mass: f32,
    /// Plant biomass on land, kg.
    #[serde(default)]
    pub land_plant_biomass: f32,
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
    /// Of the grazing, the share taken from motile sheets (kg/day).
    #[serde(default)]
    pub mat_grazing: f32,
    pub predation_intake: f32,
    pub scavenging_intake: f32,
    pub metabolic_cost: f32,
    pub deaths_starved: u64,
    pub deaths_old: u64,
    #[serde(default)]
    pub deaths_suffocated: u64,
    /// Dried out on land: water lost through the skin faster than drunk or eaten.
    #[serde(default)]
    pub deaths_dried: u64,
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
    /// Mean waterproofing (land tolerance) and trunk flatness.
    #[serde(default)]
    pub waterproof: f32,
    #[serde(default)]
    pub flat: f32,
    /// Mouth cross-section, m², and mean gross intake per kilogram per day.
    #[serde(default)]
    pub mouth: f32,
    #[serde(default)]
    pub intake: f32,
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
    /// Powered lift from the wing stroke over holding still, metres.
    #[serde(default)]
    pub lift: f32,
    /// Land gait signature means: stepping limbs, trunk clearance (m), cost of transport.
    #[serde(default)]
    pub limbs: f32,
    #[serde(default)]
    pub clearance: f32,
    #[serde(default)]
    pub cot: f32,
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

/// A pair of creature types waiting for the arena: the two bodies that met
/// first stand for their lineages, with the turn signs their tests gave.
#[derive(Clone, Debug)]
struct ArenaRequest {
    pred: u32,
    prey: u32,
    env: Environment,
    genome_pred: Genome,
    genome_prey: Genome,
    sign_pred: f32,
    sign_prey: f32,
    travel_pred: [f32; 3],
    travel_prey: [f32; 3],
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
    /// Fought-out outcomes per (predator lineage, prey lineage, medium): the
    /// shares of eaten, fled and survived over `arena::BOUTS` bouts. The
    /// lineage is the "creature type"; the bodies that first met stand for it.
    arena_cache: HashMap<(u32, u32, Environment), crate::arena::ArenaOutcome>,
    /// Pairs whose first attack has happened and that have not been fought
    /// yet, in order of first attack.
    arena_pending: Vec<ArenaRequest>,
    arena_seen: HashSet<(u32, u32, Environment)>,
    /// Arena pairs fought per day while any wait (a fraction accumulates), on
    /// top of the body budget; the arena otherwise starves whenever bodies
    /// are waiting, which in a populated world is always.
    pub arena_rate: f32,
    arena_credit: f32,
    /// A predator lineage's record: summed eaten share and number of pairs
    /// fought, the prior for its pairs not yet fought.
    arena_record: HashMap<u32, (f32, u32)>,
    pub stats: EcoStats,
    pub history: Vec<[f32; 7]>,
}

/// Everything a world needs to carry on from where it was: what the
/// physics has measured, what the arena has found, and who is alive. The
/// day's queues are rebuilt on restore.
#[derive(Serialize, Deserialize)]
pub struct Checkpoint {
    planet: PlanetParams,
    patches: Vec<Patch>,
    neighbours: Vec<Vec<usize>>,
    plants: Vec<Plant>,
    animals: Vec<Animal>,
    day: u64,
    terrain_seed: u32,
    patch_area: f32,
    next_lineage: u32,
    trait_cache: Vec<(String, Traits)>,
    arena_cache: Vec<((u32, u32, Environment), crate::arena::ArenaOutcome)>,
    arena_record: Vec<(u32, (f32, u32))>,
    arena_rate: f32,
    arena_credit: f32,
    stats: EcoStats,
    history: Vec<[f32; 7]>,
}

impl EcoWorld {
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            planet: self.planet.clone(),
            patches: self.patches.clone(),
            neighbours: self.neighbours.clone(),
            plants: self.plants.clone(),
            animals: self.animals.clone(),
            day: self.day,
            terrain_seed: self.terrain_seed,
            patch_area: self.patch_area,
            next_lineage: self.next_lineage,
            trait_cache: self.trait_cache.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            arena_cache: self.arena_cache.iter().map(|(k, v)| (*k, *v)).collect(),
            arena_record: self.arena_record.iter().map(|(k, v)| (*k, *v)).collect(),
            arena_rate: self.arena_rate,
            arena_credit: self.arena_credit,
            stats: self.stats.clone(),
            history: self.history.clone(),
        }
    }

    /// A world from a checkpoint: bodies decoded again, queues rebuilt, the
    /// random stream reseeded from the day (a restored run is not the same
    /// run, but it is the same world).
    pub fn restore(c: Checkpoint) -> Self {
        let derived = c.planet.derive();
        let gravity = c.planet.gravity;
        let mut plants = c.plants;
        for p in plants.iter_mut() {
            if p.body.is_none() {
                p.body = Some(decode_plant(&p.genome, gravity));
            }
        }
        let mut w = EcoWorld {
            planet: c.planet,
            derived,
            patches: c.patches,
            neighbours: c.neighbours,
            plants,
            animals: c.animals,
            day: c.day,
            terrain_seed: c.terrain_seed,
            patch_area: c.patch_area,
            rng: StdRng::seed_from_u64(c.day ^ ((c.terrain_seed as u64) << 20)),
            next_lineage: c.next_lineage,
            trait_cache: c.trait_cache.into_iter().collect(),
            pending: Vec::new(),
            arena_cache: c.arena_cache.into_iter().collect(),
            arena_pending: Vec::new(),
            arena_seen: HashSet::new(),
            arena_rate: c.arena_rate,
            arena_credit: c.arena_credit,
            arena_record: c.arena_record.into_iter().collect(),
            stats: c.stats,
            history: c.history,
        };
        w.arena_seen = w.arena_cache.keys().copied().collect();
        set_shore_levels(&mut w.patches);
        for i in 0..w.animals.len() {
            w.request_traits(i);
        }
        w
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        let bytes = bincode::serialize(&self.checkpoint()).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let tmp = format!("{path}.tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(tmp, path)
    }

    pub fn load(path: &str) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        let c: Checkpoint = bincode::deserialize(&bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(Self::restore(c))
    }
}

/// Rank each shore cell on the intertidal band by elevation: the lowest
/// shore cell is the low-water mark (level 0), the highest the high-water
/// mark (level 1).
fn set_shore_levels(patches: &mut [Patch]) {
    let mut shore: Vec<usize> = (0..patches.len()).filter(|&i| patches[i].shore).collect();
    shore.sort_by(|&a, &b| patches[a].elevation.partial_cmp(&patches[b].elevation).unwrap());
    let n = shore.len().max(2) as f32 - 1.0;
    for (rank, &i) in shore.iter().enumerate() {
        patches[i].shore_level = rank as f32 / n;
    }
}

/// A shore cell's submersion today: under water most of the day at the
/// low-water mark, barely at the high-water mark, swinging with the
/// spring-neap cycle.
fn shore_submersion(level: f32, tide: f32) -> f32 {
    (0.9 - 0.8 * level + (tide - 0.5)).clamp(0.05, 0.95)
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

/// Measure one body in one environment: the episodes of the battery and the
/// traits they set. Pure, so a day's batch can run on every core.
/// What the decoded body says before any physics: mass, surfaces, muscle,
/// mouth, claws, plating, the rest-pose reach, and the oxygen-limited
/// activity. Physics overlays speeds, power and the gait on these.
fn decoded_traits(pheno: &Phenotype, derived: &Derived) -> Traits {
    let bm = body_metrics(pheno);
    Traits {
        mass: pheno.total_mass,
        activity: fatigue_factor(pheno, derived, 0.0),
        fatigue_water: fatigue_factor(pheno, derived, 1.0),
        fatigue_land: fatigue_factor(pheno, derived, 0.0),
        // Muscle work: cross-section of driven joints weighted by how far
        // and how often they are swung. A rigid strut or a limp limb costs
        // only its mass.
        section_area: pheno
            .parts
            .iter()
            .skip(1)
            .map(|p| p.muscle_section * (1.0 - p.stiffness) * p.cpg.freq_mult * (p.cpg.amp.abs().min(p.limit) / 0.8).min(1.5))
            .sum::<f32>(),
        base_freq: pheno.base_freq,
        membrane_area: pheno.membrane_area,
        membrane_mass: pheno.membrane_mass,
        reach: bm.reach,
        mouth_area: bm.mouth_area,
        claws: bm.claws,
        plating: bm.plating,
        upright: 0.3,
        ..Default::default()
    }
}

fn measure_body(planet: &PlanetParams, genome: &Genome, env: Environment, mut traits: Traits) -> Traits {
    let pheno = decode(&genome);
    // Until now this body may have been accounted as its parent's (the
    // neutral prior at birth); from here on it is itself.
    {
        let own = decoded_traits(&pheno, &planet.derive());
        traits.mass = own.mass;
        traits.activity = own.activity;
        traits.fatigue_water = own.fatigue_water;
        traits.fatigue_land = own.fatigue_land;
        traits.section_area = own.section_area;
        traits.base_freq = own.base_freq;
        traits.membrane_area = own.membrane_area;
        traits.membrane_mass = own.membrane_mass;
        traits.mouth_area = own.mouth_area;
        traits.claws = own.claws;
        traits.plating = own.plating;
        if !traits.measured_land && !traits.measured_water && !traits.measured_air {
            traits.reach = own.reach;
        }
    }
    let cfg = SimConfig { episode_time: TRAIT_EPISODE_S, ..Default::default() };
    let r = CreatureSim::run_episode(pheno.clone(), planet, env, cfg);
    // Sustained progress only, and only what the muscles add: the same
    // body is run again with its motors off (sham control) and that
    // passive drift, a buoyant rise with a tilted fin or a slide after
    // a topple, is subtracted. The air drop has its own control (a stone).
    let mut passive_shore = 0.0;
    // In the air the control is the same body holding its wings
    // still (amp_scale 0): what the held wing does is the glide,
    // what the stroke adds over it is powered lift.
    let mut hold_air: Option<EpisodeResult> = None;
    let passive = if r.exploded {
        0.0
    } else if env == Environment::Air {
        let hold = SimConfig { episode_time: TRAIT_EPISODE_S, amp_scale: 0.0, ..Default::default() };
        hold_air = Some(CreatureSim::run_episode(pheno, planet, env, hold));
        0.0
    } else if r.sustained_speed < 0.005 && env != Environment::Shore {
        // No progress under power: nothing for the control to subtract.
        0.0
    } else {
        let sham = SimConfig { episode_time: TRAIT_EPISODE_S, motors: false, ..Default::default() };
        let c = CreatureSim::run_episode(pheno, planet, env, sham);
        passive_shore = if c.exploded { 0.0 } else { c.shore_progress };
        if c.exploded { 0.0 } else { c.sustained_speed }
    };
    let speed = if r.exploded { 0.0 } else { (r.sustained_speed - passive).max(0.0) };
    // Nimbleness, only for bodies that locomote: the same body driven
    // asymmetrically (turning) and with a shifted posture (climb/dive).
    let mut turn_sign = 0.0f32;
    let (turn, climb) = if speed > 0.01 && !r.exploded && env != Environment::Shore {
        let pheno2 = decode(&genome);
        let st = SimConfig { episode_time: TRAIT_EPISODE_S, steer: 0.6, ..Default::default() };
        let rs = CreatureSim::run_episode(pheno2.clone(), planet, env, st);
        let turn = if rs.exploded { 0.0 } else { (rs.yaw_rate - r.yaw_rate).abs() };
        if !rs.exploded {
            turn_sign = (rs.yaw_rate - r.yaw_rate).signum();
        }
        let climb = if env == Environment::Land {
            0.0
        } else {
            let bc = SimConfig { episode_time: TRAIT_EPISODE_S, bias_shift: 0.4, ..Default::default() };
            let rc = CreatureSim::run_episode(pheno2, planet, env, bc);
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
            traits.power_water = if r.exploded { 0.0 } else { r.work / TRAIT_EPISODE_S };
            traits.cot_water = if r.exploded { 0.0 } else { r.cost_of_transport };
            traits.turn_water = turn;
            traits.turn_sign_water = turn_sign;
            traits.heading_water = r.travel_local;
            traits.climb_water = climb;
            traits.measured_water = true;
            traits.reach = traits.reach.max(r.swept_reach);
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
            let stone = crate::sim::ballistic_distance(planet.gravity);
            // Glide: what the held wing does beyond a thrown stone.
            // Powered lift: how much higher the stroke keeps the body
            // than holding the same wing (the flapping test's control).
            let (glide, lift) = match &hold_air {
                Some(h) if !h.exploded => ((h.air_distance - stone).max(0.0), (r.mean_height - h.mean_height).max(0.0)),
                _ => ((r.air_distance - stone).max(0.0), 0.0),
            };
            traits.glide_distance = if r.exploded { 0.0 } else { glide };
            traits.flap_lift = if r.exploded { 0.0 } else { lift };
            // Speed through the air for the share of the episode spent aloft.
            traits.air_speed = if r.exploded { 0.0 } else { r.distance / TRAIT_EPISODE_S * r.airborne_frac };
            traits.turn_air = turn;
            traits.climb_air = climb;
            traits.measured_air = true;
            traits.reach = traits.reach.max(r.swept_reach);
        }
        _ => {
            traits.land_speed = speed;
            traits.power_land = if r.exploded { 0.0 } else { r.work / TRAIT_EPISODE_S };
            traits.cot_land = if r.exploded { 0.0 } else { r.cost_of_transport };
            traits.stepping_limbs = r.stepping_limbs as f32;
            traits.trunk_clearance = r.trunk_clearance;
            traits.trunk_spin = r.trunk_spin;
            traits.turn_land = turn;
            traits.turn_sign_land = turn_sign;
            traits.heading_land = r.travel_local;
            traits.measured_land = true;
            // Reach is what the body swept in the episode, not the
            // rest pose it was drawn in.
            if r.swept_reach > 0.0 {
                traits.reach = r.swept_reach;
            }
            traits.upright = (r.mean_height / (0.5 * traits.reach).max(1e-4)).clamp(0.0, 1.0);
        }
    }
    traits
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
        let raw: Vec<f32> = positions.iter().map(|p| terrain::elevation_at_res(*p, terrain_seed, n)).collect();
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
                Patch { pos: positions[i], elevation, temp_c, water, shore: false, shore_level: 0.0, moisture, nutrient: NUTRIENT0_PER_M2 * patch_area, dead_matter: DEAD0_PER_M2 * patch_area, carrion: 0.0, canopy: 0.0, submersion: if water { 1.0 } else { 0.0 }, light_left: 1.0, bacteria: 0.0, motile_area: 0.0 }
            })
            .collect();
        // Fresh water: an inland basin (a land cell lower than every
        // neighbour, none of them sea) holds a lake. Lakes are where plants
        // and animals went ashore in life, a wet foothold with no sea's seed
        // rain swamping what adapts there; their shores are inland shores.
        for i in 0..n {
            if patches[i].water || neighbours[i].iter().any(|&j| patches[j].water) {
                continue;
            }
            if neighbours[i].iter().all(|&j| patches[j].elevation > patches[i].elevation) {
                patches[i].water = true;
                patches[i].submersion = 1.0;
                patches[i].moisture = 1.0;
            }
        }
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
        set_shore_levels(&mut patches);
        for i in 0..n {
            if patches[i].shore {
                patches[i].submersion = shore_submersion(patches[i].shore_level, 0.5);
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
            arena_cache: HashMap::new(),
            arena_pending: Vec::new(),
            arena_seen: HashSet::new(),
            arena_rate: 1.0,
            arena_credit: 0.0,
            arena_record: HashMap::new(),
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
            let mut genome = Genome::random_with(&mut self.rng);
            // Everyone starts as a small algae grazer; predation, scavenging
            // and body size evolve from there (diet is what is eaten, not a gene).
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
        let t = decoded_traits(&decode(g), &self.derived);
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
        let old_sea = terrain::elevation_at_res(self.patches[0].pos, self.terrain_seed, self.patches.len()) - self.patches[0].elevation;
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
            if self.patches[i].water || self.neighbours[i].iter().any(|&j| self.patches[j].water) {
                continue;
            }
            if self.neighbours[i].iter().all(|&j| self.patches[j].elevation > self.patches[i].elevation) {
                self.patches[i].water = true;
                self.patches[i].submersion = 1.0;
                self.patches[i].moisture = 1.0;
            }
        }
        for i in 0..n {
            if !self.patches[i].water && self.neighbours[i].iter().any(|&j| self.patches[j].water) {
                self.patches[i].shore = true;
                self.patches[i].submersion = 0.5;
                self.patches[i].moisture = self.patches[i].moisture.max(0.85);
            }
        }
        set_shore_levels(&mut self.patches);
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
                let ph = decode(&a.genome);
                a.traits.activity = fatigue_factor(&ph, &derived, 0.0);
                a.traits.fatigue_water = fatigue_factor(&ph, &derived, 1.0);
                a.traits.fatigue_land = fatigue_factor(&ph, &derived, 0.0);
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
        // Any surface may lift: every body with a sheet is dropped once.
        let winged = t.membrane_area > 0.0;
        // The queue is rebuilt from scratch each day and a body is requested
        // once per rebuild, so no duplicate check (which was quadratic) is needed.
        if winged && !t.measured_air {
            self.pending.push((idx, Environment::Air));
        }
        for env in order {
            let done = match env {
                Environment::Water => t.measured_water,
                _ => t.measured_land,
            };
            if !done {
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
            // Bodies on dry ground or the upper shore are measured before the
            // sea's: the land is where an unmeasured body's speed decides
            // whether it eats, and the sea's crowd would otherwise bury them.
            // (A scheduling choice inside the performance guard, not biology.)
            let (land, sea): (Vec<_>, Vec<_>) = order.into_iter().partition(|&(idx, _)| self.patches[self.animals[idx].patch].submersion < 0.5);
            let mut order = sea;
            order.extend(land);
            // Popped from the back: reverse so the first chosen is popped first.
            order.reverse();
            self.pending = order;
        }
        // Gather the day's batch: one request per unmeasured (body, environment).
        let mut batch: Vec<(String, Genome, Environment, Traits)> = Vec::new();
        while batch.len() < budget {
            let Some((idx, env)) = self.pending.pop() else { break };
            if idx >= self.animals.len() {
                continue;
            }
            let key = self.animals[idx].key.clone();
            let traits = self.trait_cache.get(&key).copied().unwrap_or(self.animals[idx].traits);
            let already = match env {
                Environment::Water => traits.measured_water,
                Environment::Air => traits.measured_air,
                Environment::Shore => traits.measured_shore,
                _ => traits.measured_land,
            };
            if already || batch.iter().any(|(k, _, e, _)| *k == key && *e == env) {
                for a in self.animals.iter_mut() {
                    if a.key == key {
                        a.traits = traits;
                    }
                }
                continue;
            }
            batch.push((key, self.animals[idx].genome.clone(), env, traits));
        }
        // Run the batch, on every core where there are cores (not in wasm).
        let planet = &self.planet;
        let results: Vec<Traits> = {
            #[cfg(target_arch = "wasm32")]
            {
                batch.iter().map(|(_, g, env, t)| measure_body(planet, g, *env, *t)).collect()
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).max(1);
                if batch.len() <= 1 || threads == 1 {
                    batch.iter().map(|(_, g, env, t)| measure_body(planet, g, *env, *t)).collect()
                } else {
                    let chunk = (batch.len() + threads - 1) / threads;
                    std::thread::scope(|sc| {
                        let hs: Vec<_> = batch.chunks(chunk).map(|c| sc.spawn(move || c.iter().map(|(_, g, env, t)| measure_body(planet, g, *env, *t)).collect::<Vec<_>>())).collect();
                        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
                    })
                }
            }
        };
        for ((key, _, _, _), traits) in batch.into_iter().zip(results) {
            self.stats.trait_evals += 1;
            done += 1;
            self.trait_cache.insert(key.clone(), traits);
            for a in self.animals.iter_mut() {
                if a.key == key {
                    a.traits = traits;
                }
            }
        }
        // Then the arena: pairs of creature types whose first attack has
        // happened and that were not yet fought out. The arena has its own
        // allowance of `arena_rate` pairs a day (a pair costs about
        // `PAIR_COST` body episodes), because the body queue is never empty
        // in a populated world and would otherwise starve it.
        if !self.arena_pending.is_empty() {
            self.arena_credit += self.arena_rate;
        }
        while self.arena_credit >= 1.0 {
            let Some(req) = self.arena_pending.pop() else { self.arena_credit = 0.0; break };
            self.arena_credit -= 1.0;
            let key = (req.pred, req.prey, req.env);
            if self.arena_cache.contains_key(&key) {
                continue;
            }
            let pp = decode(&req.genome_pred);
            let pq = decode(&req.genome_prey);
            let seed = (req.pred as u64) << 32 | req.prey as u64;
            let out = crate::arena::run_pair(&pp, &pq, &self.planet, req.env, (req.sign_pred, req.travel_pred), (req.sign_prey, req.travel_prey), seed);
            self.arena_cache.insert(key, out);
            let rec = self.arena_record.entry(req.pred).or_insert((0.0, 0));
            rec.0 += out.eaten;
            rec.1 += 1;
            self.stats.arena_pairs = self.arena_cache.len();
            let n = self.arena_cache.len().max(1) as f32;
            self.stats.arena_eaten = self.arena_cache.values().map(|o| o.eaten).sum::<f32>() / n;
            self.stats.arena_fled = self.arena_cache.values().map(|o| o.fled).sum::<f32>() / n;
            done += crate::arena::PAIR_COST;
        }
        self.stats.pending_evals = self.pending.len();
        self.stats.arena_pending = self.arena_pending.len();
        done
    }

    /// The fought-out outcome for a predator lineage against a prey lineage
    /// in a medium, if that pair has been through the arena.
    fn arena_outcome(&self, pred: u32, prey: u32, env: Environment) -> Option<crate::arena::ArenaOutcome> {
        let env = if env == Environment::Water { Environment::Water } else { Environment::Land };
        self.arena_cache.get(&(pred, prey, env)).copied()
    }

    /// A predator lineage's own record (mean eaten share over the pairs it
    /// has fought), once it has fought a few: the prior for pairs it has not.
    /// A predator lineage's record over the pairs it has fought: the sum of
    /// eaten shares and the number of fights.
    fn arena_record(&self, pred: u32) -> Option<(f32, u32)> {
        self.arena_record.get(&pred).copied()
    }

    /// Fights a lineage's record is weighed against the prior with: a lost
    /// first bout must not zero every pair it has never fought.
    pub const ARENA_PRIOR_WEIGHT: f32 = 5.0;

    /// Every fought-out pair: (predator lineage, prey lineage, medium, outcome).
    pub fn arena_outcomes(&self) -> Vec<(u32, u32, Environment, crate::arena::ArenaOutcome)> {
        self.arena_cache.iter().map(|(k, v)| (k.0, k.1, k.2, *v)).collect()
    }

    /// Queue a pair for the arena at its first attack, with the two bodies
    /// that met standing for their lineages.
    fn arena_request(&mut self, pred_idx: usize, prey_idx: usize, env: Environment) {
        let env = if env == Environment::Water { Environment::Water } else { Environment::Land };
        let (a, b) = (&self.animals[pred_idx], &self.animals[prey_idx]);
        let key = (a.lineage, b.lineage, env);
        if self.arena_cache.contains_key(&key) || !self.arena_seen.insert(key) {
            return;
        }
        if a.traits.mouth_area <= 0.0 && a.traits.claws <= 0.0 {
            // Nothing to bite or hold with: the fight is decided without a bout.
            self.arena_cache.insert(key, crate::arena::ArenaOutcome { eaten: 0.0, fled: 0.0, survived: 1.0, bouts: 0 });
            return;
        }
        let (sp, sq, tp, tq) = if env == Environment::Water {
            (a.traits.turn_sign_water, b.traits.turn_sign_water, a.traits.heading_water, b.traits.heading_water)
        } else {
            (a.traits.turn_sign_land, b.traits.turn_sign_land, a.traits.heading_land, b.traits.heading_land)
        };
        self.arena_pending.push(ArenaRequest { pred: a.lineage, prey: b.lineage, env, genome_pred: a.genome.clone(), genome_prey: b.genome.clone(), sign_pred: sp, sign_prey: sq, travel_pred: tp, travel_prey: tq });
    }

    /// Larval drift: a random walk through the birth cell's medium, as a
    /// seed's (water larvae drift through water and strand on the shore,
    /// land young stay on land and the shore, the shore's go either way).
    /// A walk that kept the shore's young to wet cells meant nothing born
    /// on the shore could ever be born inland.
    fn disperse_same_habitat(&mut self, from: usize, reach: f32) -> usize {
        self.disperse(from, reach)
    }

    /// A seed's random walk of `hops` steps, through its parent's medium: a
    /// submerged plant's spores drift through water and strand on the shore;
    /// a land plant's seeds blow over land and the shore; a shore plant's
    /// go either way. (A free walk let every sea alga rain seeds on the
    /// interior, a gene flow that swamped land adaptation where it began.)
    fn disperse(&mut self, from: usize, reach: f32) -> usize {
        let class = |sub: f32| -> i8 { if sub >= 1.0 { 2 } else if sub > 0.0 { 1 } else { 0 } };
        let home = class(self.patches[from].submersion);
        let mut p = from;
        let mut hops = reach.floor() as usize;
        if self.rng.gen_bool((reach.fract()) as f64) {
            hops += 1;
        }
        for _ in 0..hops {
            let nb: Vec<usize> = self.neighbours[p].iter().copied().filter(|&k| (class(self.patches[k].submersion) - home).abs() <= 1).collect();
            if nb.is_empty() {
                break;
            }
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
        // Motile sheets float at the surface: they shade each other and the
        // beds beneath them. Their leaf area per patch sets the light left for
        // the plants and each sheet's share of the surface light.
        let mut motile_area: Vec<f32> = vec![0.0; n_patches];
        for a in &self.animals {
            let lin = (a.mass / a.traits.mass.max(1e-6)).clamp(0.01, 1.0).powf(1.0 / 3.0);
            motile_area[a.patch] += a.traits.membrane_area * lin * lin;
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
                p.height = if is_water { want.min(1.5) } else { want }.max(0.0002);
            }
            let mut order = list.clone();
            order.sort_by(|a, b| self.plants[*b].height.partial_cmp(&self.plants[*a].height).unwrap());
            // Light arrives per square metre; `remaining` is in m^2 of full sun.
            // Water attenuates it, which is the pull toward land.
            let patch_plant_mass: f32 = order.iter().map(|&i| self.plants[i].biomass).sum();
            let surface_shade = (-0.5 * motile_area[pi] / a).exp();
            let mut remaining = light * a * (0.3 * sub + (1.0 - sub)) * surface_shade;
            let mut intercepts = Vec::with_capacity(order.len());
            for &i in &order {
                let p = &self.plants[i];
                let leaf_area = p.leaf_area();
                let frac = 1.0 - (-0.5 * leaf_area / a).exp();
                let got = remaining * frac;
                remaining -= got;
                intercepts.push((i, got, leaf_area));
            }
            let light_left = if light > 0.0 { (remaining / (light * a * (0.3 * sub + (1.0 - sub)))).clamp(0.0, 1.0) } else { 0.0 };
            // Nutrient is drawn as tissue is made, tallest first (the
            // dominants get theirs); what is left limits the rest.
            let mut nutrient_left = patch.nutrient;
            let mut nutrient_used = 0.0;
            let mut litter_today = 0.0;
            // Osmotrophy: organisms absorb dead organic matter through their surface
            // in proportion to their heterotrophy gene; the pool is shared.
            let dom_density = patch.dead_matter / a; // kg/m^2
            // Depth: light reaching a plant's top falls off through the water above it.
            let depth = (-patch.elevation).max(0.0) * sub.min(1.0);
            // Suspended particles (bacteria and drifting fragments) per m^3, for phagocytosis.
            let particles = (patch.bacteria + SUSPENDED_FRACTION * patch_plant_mass) / (a * 2.0);
            let mut phago_demand = 0.0f32;
            for &i in &order {
                let p = &self.plants[i];
                phago_demand += p.genome.heterotrophy.clamp(0.0, 1.0) * p.leaf_area() * (CURRENT_M_S * sub + 0.02 * p.genome.motility.clamp(0.0, 1.0)) * 86400.0 * particles * CAPTURE_EFF;
            }
            let phago_share = if phago_demand > 0.0 { (patch.bacteria * 0.3 / phago_demand).min(1.0) } else { 1.0 };
            let mut bacteria_used = 0.0f32;
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
                // Roots take up what the soil's moisture lets them; leaves
                // transpire by area, less the cuticle and stomata the land
                // tolerance gene builds. Photosynthesis follows the water
                // (stomata close as the plant dries).
                let water_frac = if sub >= 1.0 {
                    1.0
                } else {
                    let uptake = root * patch.moisture * 0.6;
                    let demand = leaf_area * (0.002 + 0.015 * aridity) * (0.5 + tf) * (0.5 + (sla / 20.0).min(2.0)) * (1.0 - 0.8 * tol);
                    let dry = if demand > 0.0 { (uptake / demand).min(1.0) } else { 1.0 };
                    sub + (1.0 - sub) * dry
                };
                // Wilting: tissue that stays dry dies, unless it is the kind
                // that survives drying (the same gene). A fixed factor used to
                // take three quarters of an unadapted plant's photosynthesis in
                // air regardless of how wet the ground was; that was a wall at
                // the shore, not a water balance.
                let wilt = ((0.3 - water_frac).max(0.0) / 0.3) * (1.0 - sub);
                let habitat_eff = 1.0;
                // 0.02 kg dry mass per m^2 of fully lit leaf per day; the patch's
                // Beer-Lambert light budget caps canopy production at ~2 kg/day.
                // Tough, long-lived leaves photosynthesise slower per area (leaf economics).
                let tough = p.genome.leaf_toughness.clamp(0.0, 1.0);
                let het = p.genome.heterotrophy.clamp(0.0, 1.0);
                // Mixotroph's trade-off: absorption machinery displaces chloroplasts.
                let photo = (got * 0.02 * cf * tf * water_frac * habitat_eff * (1.0 - 0.35 * tough) * (1.0 - 0.6 * het)).min(nutrient_left / NUTRIENT_FRACTION);
                nutrient_left -= photo * NUTRIENT_FRACTION;
                nutrient_used += photo * NUTRIENT_FRACTION;
                let absorbed = het * p.leaf_area() * dom_density * OSMOTROPHY_RATE * dom_share * (0.3 + tf);
                dom_used += absorbed;
                // Phagocytosis: particles passing the surface, captured in proportion.
                // Motility pays before it is an animal: a swimming cell meets more
                // particles (2 cm/s per unit motility) and can hold nearer the light.
                let mot = p.genome.motility.clamp(0.0, 1.0);
                let captured = het * p.leaf_area() * (CURRENT_M_S * sub + 0.02 * mot) * 86400.0 * particles * CAPTURE_EFF * phago_share;
                bacteria_used += captured;
                // Light left after the water column above this plant's top. A
                // sheet with no stem and no root floats at the surface (the founding
                // algae); anything anchored sits on the bed.
                let floats = p.body().stem_mass <= 0.0 && p.body().root_mass <= 0.0;
                let depth_attn = if floats { 1.0 } else { (-WATER_EXTINCTION * ((depth - p.height) * (1.0 - mot)).max(0.0)).exp() };
                let got = got * depth_attn;
                // Leaves respire fast, woody and root tissue slowly.
                let leaf_m = p.leaf_mass();
                // Transport: feeding leaves through long stems costs upkeep per kg*m.
                let transport = p.body().transport * (p.biomass / p.body().target_mass).clamp(0.0, 1.0);
                // The same continuum from the sessile side: a plant with some
                // motility already pays that share of an animal's field rate.
                let mot_tissue = 0.0025 * p.genome.motility.clamp(0.0, 1.0);
                let respiration = (leaf_m * 0.004 + (p.biomass - leaf_m).max(0.0) * (0.0005 + mot_tissue) + transport * 0.002) * (0.3 + tf) * (1.0 + 0.3 * tol);
                production_today += photo;
                p.energy += photo + (absorbed + captured) * 0.6 - respiration - p.biomass * 0.02 * wilt * (1.0 - 0.8 * tol);
                // Leaf economics: thin leaves live weeks, thick ones years
                // (Wright et al. 2004: lifespan ~ LMA^1.5, a year at 0.1 kg/m^2).
                // Shed leaves are litter and must be regrown from energy.
                if leaf_m > 0.0 && sla > 0.0 {
                    let lma = 1.0 / sla;
                    let leaf_lifespan = (365.0 * (lma / 0.1).powf(1.5)).clamp(20.0, 2000.0);
                    let shed = leaf_m / leaf_lifespan;
                    p.biomass = (p.biomass - shed).max(1e-9);
                    litter_today += shed;
                }
                p.light_frac = if light > 0.0 { got / (light * a) } else { 0.0 };
                p.water_frac = water_frac;
                p.age += 1.0;
                // Growth up to the structural cap (water lifts the cap), then seeds.
                if p.energy > 0.0 {
                    let cap = p.body().target_mass * (sub + (1.0 - sub) * p.body().growth_cap.powi(3));
                    if p.biomass < cap {
                        let grow = p.energy.min(p.biomass * 0.05 + 1e-8).min(cap - p.biomass);
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
                let m = p.genome.motility.clamp(0.0, 1.0);
                let lifespan = (1.0 - m) * p.genome.lifespan.max(1.0) + m * 150.0 * p.body().target_mass.max(1e-7).powf(0.25) * p.genome.longevity.max(0.1);
                if p.energy < -p.biomass * 0.3 || p.age > lifespan {
                    dead_plants.push(i);
                }
            }
            let pt = &mut self.patches[pi];
            pt.nutrient = (pt.nutrient - nutrient_used).max(0.0);
            pt.dead_matter = (pt.dead_matter - dom_used).max(0.0) + litter_today;
            pt.bacteria = (pt.bacteria - bacteria_used).max(0.0);
            pt.light_left = light_left;
            pt.motile_area = motile_area[pi];
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
                {
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
        // What each body offers a grazer as a sheet, computed once: its
        // accessible leaf mass and its speed in its own cell.
        let sheet_info: Vec<(f32, f32, f32, f32)> = self
            .animals
            .iter()
            .map(|s| {
                if s.traits.membrane_mass <= 0.0 || s.mass <= 2e-7 {
                    return (0.0, 0.0, 0.0, 0.0);
                }
                let s_lin = (s.mass / s.traits.mass.max(1e-7)).clamp(0.01, 1.0).powf(1.0 / 3.0);
                let s_sub = self.patches[s.patch].submersion;
                let s_speed = (s.traits.water_speed * s_sub + s.traits.land_speed * (1.0 - s_sub)) * s.genome.motility.clamp(0.0, 1.0) * s_lin;
                let leaf = (s.traits.membrane_mass / s.traits.mass.max(1e-7)).min(1.0) * (s.mass - 2e-7);
                let tough = s.genome.leaf_toughness.clamp(0.0, 1.0);
                (leaf * 0.5 * (1.0 - 0.8 * tough), s_speed, s.traits.reach * s_lin, s.mass - 2e-7)
            })
            .collect();
        let mut killed: Vec<usize> = Vec::new();
        // Flag per animal, so the checks below stay constant time (a list scan
        // made a day of grazing deaths quadratic).
        let mut killed_flag: Vec<bool> = vec![false; self.animals.len()];
        let (mut graze_today, mut prey_today, mut carrion_today, mut cost_today, mut mats_today) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let n_animals = self.animals.len();
        let mut animals_in: Vec<Vec<usize>> = vec![Vec::new(); n_patches];
        for (i, a) in self.animals.iter().enumerate() {
            animals_in[a.patch].push(i);
        }
        // Tide: the intertidal band is under water half the day on average,
        // between a third and two thirds over a spring-neap cycle of 14.8 days.
        let tide = 0.5 + 0.17 * (2.0 * PI * self.day as f32 / 14.77).sin();
        for p in self.patches.iter_mut() {
            p.canopy = 0.0;
            if p.shore {
                p.submersion = shore_submersion(p.shore_level, tide);
            }
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
            if killed_flag[ai] {
                continue;
            }
            // --- read state -------------------------------------------------
            let (patch, in_water, adult_mass, mass, motility, _section, _freq, activity, _gut_type, fat_frac, gut_frac, membrane_area, water_speed_raw, land_speed_raw, measured_land, measured_water, eff_plant, eff_meat, prev_intake) = {
                let a = &self.animals[ai];
                let (ep, em) = a.efficiencies();
                (
                    a.patch,
                    self.patches[a.patch].water,
                    a.traits.mass.max(1e-7),
                    a.mass.max(1e-7),
                    a.genome.motility.clamp(0.0, 1.0),
                    a.traits.section_area,
                    a.traits.base_freq,
                    a.traits.activity,
                    a.genome.gut_type.clamp(0.0, 1.0),
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
                (t.reach.max(0.0002), t.mouth_area.max(1e-9), t.claws, t.plating, sb + (1.0 - sb) * t.upright, t.air_speed, t.measured_air, t.glide_distance / crate::sim::AIR_DROP_HEIGHT)
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
            // Flight over the ground is the stroke's: a body falling from a
            // launch moves through the air too, but only the lift its wings
            // beat (a metre of powered lift over the held wing is sustained
            // flight) carries it anywhere.
            let powered = self.animals[ai].traits.flap_lift.clamp(0.0, 1.0);
            let air_speed = air_speed_raw * motility * lin_f * (1.0 - 0.3 * plating) * powered;
            let travel = if in_water { speed_here } else { speed_here.max(air_speed) };

            // --- costs -------------------------------------------------------
            // Field metabolic rate: Kleiber exponent, ectotherm anchor, oxygen limit.
            // Maintenance is a continuum, not a kingdom: tissue upkeep like a
            // plant's (membranes 0.4%/day, the rest 0.05%/day), plus the
            // vertebrate field rate scaled by motility. A barely motile sheet pays
            // a sheet's bills; a fully motile body pays an animal's.
            // Maintenance is tissue upkeep: sheets at a leaf's 0.4%/day; other
            // tissue at 0.05%/day for a plant's wood rising to 0.3%/day for a fully
            // motile body (muscle tone, nerves, osmoregulation), with Q10 and the
            // oxygen limit. Locomotion is charged separately as the work done.
            let membrane_mass = (self.animals[ai].traits.membrane_mass * size_f).min(mass);
            // Kleiber: the bulk tissue's rate goes as mass to the three
            // quarters (the rates above are per kilogram at one kilogram), so
            // a gram of a large body costs less to keep than a gram of a small
            // one. A sheet is all surface and keeps a leaf's per-area rate.
            // A waterproofed integument (the land tolerance gene) costs upkeep,
            // as a plant's cuticle does.
            let waterproof = self.animals[ai].genome.land_tolerance.clamp(0.0, 1.0);
            let fmr = (membrane_mass * 0.004 + (mass - membrane_mass).max(0.0).powf(0.75) * (0.0005 + 0.0025 * motility)) * (0.3 + temp_factor(temp)) * q / o2.min(1.0).max(0.3) * (1.0 + 0.3 * waterproof);
            let upkeep = 0.0f32; // claws and plating are real parts and already cost mass
            // Short of oxygen (gills in air, lungs under water) the tissue runs
            // an anaerobic debt that costs it, continuously, up to 5% of the
            // body a day with no gas exchange at all.
            let upkeep = upkeep + mass * 0.05 * ((0.2 - resp_eff).max(0.0) / 0.2);
            // What the muscles can keep up all day: the oxygen budget in this
            // medium (gills under water, lungs in air), from the body's own tests.
            let activity = {
                let t = &self.animals[ai].traits;
                let fw = if t.fatigue_water > 0.0 { t.fatigue_water } else { activity };
                let fl = if t.fatigue_land > 0.0 { t.fatigue_land } else { activity };
                fw * sub + fl * (1.0 - sub)
            };
            // Moving costs what the gait episode measured: mechanical power at
            // full drive (scaled to this individual's mass) over the foraging
            // day, through muscle efficiency, in dry biomass. A body not yet
            // measured pays the muscle-section estimate instead.
            // Moving is charged by the metre actually walked today (below,
            // once the day's food is known), at the gait's measured cost of
            // transport; standing still costs nothing beyond maintenance.
            let activity_cost: f32;
            let mut delta = -(fmr + upkeep);
            // A motile organism with membranes still photosynthesises (Euglena,
            // not a sudden animal): the same leaf chemistry as a plant, halved for
            // a surface that is not held flat to the sun, and displaced by the
            // heterotrophy gene like a mixotroph's chloroplasts.
            {
                let het = self.animals[ai].genome.heterotrophy.clamp(0.0, 1.0);
                let leaf_now = membrane_area * lin_f * lin_f;
                if leaf_now > 0.0 && het < 1.0 {
                    let tf = temp_factor(temp);
                    // At the surface, sharing with every other floating sheet: the
                    // mat intercepts 1 - exp(-0.5 A/a) of the light and each sheet gets
                    // its share by area, so a crowded surface is a dim one.
                    let mat = self.patches[patch].motile_area.max(leaf_now);
                    let intercepted = light * a_area * (0.3 * sub + (1.0 - sub)) * (1.0 - (-0.5 * mat / a_area).exp());
                    // The same nutrient draw a plant makes: tissue needs its
                    // nutrient, and the cell's pool is what is left today.
                    let photo = ((1.0 - 0.6 * het) * intercepted * (leaf_now / mat) * 0.02 * cf * tf).min(self.patches[patch].nutrient / NUTRIENT_FRACTION);
                    self.patches[patch].nutrient -= photo * NUTRIENT_FRACTION;
                    delta += photo;
                    production_today += photo;
                }
            }
            cost_today += fmr + upkeep;
            // What is metabolised is excreted: its nutrient returns to the cell.
            self.patches[patch].nutrient += (fmr + upkeep) * NUTRIENT_FRACTION;

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
            let space_at_start = space;

            // --- foraging options, each as a Holling type II intake rate ---------
            // Bite rate from the mouth: trunk cross-section (juveniles scaled),
            // about a tenth of body mass per day for a well-proportioned kilogram.
            let bite_rate = (12.0 * mouth_area * lin_f * lin_f * q * resp_eff).max(1e-6);
            // The mouth's opening, for what can be swallowed whole.
            let gape = 2.0 * (mouth_area * lin_f * lin_f / PI).sqrt();
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
            let filtered = (clearance_per_kg(mass) * mass * sub / (a * 2.0)).min(1.0);
            let search = (search + SUSPENDED_FRACTION * filtered).min(1.0);
            // The filter also takes bacteria (eaten at meat-like quality).
            let bacteria_rate = filtered * self.patches[patch].bacteria * 0.5;

            // Plants here (accessible leaf mass), plus shore plants reachable from the water.
            let mut plant_targets: Vec<(usize, f32)> = Vec::new();
            let mut plant_b = 0.0f32;
            for &pi in &plants_in[patch] {
                let p = &self.plants[pi];
                if p.biomass < 4e-7 {
                    continue;
                }
                let lf = p.body().leaf_mass / p.body().target_mass;
                // Browsing height matters on land; floating algae are all within reach.
                let browse = if in_water { 1.0 } else { (reach_h / p.height.max(0.0002)).clamp(0.1, 1.0) };
                // Tender leaves are half takeable in a day; tough ones resist grazing.
                let tough = p.genome.leaf_toughness.clamp(0.0, 1.0);
                let acc = (p.biomass - 4e-7) * lf * 0.5 * (1.0 - 0.8 * tough) * browse;
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
                        if p.biomass < 4e-7 {
                            continue;
                        }
                        let lf = p.body().leaf_mass / p.body().target_mass;
                        let browse = if js >= 1.0 { 1.0 } else { (reach_h / p.height.max(0.0002)).clamp(0.1, 1.0) };
                        let tough = p.genome.leaf_toughness.clamp(0.0, 1.0);
                        let acc = (p.biomass - 4e-7) * lf * 0.5 * (1.0 - 0.8 * tough) * browse * spill;
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
                        if p.biomass < 4e-7 {
                            continue;
                        }
                        let lf = p.body().leaf_mass / p.body().target_mass;
                        let tough = p.genome.leaf_toughness.clamp(0.0, 1.0);
                        let acc = (p.biomass - 4e-7) * lf * 0.5 * (1.0 - 0.8 * tough) * amph;
                        if acc > 0.0 {
                            plant_targets.push((pi, acc));
                            plant_b += acc;
                        }
                    }
                }
            }
            // Motile sheets are leaf tissue too: the same bite takes them, as
            // far as the grazer can keep up with them (a drifting mat is as
            // grazeable as a floating alga; a sheet swimming away from a slower
            // grazer is not). The continuum has no kingdom line for a mouth.
            let mut sheet_targets: Vec<(usize, f32)> = Vec::new();
            for &j in &animals_in[patch] {
                if j == ai || killed_flag[j] {
                    continue;
                }
                let (base, s_speed, s_reach, s_whole) = sheet_info[j];
                if base <= 0.0 {
                    continue;
                }
                let keep_up = if speed_here > 0.0 { (1.0 - s_speed / speed_here).clamp(0.0, 1.0) } else if s_speed <= 0.0 { 1.0 } else { 0.0 };
                // A sheet small enough to pass the mouth whole is filtered from
                // the water the body clears (zooplankton on phytoplankton), on
                // top of what a bite takes.
                let swallow = if in_water && s_reach <= gape { filtered } else { 0.0 };
                let acc = base * keep_up + s_whole * swallow;
                if acc > 0.0 {
                    sheet_targets.push((j, acc));
                    plant_b += acc;
                }
            }
            // Competition is depletion: what one eats the next does not find.
            // (A Beddington-DeAngelis interference term per competitor used to
            // divide every intake by the crowd: at 350 grazers a cell it
            // throttled feeding fifty-fold, so nothing was ever food-limited,
            // the mats were never eaten down, and the smallest fastest breeder
            // won a scramble that no resource ever decided.)
            let plant_rate = search * plant_b / (1.0 + search * handling * plant_b); // kg/day
            let carrion_here = self.patches[patch].carrion;
            // Carrion is smelt from afar: the detection strip is three times wider.
            // A flier surveys from height: a far wider strip (vultures find carcasses from kilometres up).
            let strip = if sub < 1.0 && air_speed > 0.0 { 40.0 } else { 6.0 };
            let carrion_search = ((PI * reach_now * reach_now + strip * reach_now * path) / a).min(1.0);
            let carrion_rate = carrion_search * carrion_here / (1.0 + carrion_search * handling * carrion_here);
            {
                // Bacteria through the filter: cheap, meat-like food for anything that strains water.
                let take = bacteria_rate.min(self.patches[patch].bacteria * 0.3);
                self.patches[patch].bacteria -= take;
                delta += take * eff_meat;
                self.animals[ai].ate_carrion += take * eff_meat;
            }
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
                let mut candidates: Vec<usize> = animals_in[patch].iter().copied().filter(|&j| j != ai && !killed_flag[j]).collect();
                for &(j, cover) in &ring_cells {
                    for &k in &animals_in[j] {
                        if k != ai && !killed_flag[k] && self.rng.gen_bool(cover as f64) {
                            candidates.push(k);
                        }
                    }
                }
                let mut best: Option<(usize, f32, f32)> = None; // (idx, p_success, profitability)
                for _ in 0..candidates.len().min(3) {
                    let j = candidates[self.rng.gen_range(0..candidates.len())];
                    let prey = &self.animals[j];
                    let pm = prey.mass.max(1e-7);
                    let p_in_water = self.patches[prey.patch].water;
                    let p_sub = self.patches[prey.patch].submersion;
                    let p_lin = (pm / prey.traits.mass.max(1e-7)).clamp(0.01, 1.0).powf(1.0 / 3.0);
                    let prey_speed = (prey.traits.water_speed * p_sub + prey.traits.land_speed.max(prey.traits.air_speed) * (1.0 - p_sub)) * prey.genome.motility.clamp(0.0, 1.0) * p_lin * (1.0 - 0.3 * prey.traits.plating);
                    // Manoeuvrability decides chases as much as speed (Howland 1974):
                    // a turn rate of about 1.5 rad/s doubles either side's term.
                    let my_turn = self.animals[ai].traits.turn_water * sub + self.animals[ai].traits.turn_land.max(self.animals[ai].traits.turn_air) * (1.0 - sub);
                    let prey_turn = prey.traits.turn_water * p_sub + prey.traits.turn_land.max(prey.traits.turn_air) * (1.0 - p_sub);
                    // Chases scale with body size (Howland 1974): speeds in body
                    // lengths per second, a sessile party at the floor of 0.2.
                    let my_bl = speed_here / (2.0 * reach_now).max(1e-4);
                    let prey_bl = prey_speed / (2.0 * prey.traits.reach * p_lin).max(1e-4);
                    let attack = m34 * (0.2 + my_bl.min(10.0)) * (0.5 + 3.0 * claws) * (0.5 + 0.5 * upright) * (0.5 + 0.5 * (my_turn / 1.5).min(1.0));
                    // Taking off is the escape a ground predator cannot follow: a glider
                    // needs a perch (its patch's launch height), a flier needs nothing.
                    // Taking off: a glide from the cell's launch height, or the
                    // stroke's own lift (a metre of powered lift is a take-off).
                    let takeoff = if p_in_water {
                        0.0
                    } else {
                        let e = self.patches[prey.patch].elevation;
                        let drop = self.neighbours[prey.patch].iter().map(|&k| e - self.patches[k].elevation).fold(0.0f32, f32::max);
                        (prey.traits.glide_distance / crate::sim::AIR_DROP_HEIGHT * (self.patches[prey.patch].canopy + drop.min(50.0)) / 10.0 + prey.traits.flap_lift + prey.traits.air_speed).clamp(0.0, 1.0)
                    };
                    let defence = pm.powf(0.75) * (0.2 + prey_bl.min(10.0)) * (0.5 + 2.0 * prey.traits.plating) * (1.0 + 3.0 * takeoff) * (0.5 + 0.5 * (prey_turn / 1.5).min(1.0));
                    let scale = 0.25 * (attack + defence) + 1e-6;
                    // Encounter: detection distance scales with prey body length, so
                    // small prey are found at a fraction of the rate of large prey.
                    // Encounter: the predator meets the prey that its day's range
                    // covers, detecting a prey smaller than itself at a fraction of
                    // the rate set by how large it looks (its reach against the
                    // predator's own, at the prey's current size).
                    let looks = (prey.traits.reach * p_lin / reach_now.max(1e-3)).min(1.0);
                    let encounter = (range_area / a).min(1.0) * looks;
                    // Capture: attack versus defence, times a hump in log size ratio
                    // (Brose 2006: small ectotherm predators do best on prey 10-100x
                    // smaller; much larger prey is possible but rare).
                    let z = (mass / pm).log10();
                    let hump = (-(z - 1.0).powi(2) / 2.0).exp();
                    // The capture itself is fought out in the arena per pair of
                    // body types; until that pair has been through it, the
                    // attack-versus-defence form stands in as the prior.
                    let medium = if sub >= 0.5 || self.patches[prey.patch].water { Environment::Water } else { Environment::Land };
                    // No mouth and no claws means nothing to bite or hold with:
                    // no attempt, no fight. That is the arena's own rule applied
                    // before the arena, not a tuning.
                    let prior = hump / (1.0 + (-(attack - defence) / scale).exp());
                    let p_capture = match self.arena_outcome(self.animals[ai].lineage, prey.lineage, medium) {
                        Some(o) => o.eaten,
                        // Not fought yet: the prior, weighed with the predator's
                        // own record over the prey it has fought.
                        None => match self.arena_record(self.animals[ai].lineage) {
                            Some((sum, n)) => (prior * Self::ARENA_PRIOR_WEIGHT + sum) / (Self::ARENA_PRIOR_WEIGHT + n as f32),
                            None => prior,
                        },
                    };
                    let p_success = encounter * p_capture;
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
                        // The first real attack between two types sends the pair
                        // to the arena; later attacks use what it found.
                        let medium = if sub >= 0.5 || self.patches[self.animals[j].patch].water { Environment::Water } else { Environment::Land };
                        self.arena_request(ai, j, medium);
                        if self.rng.gen_bool(p_success.clamp(0.0, 1.0) as f64) {
                            let prey_mass = self.animals[j].mass;
                            let prey_reserve = self.animals[j].reserve.max(0.0);
                            let z = (mass / prey_mass.max(1e-7)).log10();
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
                            killed_flag[j] = true;
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
                        for (j, acc) in &sheet_targets {
                            let s = &mut self.animals[*j];
                            let bite = (acc * scale_take).min(s.mass * 0.9);
                            s.mass -= bite;
                            take -= bite;
                            food_plant += bite;
                            mats_today += bite;
                            // Grazed below a tenth of its size, a sheet is gone.
                            if s.mass < 0.1 * s.traits.mass {
                                killed.push(*j);
                                killed_flag[*j] = true;
                            }
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

            // --- the day's walking, and what it cost ---------------------------
            // The body forages until it has found what it can eat (the bite
            // rate, or the room in its gut), then stops: the path needed is the
            // search that brings the day's food to that, up to the whole day's
            // path. The metres walked are charged at the gait's measured cost of
            // transport (J per kg per m) through muscle efficiency; a body not
            // yet measured pays the field estimate of 10.7 J per kg^0.68 per m.
            {
                let food_found = plant_b + carrion_here;
                let want = bite_rate.min(space_at_start).max(0.0);
                let path_needed = if food_found <= 0.0 {
                    path
                } else {
                    let h = handling;
                    if h * want >= 1.0 {
                        path
                    } else {
                        let s_needed = want / (food_found * (1.0 - h * want));
                        ((s_needed * a - PI * reach_now * reach_now) / (2.0 * reach_now).max(1e-3)).clamp(0.0, path)
                    }
                };
                let t = &self.animals[ai].traits;
                let cot = t.cot_water * sub + t.cot_land * (1.0 - sub);
                let joules = if cot > 0.0 { cot * mass * g * path_needed / MUSCLE_EFFICIENCY } else { 10.7 * mass.powf(0.68) * path_needed };
                activity_cost = joules / J_PER_KG_DRY;
                delta -= activity_cost;
                cost_today += activity_cost;
                self.patches[patch].nutrient += activity_cost * NUTRIENT_FRACTION;
            }

            // --- water: loss through the skin in air, against drink and food ----
            // Evaporation is per square metre of body surface, so it is the
            // surface-to-mass ratio that decides who can live on dry land: a
            // sheet or a tiny body dries out in a day unless its integument is
            // waterproofed (the land tolerance gene) or the ground is wet.
            {
                let moisture = self.patches[patch].moisture;
                let tf = temp_factor(temp);
                let a = &mut self.animals[ai];
                let surface = (6.0 * (a.mass / 1000.0).powf(2.0 / 3.0)).max(2.0 * a.traits.membrane_area * (a.mass / a.traits.mass.max(1e-3)).powf(2.0 / 3.0));
                let loss = surface * EVAPORATION_KG_PER_M2_DAY * (1.0 - sub) * (1.0 - moisture) * (0.5 + tf) * (1.0 - 0.95 * waterproof);
                let food_water = food_plant * WATER_PER_KG_PLANT + (food_carrion + food_meat) * WATER_PER_KG_MEAT;
                // Dew settles on the body's own surface in proportion to the
                // air's moisture: a tenth of a millimetre or so a night.
                let dew = surface * DEW_KG_PER_M2 * moisture * (1.0 - sub);
                a.water = (a.water - loss + food_water + dew).min(0.0);
                // Open water is drunk to the fill for the share of the day the
                // body is in it (all of it at sea, a tenth at the high-water
                // mark). Humid ground is not a drink: inland the water is in the
                // food.
                a.water *= 1.0 - sub;
            }

            // --- movement: when hungry, toward the best reachable neighbour --------
            let hungry = delta < 0.0 && self.animals[ai].reserve < 0.5 * (0.05 * mass + fat_frac * mass.powf(1.19));
            if hungry && self.rng.gen_bool(0.6) {
                let directed = self.rng.gen_bool(0.5);
                let mut best: Option<(usize, f32)> = None;
                // A glider reaches the next patch when a launch carries it ten metres;
                // a flier ranges over everything within an hour's flight.
                let can_glide_over = measured_air && glide_ratio * launch_h >= patch_w;
                let flier = measured_air && air_speed > 0.0;
                // Relocation is a distance: a flier an hour's flight, a crawler or
                // swimmer its day's path, in cells of this width; the move is paid
                // per cell crossed at the gait's measured cost of transport.
                let reloc_m = path.max(air_speed * 3600.0);
                let range_hops = ((reloc_m / patch_w).round() as usize).clamp(1, 12);
                // Breadth-first over the neighbour graph up to range_hops, each
                // cell with the number of cells crossed to reach it.
                let candidates: Vec<(usize, usize)> = {
                    let mut seen = vec![patch];
                    let mut frontier = vec![patch];
                    let mut out: Vec<(usize, usize)> = Vec::new();
                    for hop in 1..=range_hops {
                        let mut next = Vec::new();
                        for &q in &frontier {
                            for &k in &self.neighbours[q] {
                                if !seen.contains(&k) {
                                    seen.push(k);
                                    next.push(k);
                                    out.push((k, hop));
                                }
                            }
                        }
                        if next.is_empty() {
                            break;
                        }
                        frontier = next;
                    }
                    out
                };
                let mut best_hops = 1usize;
                for (cand, hops) in candidates {
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
                        best_hops = hops;
                    }
                }
                if let Some((target, _)) = best {
                    let target_water = self.patches[target].water;
                    // The move costs the measured cost of transport (J per kg per
                    // metre, Tucker's form when unmeasured) over the cells crossed.
                    let t = &self.animals[ai].traits;
                    let cot = if target_water { t.cot_water } else { t.cot_land };
                    let dist = best_hops as f32 * patch_w;
                    delta -= if cot > 0.0 {
                        cot * mass * g * dist / MUSCLE_EFFICIENCY / J_PER_KG_DRY
                    } else {
                        10.7 * mass.powf(0.68) * dist / J_PER_KG_DRY
                    };
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
                // Dissolved matter is taken up through a wet surface: in water,
                // or on land in proportion to how wet the ground is.
                let wet = sub + (1.0 - sub) * self.patches[a.patch].moisture;
                let absorbed = (het * surface * wet * (pool / a_area) * OSMOTROPHY_RATE * q).min(pool * 0.2);
                self.patches[a.patch].dead_matter -= absorbed;
                // Phagocytosis: particles swept past the surface by water and by moving.
                let particles = self.patches[a.patch].bacteria / (a_area * 2.0);
                let captured = (het * surface * (CURRENT_M_S * sub + speed_here) * 86400.0 * particles * CAPTURE_EFF).min(self.patches[a.patch].bacteria * 0.3);
                self.patches[a.patch].bacteria -= captured;
                delta += (absorbed + captured) * 0.6;
                a.ate_plant += (absorbed + captured) * 0.6;
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
                // One gene for both kingdoms: reproduce at this fraction of full
                // size (the plant's repro_frac), so a sheet that turns motile keeps
                // the maturity its line evolved.
                let mature = a.mass >= a.genome.repro_frac.clamp(0.1, 0.98) * adult_mass;
                if mature {
                    let mass0 = (a.genome.offspring_frac.clamp(0.01, 0.5) * adult_mass).max(1e-8);
                    let cost = mass0 * 1.3; // body plus a starter reserve
                    if surplus >= cost && self.rng.gen_bool(0.8) {
                        a.reserve -= cost;
                        a.offspring += 1;
                        let (child, body_changed) = if self.rng.gen_bool(0.75) { a.genome.mutate_scaled(&mut self.rng, BODY_MUTATION_SCALE) } else { (a.genome.clone(), false) };
                        births.push(Birth { genome: child, patch: a.patch, lineage: a.lineage, body_changed, parent_traits: a.traits, mass0, reserve0: mass0 * 0.3 });
                    }
                    // Beyond two offspring's worth of store, surplus is wasted as heat.
                    let max_store = cap + 2.0 * cost;
                    if a.reserve > max_store {
                        a.reserve = max_store;
                    }
                }
            }
            if a.reserve < 0.0 {
                // Starved; counted as suffocated when it was the oxygen debt
                // (a gill-breather on dry land, a lung-breather under water).
                if resp_eff < 0.2 {
                    self.stats.deaths_suffocated += 1;
                } else {
                    self.stats.deaths_starved += 1;
                }
                deaths.push(ai);
            } else if a.water < -(LETHAL_WATER_LOSS + a.genome.water_store.clamp(0.0, 0.5)) * a.mass {
                self.stats.deaths_dried += 1;
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
                    plant.biomass = b.mass0.min(plant.body().target_mass).max(2e-7);
                    plant.energy = b.reserve0;
                    self.plants.push(plant);
                    self.stats.animal_to_plant += 1;
                    self.stats.births += 1;
                } else {
                    self.patches[b.patch].carrion += b.mass0;
                }
                continue;
            }
            // Same body as the parent: inherit its measured speeds instead of
            // spending physics on it (controller-only mutations are approximated).
            let key = hash_genome(&b.genome);
            if !b.body_changed {
                self.trait_cache.entry(key.clone()).or_insert(b.parent_traits);
            } else {
                // A changed body is accounted as its parent's body, on both
                // sides of the ledger (mass and cost as well as speed), until
                // its own physics has run: a prior that is neutral, so a queue
                // of unmeasured bodies cannot select against new parts by
                // charging for them before they are credited.
                let mut prior = b.parent_traits;
                prior.measured_land = false;
                prior.measured_water = false;
                prior.measured_air = false;
                prior.measured_shore = false;
                self.trait_cache.entry(key.clone()).or_insert(prior);
            }
            let traits = self.lookup_traits(&b.genome);
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
        self.stats.mat_grazing = mats_today;
        self.stats.predation_intake = prey_today;
        self.stats.scavenging_intake = carrion_today;
        self.stats.metabolic_cost = cost_today;

        // ---------------- Decomposition and nutrient return ------------------
        for pt in self.patches.iter_mut() {
            let tf = temp_factor(pt.temp_c);
            let rate = 0.03 * (0.2 + tf) * (0.3 + 0.7 * pt.moisture);
            // Microbial loop: decomposers turn dead matter into their own biomass
            // (growth yield 0.4) and mineral nutrient; bacteria in turn respire
            // and die back to nutrient at a fifth a day, unless eaten first.
            let dec = pt.dead_matter * rate;
            pt.dead_matter -= dec;
            pt.bacteria += dec * BACTERIAL_YIELD;
            // The decayed tissue's nutrient is mineralised, except what the
            // decomposers keep in their own biomass until they turn over.
            pt.nutrient += dec * (1.0 - BACTERIAL_YIELD) * NUTRIENT_FRACTION;
            let turnover = pt.bacteria * 0.2 * (0.2 + tf);
            pt.bacteria -= turnover;
            pt.nutrient += turnover * NUTRIENT_FRACTION;
            let rot = pt.carrion * (rate * 3.0).min(0.5);
            pt.carrion -= rot;
            pt.nutrient += rot * NUTRIENT_FRACTION;
            // Slow weathering input and leaching, in balance at the initial pool.
            let n0 = NUTRIENT0_PER_M2 * self.patch_area;
            pt.nutrient = (pt.nutrient + n0 * 0.001 - pt.nutrient * 0.001).max(0.0);
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
        s.powered = 0;
        s.land_herbivores = 0;
        s.best_land_speed = 0.0;
        s.omnivores = 0;
        s.mean_carn = 0.0;
        s.mean_detr = 0.0;
        s.mean_mass = 0.0;
        s.mean_parts = 0.0;
        let na = self.animals.len().max(1) as f32;
        s.land_plants = self.plants.iter().filter(|p| !self.patches[p.patch].water).count();
        s.land_plant_biomass = self.plants.iter().filter(|p| !self.patches[p.patch].water).map(|p| p.biomass).sum();
        s.inland_animals = 0;
        s.inland_mass = 0.0;
        for a in &self.animals {
            if self.patches[a.patch].submersion <= 0.0 {
                s.inland_animals += 1;
                s.inland_mass += a.mass;
            }
        }
        s.inland_mass /= s.inland_animals.max(1) as f32;
        s.land_patches = self.patches.iter().filter(|p| !p.water).count();
        {
            let mut veg = vec![false; self.patches.len()];
            for p in &self.plants {
                if !self.patches[p.patch].water && p.biomass > 5e-6 {
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
            if a.traits.flap_lift >= 0.5 || a.traits.glide_distance >= crate::sim::AIR_DROP_HEIGHT {
                s.fliers += 1;
            }
            if a.traits.flap_lift >= 0.5 {
                s.powered += 1;
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
            waterproof: 0.0,
            flat: 0.0,
            mouth: 0.0,
            intake: 0.0,
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
            lift: 0.0,
            limbs: 0.0,
            clearance: 0.0,
            cot: 0.0,
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
                "gait" => l.land_speed * l.clearance * l.limbs,
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
                    s.lift += a.traits.flap_lift / c;
                    s.limbs += a.traits.stepping_limbs / c;
                    s.clearance += a.traits.trunk_clearance / c;
                    s.cot += a.traits.cot_land / c;
                    s.glide += a.traits.glide_distance / c;
                    s.turn += (if self.patches[a.patch].water { a.traits.turn_water } else { a.traits.turn_land.max(a.traits.turn_air) }) / c;
                    s.climb += a.traits.climb_water.max(a.traits.climb_air) / c;
                    s.shore += a.traits.shore / c;
                    s.breathing += a.genome.breathing / c;
                    s.shore_share += if self.patches[a.patch].shore { 1.0 / c } else { 0.0 };
                    let sb = self.patches[a.patch].submersion;
                    let sp = a.traits.water_speed * sb + a.traits.land_speed * (1.0 - sb);
                    s.speed_bl += sp / (2.0 * a.traits.reach).max(1e-4) / c;
                    s.parts += decode(&a.genome).part_count() as f32 / c;
                    s.membrane_area += a.traits.membrane_area / c;
                    s.mouth += a.traits.mouth_area / c;
                    s.intake += a.intake_rate / a.mass.max(1e-7) / c;
                    s.lung_frac += a.genome.lung_frac / c;
                    s.waterproof += a.genome.land_tolerance / c;
                    s.flat += a.genome.nodes[a.genome.root].flat() / c;
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
            terrain::elevation_at_res(p.pos, self.terrain_seed, self.patches.len()) - p.elevation
        };
        let mut positions = Vec::with_capacity(ico.positions.len() * 3);
        let mut vertex_patch = Vec::with_capacity(ico.positions.len());
        let mut vertex_elevation = Vec::with_capacity(ico.positions.len());
        let mut vertex_temp = Vec::with_capacity(ico.positions.len());
        for v in &ico.positions {
            positions.extend_from_slice(v);
            let patch = terrain::nearest_patch(&centres, *v);
            vertex_patch.push(patch as u32);
            let e = terrain::elevation_at_res(*v, self.terrain_seed, self.patches.len()) - sea_level_raw;
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
    fn a_checkpoint_restores_the_world() {
        let mut w = EcoWorld::new(PlanetParams::earth(), 300, 9);
        for _ in 0..200 {
            w.measure_pending(4);
            w.tick();
        }
        let path = std::env::temp_dir().join("evo_checkpoint_test.bin");
        let path = path.to_str().unwrap().to_string();
        w.save(&path).unwrap();
        let mut r = EcoWorld::load(&path).unwrap();
        assert_eq!(r.day, w.day);
        assert_eq!(r.plants.len(), w.plants.len());
        assert_eq!(r.animals.len(), w.animals.len());
        for _ in 0..5 {
            r.measure_pending(4);
            r.tick();
        }
        assert_eq!(r.day, w.day + 5);
        assert!(!r.plants.is_empty());
        let _ = std::fs::remove_file(&path);
    }

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
        // A mid-sized world. Stop as soon as land is colonised: with no
        // population guard a long run grows large and slow, and the question
        // here is only whether colonisation happens.
        let mut w = EcoWorld::new(PlanetParams::earth(), 800, 5);
        for _ in 0..2500 {
            w.tick();
            if w.stats.land_plants > 0 {
                break;
            }
        }
        assert!(w.stats.plants > 0);
        assert!(w.stats.land_plants > 0, "no land plants after 2500 days: {:?}", w.stats);
    }
}
