//! Rapier-backed creature simulation.
//!
//! One `CreatureSim` is one creature in one world. The same code runs the
//! headless fitness oracle and the visible replay, so the champion behaves
//! exactly as it was scored.
//!
//! Forces beyond rigid-body contact:
//! * per-part anisotropic quadratic drag in the surrounding fluid,
//! * per-membrane flat-plate normal force (C_N = C_Pa * sin(alpha)) plus
//!   parasitic drag, sampled on a 2x2 sub-panel grid so lift makes torque,
//! * Archimedes buoyancy with submersion-proportional damping in water,
//! * CPG-driven position motors whose strength is scaled by an oxygen budget
//!   (lung fraction x torso volume x pO2 against muscle section x frequency).

use crate::decode::{Phenotype, MEMBRANE_THICKNESS};
use crate::genome::{JointKind, NodeKind};
use crate::planet::{Derived, PlanetParams};
use rapier3d::na::{Isometry3, Point3, UnitQuaternion, Vector3};
use rapier3d::prelude::*;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Environment {
    /// Flat ground, atmosphere. Fitness: horizontal distance.
    Land,
    /// Submerged in water, ground far below. Fitness: horizontal distance.
    Water,
    /// Dropped from a height over ground. Fitness: horizontal distance at
    /// landing plus time aloft. Rewards gliding, then flapping.
    Air,
}

impl Environment {
    pub const ALL: [Environment; 3] = [Environment::Land, Environment::Water, Environment::Air];
    pub fn name(self) -> &'static str {
        match self {
            Environment::Land => "Land",
            Environment::Water => "Water",
            Environment::Air => "Air (drop)",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SimConfig {
    pub dt: f32,
    pub settle_time: f32,
    pub episode_time: f32,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self { dt: 1.0 / 120.0, settle_time: 1.0, episode_time: 8.0 }
    }
}

/// Flat-plate normal-force slope (Lu et al. 2026 form: Cl = C_Pa sin a cos a).
pub const C_P_ALPHA: f32 = 2.61;
/// Parasitic drag of a thin panel at zero incidence.
pub const CD0_PANEL: f32 = 0.09;
pub const CD_CAPSULE_AXIAL: f32 = 0.5;
pub const CD_CAPSULE_LATERAL: f32 = 1.1;
/// Oxygen budget scale: see `fatigue_factor`.
pub const O2_BUDGET_K: f32 = 40.0;
pub const AIR_DROP_HEIGHT: f32 = 10.0;
pub const WATER_DEPTH: f32 = 12.0;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EpisodeResult {
    pub fitness: f32,
    pub distance: f32,
    pub airborne_frac: f32,
    pub time_aloft: f32,
    pub exploded: bool,
    pub part_count: usize,
    pub area_per_mass: f32,
    pub fatigue: f32,
    pub hydration: f32,
    pub total_mass: f32,
    /// Mean centre-of-mass height above ground after settling (uprightness).
    pub mean_height: f32,
    /// Horizontal distance covered before first touching the ground (air drops).
    pub air_distance: f32,
}

pub struct CreatureSim {
    pub bodies: RigidBodySet,
    pub colliders: ColliderSet,
    pub impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    ccd: CCDSolver,
    query_pipeline: QueryPipeline,
    params: IntegrationParameters,
    gravity: Vector3<f32>,

    pub handles: Vec<RigidBodyHandle>,
    joint_handles: Vec<Option<ImpulseJointHandle>>,
    pub pheno: Phenotype,
    pub derived: Derived,
    pub env: Environment,
    pub cfg: SimConfig,
    pub time: f32,
    pub fatigue: f32,
    pub hydration: f32,
    pub water_level: Option<f32>,
    pub ground_y: f32,

    settled: bool,
    start_com: Vector3<f32>,
    pub time_aloft: f32,
    pub airborne_steps: u32,
    pub total_steps: u32,
    landed: bool,
    pub exploded: bool,
    pub landing_distance: Option<f32>,
    com_height_sum: f32,
}

fn part_bounding_radius(p: &crate::decode::Part) -> f32 {
    match p.kind {
        NodeKind::Membrane => (p.len * 0.5).max(p.r0),
        _ => p.len * 0.5 + p.radius,
    }
}

/// Oxygen supply over demand, clamped. Supply grows with lung fraction,
/// torso volume and partial pressure of O2; demand with total muscle section
/// and oscillation frequency.
pub fn fatigue_factor(pheno: &Phenotype, derived: &Derived) -> f32 {
    let supply = pheno.lung_frac * pheno.torso_volume * derived.o2_rel;
    let demand = (pheno.total_section_area * pheno.base_freq).max(1e-6);
    (O2_BUDGET_K * supply / demand).clamp(0.1, 1.0)
}

/// Hydration multiplier on land fitness: creatures on arid worlds need
/// stored water, which costs mass in the simulation.
pub fn hydration_factor(pheno: &Phenotype, derived: &Derived, env: Environment) -> f32 {
    if env == Environment::Water {
        return 1.0;
    }
    let needed = (derived.water_storage_frac - 0.03).max(0.0);
    if needed <= 1e-4 {
        1.0
    } else {
        0.4 + 0.6 * (pheno.water_store / needed).min(1.0)
    }
}

impl CreatureSim {
    pub fn new(pheno: Phenotype, planet: &PlanetParams, env: Environment, cfg: SimConfig) -> Self {
        let derived = planet.derive();
        let mut bodies = RigidBodySet::new();
        let mut colliders = ColliderSet::new();
        let mut impulse_joints = ImpulseJointSet::new();

        let (ground_y, water_level, spawn_y) = match env {
            Environment::Land => (0.0, None, 0.0),
            Environment::Water => (-WATER_DEPTH, Some(0.0), -WATER_DEPTH * 0.5),
            Environment::Air => (0.0, None, AIR_DROP_HEIGHT),
        };

        // Ground.
        let ground = RigidBodyBuilder::fixed().translation(vector![0.0, ground_y - 1.0, 0.0]).build();
        let gh = bodies.insert(ground);
        colliders.insert_with_parent(
            ColliderBuilder::cuboid(500.0, 1.0, 500.0).friction(0.9).build(),
            gh,
            &mut bodies,
        );

        // Place parts: root lies horizontally along +X.
        let n = pheno.parts.len();
        let mut isos: Vec<Isometry3<f32>> = Vec::with_capacity(n);
        let root_rot = UnitQuaternion::rotation_between(&Vector3::y(), &Vector3::x())
            .unwrap_or_else(UnitQuaternion::identity);
        isos.push(Isometry3::from_parts(Vector3::zeros().into(), root_rot));
        for i in 1..n {
            let p = &pheno.parts[i];
            let parent = p.parent.unwrap();
            let rot = isos[parent].rotation * p.rot_from_parent;
            let base_local = Vector3::new(0.0, -p.len * 0.5, 0.0);
            let pos = isos[parent].translation.vector + isos[parent].rotation * p.anchor_parent - rot * base_local;
            isos.push(Isometry3::from_parts(pos.into(), rot));
        }
        // Lift so the lowest point sits just above the ground (or at spawn depth).
        let min_y = (0..n)
            .map(|i| isos[i].translation.y - part_bounding_radius(&pheno.parts[i]))
            .fold(f32::INFINITY, f32::min);
        let lift = match env {
            Environment::Land => ground_y + 0.15 - min_y,
            Environment::Water => spawn_y - min_y,
            Environment::Air => spawn_y - min_y,
        };
        for iso in isos.iter_mut() {
            iso.translation.y += lift;
        }

        let fatigue = fatigue_factor(&pheno, &derived);
        let hydration = hydration_factor(&pheno, &derived, env);

        let mut handles = Vec::with_capacity(n);
        for (i, p) in pheno.parts.iter().enumerate() {
            let rb = RigidBodyBuilder::dynamic()
                .position(isos[i])
                .linear_damping(0.05)
                .angular_damping(0.2)
                .ccd_enabled(true)
                .build();
            let h = bodies.insert(rb);
            let friction = match p.kind {
                NodeKind::Pad => 1.4,
                NodeKind::Membrane => 0.5,
                NodeKind::Capsule => 0.8,
            };
            let col = match p.kind {
                NodeKind::Membrane => ColliderBuilder::cuboid(p.r0, p.len * 0.5, MEMBRANE_THICKNESS * 0.5),
                _ => ColliderBuilder::capsule_y(p.len * 0.5, p.radius),
            }
            .density(p.density)
            .friction(friction)
            .collision_groups(InteractionGroups::new(Group::GROUP_1, Group::ALL))
            .build();
            colliders.insert_with_parent(col, h, &mut bodies);
            handles.push(h);
        }

        // Joints.
        let mut joint_handles = vec![None; n];
        for i in 1..n {
            let p = &pheno.parts[i];
            let parent = p.parent.unwrap();
            let frame1 = Isometry3::from_parts(p.anchor_parent.into(), p.rot_from_parent);
            let frame2 = Isometry3::from_parts(Vector3::new(0.0, -p.len * 0.5, 0.0).into(), UnitQuaternion::identity());
            let strength = p.max_torque * fatigue;
            let stiffness = strength * 3.0;
            let damping = strength * 0.3;
            let mask = match p.joint {
                JointKind::Hinge => JointAxesMask::LOCKED_REVOLUTE_AXES,
                JointKind::Ball => JointAxesMask::LIN_X | JointAxesMask::LIN_Y | JointAxesMask::LIN_Z | JointAxesMask::ANG_Y,
                JointKind::Rigid => JointAxesMask::LOCKED_FIXED_AXES,
            };
            let mut b = GenericJointBuilder::new(mask)
                .local_frame1(frame1)
                .local_frame2(frame2)
                .contacts_enabled(false);
            match p.joint {
                JointKind::Hinge => {
                    b = b
                        .limits(JointAxis::AngX, [-p.limit, p.limit])
                        .motor_model(JointAxis::AngX, MotorModel::ForceBased)
                        .motor_position(JointAxis::AngX, p.cpg.bias.clamp(-p.limit, p.limit), stiffness, damping)
                        .motor_max_force(JointAxis::AngX, strength);
                }
                JointKind::Ball => {
                    for axis in [JointAxis::AngX, JointAxis::AngZ] {
                        b = b
                            .limits(axis, [-p.limit, p.limit])
                            .motor_model(axis, MotorModel::ForceBased)
                            .motor_position(axis, 0.0, stiffness, damping)
                            .motor_max_force(axis, strength);
                    }
                }
                JointKind::Rigid => {}
            }
            let jh = impulse_joints.insert(handles[parent], handles[i], b.build(), true);
            joint_handles[i] = Some(jh);
        }

        let mut params = IntegrationParameters::default();
        params.dt = cfg.dt;
        params.num_solver_iterations = std::num::NonZeroUsize::new(8).unwrap();

        let start_com = Vector3::zeros();
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
            handles,
            joint_handles,
            pheno,
            derived,
            env,
            cfg,
            time: 0.0,
            fatigue,
            hydration,
            water_level,
            ground_y,
            settled: env == Environment::Air,
            start_com,
            time_aloft: 0.0,
            airborne_steps: 0,
            total_steps: 0,
            landed: false,
            exploded: false,
            landing_distance: None,
            com_height_sum: 0.0,
        };
        sim.start_com = sim.com();
        if !sim.settled {
            // Heavy damping while settling, like Sims' pre-run without effectors.
            for h in sim.handles.clone() {
                if let Some(rb) = sim.bodies.get_mut(h) {
                    rb.set_linear_damping(3.0);
                    rb.set_angular_damping(3.0);
                }
            }
        }
        sim
    }

    pub fn com(&self) -> Vector3<f32> {
        let mut sum = Vector3::zeros();
        let mut m = 0.0;
        for (i, h) in self.handles.iter().enumerate() {
            if let Some(rb) = self.bodies.get(*h) {
                let mass = self.pheno.parts[i].mass;
                sum += rb.translation() * mass;
                m += mass;
            }
        }
        if m > 0.0 {
            sum / m
        } else {
            sum
        }
    }

    pub fn part_isometry(&self, i: usize) -> Isometry3<f32> {
        *self.bodies[self.handles[i]].position()
    }

    pub fn horizontal_distance(&self) -> f32 {
        let c = self.com();
        let d = c - self.start_com;
        (d.x * d.x + d.z * d.z).sqrt()
    }

    fn motors_active(&self) -> bool {
        self.settled
    }

    fn lowest_point(&self) -> f32 {
        let mut min_y = f32::INFINITY;
        for (i, h) in self.handles.iter().enumerate() {
            let rb = &self.bodies[*h];
            let p = &self.pheno.parts[i];
            let iso = rb.position();
            let half = Vector3::new(0.0, p.len * 0.5, 0.0);
            let a = iso.translation.vector + iso.rotation * half;
            let b = iso.translation.vector - iso.rotation * half;
            let r = match p.kind {
                NodeKind::Membrane => p.r0,
                _ => p.radius,
            };
            min_y = min_y.min(a.y - r).min(b.y - r);
        }
        min_y
    }

    pub fn is_airborne(&self) -> bool {
        self.lowest_point() > self.ground_y + 0.08
    }

    fn update_motors(&mut self) {
        let t = self.time;
        let f = self.pheno.base_freq;
        for i in 1..self.pheno.parts.len() {
            let Some(jh) = self.joint_handles[i] else { continue };
            let p = &self.pheno.parts[i];
            if p.joint == JointKind::Rigid {
                continue;
            }
            let strength = p.max_torque * self.fatigue;
            let stiffness = strength * 3.0;
            let damping = strength * 0.3;
            let w = 2.0 * PI * f * p.cpg.freq_mult * t;
            let (t1, t2) = if self.motors_active() {
                (
                    (p.cpg.bias + p.cpg.amp * (w + p.cpg.phase).sin()).clamp(-p.limit, p.limit),
                    (p.cpg.amp2 * (w + p.cpg.phase2).sin()).clamp(-p.limit, p.limit),
                )
            } else {
                (p.cpg.bias.clamp(-p.limit, p.limit), 0.0)
            };
            if let Some(j) = self.impulse_joints.get_mut(jh) {
                j.data.set_motor_position(JointAxis::AngX, t1, stiffness, damping);
                if p.joint == JointKind::Ball {
                    j.data.set_motor_position(JointAxis::AngZ, t2, stiffness, damping);
                }
            }
        }
    }

    fn apply_fluid_forces(&mut self) {
        let rho_air = self.derived.air_density;
        let rho_water = self.derived.water_density;
        let g = -self.gravity.y;
        let water_level = self.water_level;
        for i in 0..self.pheno.parts.len() {
            let p = &self.pheno.parts[i];
            let h = self.handles[i];
            let Some(rb) = self.bodies.get_mut(h) else { continue };
            rb.reset_forces(false);
            let iso = *rb.position();
            let center = iso.translation.vector;
            let axis = iso.rotation * Vector3::y();

            // Submersion fraction and local fluid density.
            let r_b = part_bounding_radius(p);
            let sub = match water_level {
                Some(wl) => ((wl - (center.y - r_b)) / (2.0 * r_b)).clamp(0.0, 1.0),
                None => 0.0,
            };
            let rho = rho_air * (1.0 - sub) + rho_water * sub;

            match p.kind {
                NodeKind::Membrane => {
                    // 2x2 sub-panels.
                    let normal = iso.rotation * Vector3::z();
                    let chord = iso.rotation * Vector3::x();
                    let area = p.len * 2.0 * p.r0 * 0.25;
                    for (sy, sx) in [(-0.25f32, -0.5f32), (-0.25, 0.5), (0.25, -0.5), (0.25, 0.5)] {
                        let point = center + axis * (sy * p.len) + chord * (sx * p.r0);
                        let v = rb.velocity_at_point(&Point3::from(point));
                        let speed = v.norm();
                        if speed < 1e-4 {
                            continue;
                        }
                        let vhat = v / speed;
                        let s = vhat.dot(&normal); // sin(alpha), signed
                        let f_normal = normal * (-0.5 * rho * area * speed * speed * C_P_ALPHA * s);
                        let f_para = v * (-0.5 * rho * area * CD0_PANEL * speed);
                        rb.add_force_at_point(f_normal + f_para, Point3::from(point), true);
                    }
                }
                _ => {
                    // Three samples along the axis with anisotropic drag.
                    let a_axial = PI * p.radius * p.radius;
                    let a_lat = 2.0 * p.radius * (p.len + 2.0 * p.radius);
                    for sy in [-0.4f32, 0.0, 0.4] {
                        let point = center + axis * (sy * p.len);
                        let v = rb.velocity_at_point(&Point3::from(point));
                        let speed = v.norm();
                        if speed < 1e-4 {
                            continue;
                        }
                        let va = axis * v.dot(&axis);
                        let vl = v - va;
                        let f = (va * (CD_CAPSULE_AXIAL * a_axial) + vl * (CD_CAPSULE_LATERAL * a_lat))
                            * (-0.5 * rho * speed / 3.0);
                        rb.add_force_at_point(f, Point3::from(point), true);
                    }
                }
            }

            // Buoyancy and submersion damping.
            if sub > 0.0 {
                let volume = p.mass / p.density;
                let fb = Vector3::new(0.0, rho_water * volume * g * sub, 0.0);
                let v = *rb.linvel();
                let damp = -v * (0.8 * p.mass * sub);
                rb.add_force(fb + damp, true);
            }
        }
    }

    pub fn step(&mut self) {
        if self.exploded {
            return;
        }
        if !self.settled && self.time >= self.cfg.settle_time {
            self.settled = true;
            for h in self.handles.clone() {
                if let Some(rb) = self.bodies.get_mut(h) {
                    rb.set_linear_damping(0.05);
                    rb.set_angular_damping(0.2);
                }
            }
            self.start_com = self.com();
        }
        self.update_motors();
        self.apply_fluid_forces();
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
        self.time += self.cfg.dt;

        if self.settled {
            self.total_steps += 1;
            self.com_height_sum += self.com().y - self.ground_y;
            let airborne = self.is_airborne();
            if airborne {
                self.airborne_steps += 1;
                if !self.landed {
                    self.time_aloft += self.cfg.dt;
                }
            } else if self.env == Environment::Air && !self.landed {
                self.landed = true;
                self.landing_distance = Some(self.horizontal_distance());
            }
        }

        // Explosion / NaN detection: evolution will exploit any leak.
        for h in &self.handles {
            let rb = &self.bodies[*h];
            let v = rb.linvel();
            let t = rb.translation();
            if !v.iter().all(|c| c.is_finite()) || !t.iter().all(|c| c.is_finite()) || v.norm() > 60.0 {
                self.exploded = true;
                break;
            }
        }
    }

    /// Seconds of settling before motors engage (none for the Air drop).
    pub fn settle_duration(&self) -> f32 {
        if self.env == Environment::Air {
            0.0
        } else {
            self.cfg.settle_time
        }
    }

    pub fn finished(&self) -> bool {
        self.exploded || self.time >= self.settle_duration() + self.cfg.episode_time
    }

    pub fn result(&self) -> EpisodeResult {
        let distance = self.horizontal_distance();
        let airborne_frac = if self.total_steps > 0 {
            self.airborne_steps as f32 / self.total_steps as f32
        } else {
            0.0
        };
        let fitness = if self.exploded {
            0.0
        } else {
            match self.env {
                Environment::Land => distance * self.hydration,
                Environment::Water => distance,
                Environment::Air => {
                    // Horizontal distance dominates so gliders beat parachutes;
                    // time aloft still rewards slowing the fall as a first step.
                    let d = self.landing_distance.unwrap_or(distance);
                    (d + 0.25 * self.time_aloft) * self.hydration
                }
            }
        };
        EpisodeResult {
            fitness: fitness.max(0.0),
            distance,
            airborne_frac,
            time_aloft: self.time_aloft,
            exploded: self.exploded,
            part_count: self.pheno.part_count(),
            area_per_mass: self.pheno.area_per_mass(),
            fatigue: self.fatigue,
            hydration: self.hydration,
            total_mass: self.pheno.total_mass,
            mean_height: if self.total_steps > 0 { self.com_height_sum / self.total_steps as f32 } else { 0.0 },
            air_distance: self.landing_distance.unwrap_or(distance),
        }
    }

    /// Run a full episode headless.
    pub fn run_episode(pheno: Phenotype, planet: &PlanetParams, env: Environment, cfg: SimConfig) -> EpisodeResult {
        let mut sim = CreatureSim::new(pheno, planet, env, cfg);
        while !sim.finished() {
            sim.step();
        }
        sim.result()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::decode;
    use crate::genome::Genome;

    #[test]
    fn episodes_run_and_are_deterministic() {
        let planet = PlanetParams::earth();
        let cfg = SimConfig { episode_time: 2.0, ..Default::default() };
        for seed in 0..6u64 {
            let g = Genome::random(seed);
            let a = CreatureSim::run_episode(decode(&g), &planet, Environment::Land, cfg);
            let b = CreatureSim::run_episode(decode(&g), &planet, Environment::Land, cfg);
            assert!(a.fitness.is_finite());
            assert_eq!(a.fitness.to_bits(), b.fitness.to_bits(), "non-deterministic for seed {seed}");
        }
    }

    #[test]
    fn membrane_in_dense_air_falls_slower() {
        // A single large light membrane dropped in Earth air vs near-vacuum.
        let mut g = Genome::random(1);
        g.nodes.truncate(1);
        g.nodes[0].connections.clear();
        g.nodes[0].kind = NodeKind::Membrane;
        g.nodes[0].len = 1.5;
        g.nodes[0].r0 = 0.75;
        g.nodes[0].recursive_limit = 1;
        g.lung_frac = 0.0;
        g.water_store = 0.0;
        let cfg = SimConfig { settle_time: 0.0, episode_time: 1.5, ..Default::default() };
        let earth = PlanetParams::earth();
        let mut thin = PlanetParams::earth();
        thin.pressure_atm = 0.001;
        let fall = |planet: &PlanetParams| {
            let mut sim = CreatureSim::new(decode(&g), planet, Environment::Air, cfg);
            // Face the panel upward: root lies along X, so normal (local Z) is world Z.
            // Rotate so that normal points up.
            let h = sim.handles[0];
            let rb = sim.bodies.get_mut(h).unwrap();
            let rot = UnitQuaternion::rotation_between(&Vector3::z(), &Vector3::y()).unwrap();
            let pos = *rb.position();
            rb.set_position(Isometry3::from_parts(pos.translation, rot * pos.rotation), true);
            while !sim.finished() {
                sim.step();
            }
            sim.com().y
        };
        let y_earth = fall(&earth);
        let y_thin = fall(&thin);
        assert!(y_earth > y_thin + 0.5, "earth {y_earth} thin {y_thin}");
    }
}
