//! The arena: two bodies in one physics world, a predator that steers at
//! its prey and a prey that steers away, in the medium they met in.
//!
//! Whether one body type can eat another is not a formula here; it is
//! fought out. A bout ends when the predator has held the prey with its
//! mouth or its clawed feet for `HOLD_TO_SUBDUE` seconds (eaten), or when
//! time runs out: if no bite ever landed and the prey is farther away than
//! it started, it fled; otherwise it survived (bitten but not held, or
//! neither party got anywhere). A pair of body types is fought `BOUTS`
//! times from perturbed starts; the three shares are cached by the ecology
//! and reused for every meeting of the same two types.
//!
//! What is physical about it: the chase runs on the bodies' measured gaits
//! and turning, steered by a bearing controller; a bite needs the mouth
//! (the front of the trunk, with a gape of its diameter) to reach a prey
//! part that fits in it; claws are gripping feet in contact. Nothing about
//! size, speed, weapons or armour enters except through the bodies.

use crate::decode::{body_metrics, Part, Phenotype};
use crate::planet::PlanetParams;
use crate::sim::{add_ground, apply_fluid_forces, build_creature, fatigue_factor, update_motors, Built, Drive, Environment, WorldCtx, WATER_DEPTH};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rapier3d::na::Vector3;
use rapier3d::prelude::*;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Seconds a bout may run.
pub const BOUT_TIME: f32 = 6.0;
/// Seconds of mouth or claw hold that subdue the prey.
pub const HOLD_TO_SUBDUE: f32 = 1.0;
/// Bouts per pair of body types.
pub const BOUTS: usize = 10;
/// Settling time before the motors engage (heavy damping, as in episodes).
pub const SETTLE: f32 = 0.5;
/// Cost of one pair in units of a single-body measurement episode, for the
/// ecology's physics budget: ten bouts of two bodies for six seconds each.
pub const PAIR_COST: usize = 24;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ArenaOutcome {
    /// Share of bouts in which the predator subdued the prey.
    pub eaten: f32,
    /// Share in which the prey got away without ever being bitten.
    pub fled: f32,
    /// Share in which neither happened.
    pub survived: f32,
    pub bouts: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoutResult {
    Eaten,
    Fled,
    Survived,
}

struct Fighter {
    pheno: Phenotype,
    handles: Vec<RigidBodyHandle>,
    joint_handles: Vec<Option<ImpulseJointHandle>>,
    fatigue: f32,
    /// Which way positive steer turns this body (+1 or -1), from its turn test.
    turn_sign: f32,
}

impl Fighter {
    fn com(&self, bodies: &RigidBodySet) -> Vector3<f32> {
        let mut sum = Vector3::zeros();
        let mut m = 0.0;
        for (i, h) in self.handles.iter().enumerate() {
            if let Some(rb) = bodies.get(*h) {
                sum += rb.translation() * self.pheno.parts[i].mass;
                m += self.pheno.parts[i].mass;
            }
        }
        if m > 0.0 { sum / m } else { sum }
    }
    /// The trunk's forward direction (its own +Y), horizontal.
    fn heading(&self, bodies: &RigidBodySet) -> Vector3<f32> {
        let d = bodies.get(self.handles[0]).map(|rb| rb.position().rotation * Vector3::y()).unwrap_or_else(Vector3::x);
        let mut h = Vector3::new(d.x, 0.0, d.z);
        if h.norm() < 1e-4 {
            h = Vector3::x();
        }
        h.normalize()
    }
    /// Steer so the heading turns toward `target` (or away from it).
    fn steer_toward(&self, bodies: &RigidBodySet, target: Vector3<f32>, away: bool) -> f32 {
        let com = self.com(bodies);
        let mut to = target - com;
        to.y = 0.0;
        if away {
            to = -to;
        }
        if to.norm() < 1e-4 {
            return 0.0;
        }
        let to = to.normalize();
        let h = self.heading(bodies);
        // Signed bearing about the vertical: positive means the target is to
        // the heading's left (right-hand rule about +Y).
        let bearing = (h.x * to.z - h.z * to.x).atan2(h.dot(&to));
        (self.turn_sign * bearing / (PI / 2.0)).clamp(-1.0, 1.0)
    }
}

pub struct ArenaSim {
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    ccd: CCDSolver,
    query_pipeline: QueryPipeline,
    params: IntegrationParameters,
    gravity: Vector3<f32>,
    ctx: WorldCtx,
    pred: Fighter,
    prey: Fighter,
    pub time: f32,
    settled: bool,
    start_dist: f32,
    /// Seconds of mouth or claw hold so far.
    pub hold: f32,
    pub bitten: bool,
    pub exploded: bool,
}

impl ArenaSim {
    /// Predator at the origin facing +X; prey ahead of it at a distance set
    /// by both reaches, facing away, turned by `prey_yaw` and offset by
    /// `prey_side` across the line.
    #[allow(clippy::too_many_arguments)]
    pub fn new(pred: Phenotype, prey: Phenotype, planet: &PlanetParams, env: Environment, pred_sign: f32, prey_sign: f32, prey_yaw: f32, prey_side: f32) -> Self {
        let derived = planet.derive();
        let mut bodies = RigidBodySet::new();
        let mut colliders = ColliderSet::new();
        let mut impulse_joints = ImpulseJointSet::new();
        let env = if env == Environment::Water { Environment::Water } else { Environment::Land };
        let (ground_y, water_level, spawn_y) = match env {
            Environment::Water => (-WATER_DEPTH, Some(0.0), -WATER_DEPTH * 0.5),
            _ => (0.0, None, 0.0),
        };
        add_ground(&mut bodies, &mut colliders, ground_y, 0.0);
        let reach_p = body_metrics(&pred).reach;
        let reach_q = body_metrics(&prey).reach;
        // The start gap is in the predator's own body sizes (three reaches
        // between the two surfaces), so the chase is the same in body
        // lengths whatever the pair's sizes. A fixed metre made every slow
        // small predator fail by distance alone; a gap in the prey's size
        // put a two-metre ribbon out of any small mouth's reach.
        let dist = reach_p + reach_q + 3.0 * reach_p + 0.05;
        let zero = Vector3::zeros();
        let bp = build_creature(&mut bodies, &mut colliders, &mut impulse_joints, &pred, env, &derived, ground_y, 0.0, spawn_y, 0.0, 0.0, 0.0, zero);
        let bq = build_creature(&mut bodies, &mut colliders, &mut impulse_joints, &prey, env, &derived, ground_y, 0.0, spawn_y, dist, prey_side, prey_yaw, zero);
        let Built { handles: hp, joint_handles: jp, fatigue: fp } = bp;
        let Built { handles: hq, joint_handles: jq, fatigue: fq } = bq;
        let _ = fatigue_factor;
        let mut params = IntegrationParameters::default();
        params.dt = 1.0 / 120.0;
        params.num_solver_iterations = std::num::NonZeroUsize::new(8).unwrap();
        let ctx = WorldCtx { rho_air: derived.air_density, rho_water: derived.water_density, g: planet.gravity, water_level, ground_y, slope: 0.0, dt: params.dt };
        let mut sim = Self {
            bodies,
            colliders,
            impulse_joints,
            multibody_joints: MultibodyJointSet::new(),
            pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            ccd: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
            params,
            gravity: vector![0.0, -planet.gravity, 0.0],
            ctx,
            pred: Fighter { pheno: pred, handles: hp, joint_handles: jp, fatigue: fp, turn_sign: if pred_sign < 0.0 { -1.0 } else { 1.0 } },
            prey: Fighter { pheno: prey, handles: hq, joint_handles: jq, fatigue: fq, turn_sign: if prey_sign < 0.0 { -1.0 } else { 1.0 } },
            time: 0.0,
            settled: false,
            start_dist: 0.0,
            hold: 0.0,
            bitten: false,
            exploded: false,
        };
        sim.start_dist = sim.distance();
        for f in [&sim.pred, &sim.prey] {
            for h in &f.handles {
                if let Some(rb) = sim.bodies.get_mut(*h) {
                    rb.set_linear_damping(3.0);
                    rb.set_angular_damping(3.0);
                }
            }
        }
        sim
    }

    pub fn distance(&self) -> f32 {
        let a = self.pred.com(&self.bodies);
        let b = self.prey.com(&self.bodies);
        let d = a - b;
        (d.x * d.x + d.z * d.z).sqrt()
    }

    /// World position of a point on a part's axis at `along` (-1 base .. +1 tip).
    fn axis_point(&self, f: &Fighter, i: usize, along: f32) -> Vector3<f32> {
        let p = &f.pheno.parts[i];
        let iso = self.bodies[f.handles[i]].position();
        iso.translation.vector + iso.rotation * Vector3::new(0.0, along * p.len * 0.5, 0.0)
    }

    /// Distance from a point to a part's axis segment, minus that part's
    /// radius: how far outside the part's surface the point is.
    fn gap_to_part(&self, f: &Fighter, i: usize, point: Vector3<f32>) -> f32 {
        let p: &Part = &f.pheno.parts[i];
        let a = self.axis_point(f, i, -1.0);
        let b = self.axis_point(f, i, 1.0);
        let ab = b - a;
        let t = if ab.norm_squared() > 1e-9 { ((point - a).dot(&ab) / ab.norm_squared()).clamp(0.0, 1.0) } else { 0.0 };
        let closest = a + ab * t;
        let r = if p.flat > 0.5 { p.r0 } else { p.radius };
        (point - closest).norm() - r
    }

    /// Does the predator hold the prey this step: its mouth (the trunk's
    /// front, gape = the trunk's diameter) on a prey part that fits in it,
    /// or a clawed foot on any prey part.
    fn holding(&self) -> (bool, bool) {
        let trunk = &self.pred.pheno.parts[0];
        // The gape is the trunk's thickness at the front: a capsule's diameter,
        // a sheet's edge.
        let gape = trunk.thickness * (trunk.r0 / (0.5 * (trunk.r0 + trunk.r1)).max(1e-6));
        let mouth = self.axis_point(&self.pred, 0, 1.0);
        let mut bite = false;
        let mut claw = false;
        for (j, q) in self.prey.pheno.parts.iter().enumerate() {
            let thickness = q.thickness;
            if gape > 0.0 && thickness <= gape && self.gap_to_part(&self.prey, j, mouth) <= 0.5 * gape {
                bite = true;
            }
        }
        for (i, p) in self.pred.pheno.parts.iter().enumerate() {
            if p.grip < 0.5 {
                continue;
            }
            let tip = self.axis_point(&self.pred, i, 1.0);
            for j in 0..self.prey.pheno.parts.len() {
                if self.gap_to_part(&self.prey, j, tip) <= p.radius + 0.01 {
                    claw = true;
                }
            }
        }
        (bite, claw)
    }

    pub fn step(&mut self) {
        if self.exploded {
            return;
        }
        if !self.settled && self.time >= SETTLE {
            self.settled = true;
            for f in [&self.pred, &self.prey] {
                for h in &f.handles {
                    if let Some(rb) = self.bodies.get_mut(*h) {
                        rb.set_linear_damping(0.05);
                        rb.set_angular_damping(0.2);
                    }
                }
            }
        }
        let prey_com = self.prey.com(&self.bodies);
        let pred_com = self.pred.com(&self.bodies);
        let steer_p = self.pred.steer_toward(&self.bodies, prey_com, false);
        let steer_q = self.prey.steer_toward(&self.bodies, pred_com, true);
        for (f, steer) in [(&self.pred, steer_p), (&self.prey, steer_q)] {
            let dr = Drive { active: self.settled, steer, bias_shift: 0.0, amp_scale: 1.0 };
            let _ = update_motors(&self.bodies, &mut self.impulse_joints, &f.pheno.parts, &f.handles, &f.joint_handles, f.pheno.base_freq, f.fatigue, self.time, dr);
        }
        apply_fluid_forces(&self.ctx, &mut self.bodies, &self.pred.pheno.parts, &self.pred.handles);
        apply_fluid_forces(&self.ctx, &mut self.bodies, &self.prey.pheno.parts, &self.prey.handles);
        self.pipeline.step(
            &self.gravity,
            &self.params,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd,
            Some(&mut self.query_pipeline),
            &(),
            &(),
        );
        self.time += self.ctx.dt;
        // Contact is contact, settling or not (the start distance keeps the
        // parties apart during settling in a real bout).
        let (bite, claw) = self.holding();
        if bite || claw {
            self.hold += self.ctx.dt;
            self.bitten = true;
        }
        for f in [&self.pred, &self.prey] {
            for h in &f.handles {
                let rb = &self.bodies[*h];
                let v = rb.linvel();
                let t = rb.translation();
                if !v.iter().all(|c| c.is_finite()) || !t.iter().all(|c| c.is_finite()) || v.norm() > 60.0 {
                    self.exploded = true;
                }
            }
        }
    }

    pub fn finished(&self) -> bool {
        self.exploded || self.hold >= HOLD_TO_SUBDUE || self.time >= SETTLE + BOUT_TIME
    }

    pub fn result(&self) -> BoutResult {
        if self.exploded {
            // A numerical failure is nobody's kill.
            BoutResult::Survived
        } else if self.hold >= HOLD_TO_SUBDUE {
            BoutResult::Eaten
        } else if !self.bitten && self.distance() > self.start_dist {
            BoutResult::Fled
        } else {
            BoutResult::Survived
        }
    }

    pub fn run_bout(pred: Phenotype, prey: Phenotype, planet: &PlanetParams, env: Environment, pred_sign: f32, prey_sign: f32, prey_yaw: f32, prey_side: f32) -> BoutResult {
        let mut sim = ArenaSim::new(pred, prey, planet, env, pred_sign, prey_sign, prey_yaw, prey_side);
        while !sim.finished() {
            sim.step();
        }
        sim.result()
    }
}

/// Fight a pair of body types `BOUTS` times from perturbed starts (the prey's
/// facing within half a radian of straight away, its side offset within
/// half its reach) and return the shares of outcomes.
pub fn run_pair(pred: &Phenotype, prey: &Phenotype, planet: &PlanetParams, env: Environment, pred_sign: f32, prey_sign: f32, seed: u64) -> ArenaOutcome {
    let mut rng = StdRng::seed_from_u64(seed);
    let reach_q = body_metrics(prey).reach;
    let mut out = ArenaOutcome::default();
    for _ in 0..BOUTS {
        let yaw = rng.gen_range(-0.5..0.5f32);
        let side = rng.gen_range(-0.5..0.5f32) * reach_q;
        match ArenaSim::run_bout(pred.clone(), prey.clone(), planet, env, pred_sign, prey_sign, yaw, side) {
            BoutResult::Eaten => out.eaten += 1.0,
            BoutResult::Fled => out.fled += 1.0,
            BoutResult::Survived => out.survived += 1.0,
        }
        out.bouts += 1;
    }
    let n = out.bouts.max(1) as f32;
    out.eaten /= n;
    out.fled /= n;
    out.survived /= n;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::decode;
    use crate::genome::Genome;

    #[test]
    fn a_motionless_pair_neither_eats_nor_flees() {
        // Two random bodies with their motors off: nothing can happen but
        // settling, so every bout is "survived".
        let planet = PlanetParams::earth();
        let a = decode(&Genome::random(3));
        let b = decode(&Genome::random(4));
        let mut sim = ArenaSim::new(a, b, &planet, Environment::Land, 1.0, 1.0, 0.0, 0.0);
        // Motors never engage if we never settle: run only the settling time.
        while sim.time < SETTLE - 0.01 {
            sim.step();
        }
        assert!(!sim.bitten);
        assert_eq!(sim.result(), BoutResult::Survived);
    }

    #[test]
    fn a_mouth_on_a_small_prey_is_a_bite() {
        // The prey placed inside the predator's gape at the start: contact is
        // a hold from the first step, and a second of it is a kill.
        let planet = PlanetParams::earth();
        let mut pred = Genome::random(5);
        for n in pred.nodes.iter_mut() {
            n.flat = 0.0;
        }
        pred.nodes[0].r0 = 0.2;
        pred.nodes[0].r1 = 0.2;
        pred.nodes[0].len = 0.6;
        let mut prey = Genome::random(6);
        for n in prey.nodes.iter_mut() {
            n.len *= 0.1;
            n.r0 *= 0.1;
            n.r1 *= 0.1;
            n.flat = 0.0;
        }
        let pp = decode(&pred);
        let pq = decode(&prey);
        let mut sim = ArenaSim::new(pp, pq, &planet, Environment::Land, 1.0, 1.0, 0.0, 0.0);
        // Set the prey down touching the predator's mouth (not inside it, or
        // the solver throws it out before anything can be held).
        let mouth = sim.axis_point(&sim.pred, 0, 1.0);
        let root = sim.prey.handles[0];
        let r_prey = sim.prey.pheno.parts[0].radius + sim.prey.pheno.parts[0].len * 0.5;
        let target = mouth + Vector3::new(r_prey + 0.005, 0.0, 0.0);
        let delta = target - sim.bodies[root].translation();
        for h in sim.prey.handles.clone() {
            let rb = &mut sim.bodies[h];
            let t = *rb.translation() + delta;
            rb.set_translation(t, true);
        }
        while !sim.finished() {
            sim.step();
        }
        assert!(sim.bitten, "no bite registered with the prey in the mouth");
    }
}
