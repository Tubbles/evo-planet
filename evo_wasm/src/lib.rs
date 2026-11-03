//! Browser bindings. `EcoSim` runs the planet ecology (meant for a Web
//! Worker); `ReplaySim` steps one creature for rendering on the main thread.
//! Both use the same evo_core code, so the replay matches the measurement.

use evo_core::decode::decode;
use evo_core::genome::{Genome, NodeKind};
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

fn parse_env(s: &str) -> Environment {
    match s.to_lowercase().as_str() {
        "water" => Environment::Water,
        "air" => Environment::Air,
        "shore" => Environment::Shore,
        _ => Environment::Land,
    }
}

#[wasm_bindgen]
pub fn presets_json() -> String {
    serde_json::to_string(&PlanetParams::presets()).unwrap_or_default()
}

#[wasm_bindgen]
pub fn derive_json(planet_json: &str) -> String {
    match serde_json::from_str::<PlanetParams>(planet_json) {
        Ok(p) => serde_json::to_string(&p.derive()).unwrap_or_default(),
        Err(e) => format!("{{\"error\":\"{e}\"}}"),
    }
}

// ---------------------------------------------------------------- ecology

#[wasm_bindgen]
pub struct EcoSim {
    world: evo_core::eco::EcoWorld,
}

#[wasm_bindgen]
impl EcoSim {
    #[wasm_bindgen(constructor)]
    pub fn new(planet_json: &str, patches: usize, seed: u64) -> Result<EcoSim, JsValue> {
        let planet: PlanetParams = serde_json::from_str(planet_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(EcoSim { world: evo_core::eco::EcoWorld::new(planet, patches, seed) })
    }

    /// Inject `count` random immigrants into the sea now.
    /// Change the planet under the running world (see EcoWorld::set_planet).
    pub fn set_planet(&mut self, planet_json: &str) -> Result<(), JsValue> {
        let planet: PlanetParams = serde_json::from_str(planet_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.world.set_planet(planet);
        Ok(())
    }
    pub fn immigrate(&mut self, count: usize) {
        self.world.immigrate(count);
    }

    /// Static render geometry as JSON (positions, indices, per-vertex patch,
    /// elevation and temperature, patch centres). Fetch once per run.
    pub fn terrain_json(&self, detail: u32) -> String {
        serde_json::to_string(&self.world.terrain_mesh(detail)).unwrap_or_default()
    }

    /// Advance `days` ticks, running at most `evals` physics trait measurements first.
    pub fn tick(&mut self, days: usize, evals: usize) {
        self.world.measure_pending(evals);
        for _ in 0..days {
            self.world.tick();
        }
    }

    pub fn pending_evals(&self) -> usize {
        self.world.stats.pending_evals
    }

    pub fn snapshot_json(&self) -> String {
        serde_json::to_string(&self.world.snapshot()).unwrap_or_default()
    }

    /// Filtered, sorted lineage table (see EcoWorld::lineages_filtered).
    pub fn lineages_json(&self, kind: &str, habitat: &str, sort: &str, max: usize) -> String {
        serde_json::to_string(&self.world.lineages_filtered(kind, habitat, sort, max)).unwrap_or_default()
    }

    pub fn history_json(&self) -> String {
        serde_json::to_string(&self.world.history).unwrap_or_default()
    }

    /// Representative genome of an animal lineage as JSON {genome, env}, or "".
    pub fn representative_json(&self, lineage: u32, habitat: &str) -> String {
        match self.world.representative_in(lineage, habitat) {
            Some((g, env)) => format!(
                "{{\"genome\":{},\"env\":\"{}\"}}",
                serde_json::to_string(&g).unwrap_or_default(),
                match env {
                    Environment::Water => "water",
                    Environment::Air => "air",
                    _ => "land",
                }
            ),
            None => String::new(),
        }
    }

    pub fn day(&self) -> u64 {
        self.world.day
    }

    /// Largest plant of a lineage, decoded for display: JSON {genome, body}, or "".
    pub fn plant_representative_json(&self, lineage: u32) -> String {
        match self.world.plant_representative(lineage) {
            Some(g) => {
                let body = evo_core::plant::decode_plant(&g, self.world.planet.gravity);
                format!(
                    "{{\"genome\":{},\"body\":{}}}",
                    serde_json::to_string(&g).unwrap_or_default(),
                    serde_json::to_string(&body).unwrap_or_default()
                )
            }
            None => String::new(),
        }
    }
}

/// Decode any genome as a plant body for display.
#[wasm_bindgen]
pub fn plant_body_json(genome_json: &str, gravity: f32) -> String {
    match serde_json::from_str::<Genome>(genome_json) {
        Ok(g) => serde_json::to_string(&evo_core::plant::decode_plant(&g, gravity)).unwrap_or_default(),
        Err(e) => format!("{{\"error\":\"{e}\"}}"),
    }
}

#[derive(Serialize)]
struct PartOut {
    kind: &'static str,
    flat: f32,
    grip: f32,
    len: f32,
    radius: f32,
    r0: f32,
    parent: i32,
}

#[wasm_bindgen]
pub struct ReplaySim {
    sim: CreatureSim,
}

#[wasm_bindgen]
impl ReplaySim {
    #[wasm_bindgen(constructor)]
    pub fn new(genome_json: &str, planet_json: &str, env: &str, episode_time: f32) -> Result<ReplaySim, JsValue> {
        let genome: Genome = serde_json::from_str(genome_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let planet: PlanetParams = serde_json::from_str(planet_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let cfg = SimConfig { episode_time, ..Default::default() };
        Ok(ReplaySim { sim: CreatureSim::new(decode(&genome), &planet, parse_env(env), cfg) })
    }

    pub fn step(&mut self, n: usize) {
        for _ in 0..n {
            self.sim.step();
        }
    }

    pub fn finished(&self) -> bool {
        self.sim.finished()
    }

    pub fn parts_json(&self) -> String {
        let parts: Vec<PartOut> = self
            .sim
            .pheno
            .parts
            .iter()
            .map(|p| PartOut {
                kind: match p.kind {
                    NodeKind::Capsule => "capsule",
                    NodeKind::Membrane => "membrane",
                    NodeKind::Pad => "pad",
                },
                flat: p.flat,
                grip: p.grip,
                len: p.len,
                radius: p.radius,
                r0: p.r0,
                parent: p.parent.map(|x| x as i32).unwrap_or(-1),
            })
            .collect();
        serde_json::to_string(&parts).unwrap_or_default()
    }

    /// 7 floats per part: x y z qx qy qz qw.
    pub fn transforms(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.sim.pheno.parts.len() * 7);
        for i in 0..self.sim.pheno.parts.len() {
            let iso = self.sim.part_isometry(i);
            let t = iso.translation.vector;
            let q = iso.rotation;
            out.extend_from_slice(&[t.x, t.y, t.z, q.i, q.j, q.k, q.w]);
        }
        out
    }

    pub fn com(&self) -> Vec<f32> {
        let c = self.sim.com();
        vec![c.x, c.y, c.z]
    }

    pub fn time(&self) -> f32 {
        self.sim.time
    }
    pub fn distance(&self) -> f32 {
        self.sim.horizontal_distance()
    }
    /// Bed gradient toward +X (shore replay); 0 elsewhere.
    pub fn slope(&self) -> f32 {
        self.sim.slope
    }
    pub fn ground_y(&self) -> f32 {
        self.sim.ground_y
    }
    pub fn water_level(&self) -> f32 {
        self.sim.water_level.unwrap_or(f32::NAN)
    }
    pub fn fatigue(&self) -> f32 {
        self.sim.fatigue
    }
    pub fn hydration(&self) -> f32 {
        self.sim.hydration
    }
    pub fn exploded(&self) -> bool {
        self.sim.exploded
    }
    pub fn airborne(&self) -> bool {
        self.sim.is_airborne()
    }
    pub fn total_mass(&self) -> f32 {
        self.sim.pheno.total_mass
    }
    pub fn membrane_area(&self) -> f32 {
        self.sim.pheno.membrane_area
    }
    pub fn lung_frac(&self) -> f32 {
        self.sim.pheno.lung_frac
    }
    pub fn water_store(&self) -> f32 {
        self.sim.pheno.water_store
    }
    pub fn base_freq(&self) -> f32 {
        self.sim.pheno.base_freq
    }
    pub fn part_count(&self) -> usize {
        self.sim.pheno.part_count()
    }
    pub fn membrane_count(&self) -> usize {
        self.sim.pheno.membrane_count()
    }
}
