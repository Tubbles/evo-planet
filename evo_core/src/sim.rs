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

use crate::decode::{Part, Phenotype, MEMBRANE_THICKNESS};
use crate::genome::NodeKind;
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
    /// A beach: water over a bed that rises at SHORE_SLOPE toward +X and
    /// breaks the surface at x = 0. The body starts submerged 3 m out and
    /// must climb out by itself; the result is how far up the dry beach it got.
    Shore,
}

impl Environment {
    pub const ALL: [Environment; 3] = [Environment::Land, Environment::Water, Environment::Air];
    pub fn name(self) -> &'static str {
        match self {
            Environment::Land => "Land",
            Environment::Water => "Water",
            Environment::Air => "Air (drop)",
            Environment::Shore => "Shore",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SimConfig {
    pub dt: f32,
    pub settle_time: f32,
    pub episode_time: f32,
    /// Drive the joints. False is the sham control: same body, no muscle.
    pub motors: bool,
    /// Asymmetric drive, -1..1: mirrored parts get amplitude x(1 - steer),
    /// the others x(1 + steer). How animals turn.
    pub steer: f32,
    /// Added to every joint's bias, radians: the body's posture shift used
    /// to climb or dive.
    pub bias_shift: f32,
    /// Scales every oscillator's amplitude. 1 is the body as evolved; 0 is
    /// the "hold" control: muscles hold the rest posture (bias) but do not
    /// swing, so what oscillation adds (flapping lift, a stroke) can be
    /// separated from what posture alone does (a held wing glides).
    pub amp_scale: f32,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self { dt: 1.0 / 120.0, settle_time: 1.0, episode_time: 8.0, motors: true, steer: 0.0, bias_shift: 0.0, amp_scale: 1.0 }
    }
}

/// Flat-plate normal-force slope (Lu et al. 2026 form: Cl = C_Pa sin a cos a).
pub const C_P_ALPHA: f32 = 2.61;
/// Parasitic drag of a thin panel at zero incidence.
pub const CD0_PANEL: f32 = 0.09;
/// Incidence at which a thin plate's flow separates (about 15 degrees) and
/// the width of the blend from attached lift to the stalled normal force.
pub const STALL_ANGLE: f32 = 0.26;
pub const STALL_WIDTH: f32 = 0.12;
/// Rolling resistance coefficient of a body on the ground (soft tissue on
/// soil: 0.03-0.1). Without it a toppled capsule rolls forever.
pub const C_ROLLING: f32 = 0.05;
/// Spin drag coefficient of a part rotating about its own axis.
pub const C_SPIN: f32 = 1.0;
/// Passive joint spring per unit stiffness gene, in multiples of muscle strength.
pub const RIGID_GAIN: f32 = 20.0;
/// Joint angular speed at which muscle torque reaches zero (Hill), rad/s.
/// Vertebrate limb joints peak around 10-40 rad/s.
pub const OMEGA_MAX: f32 = 25.0;
pub const CD_CAPSULE_AXIAL: f32 = 0.5;
pub const CD_CAPSULE_LATERAL: f32 = 1.1;
/// Oxygen budget scale: see `fatigue_factor`.
pub const O2_BUDGET_K: f32 = 40.0;
pub const AIR_DROP_HEIGHT: f32 = 10.0;
/// Beach gradient (rise per metre toward +X) for the shore test.
pub const SHORE_SLOPE: f32 = 0.12;
/// Farthest the shore test starts, metres seaward of the waterline.
pub const SHORE_START_X: f32 = -4.0;
/// Gliding animals leap: the air drop starts with this forward speed (m/s).
/// A stone thrown this way lands `AIR_LAUNCH_SPEED * sqrt(2h/g)` away; only
/// distance beyond that is credited to wings.
pub const AIR_LAUNCH_SPEED: f32 = 3.0;

/// Horizontal distance a wingless body covers in the air drop (the leap alone).
pub fn ballistic_distance(gravity: f32) -> f32 {
    AIR_LAUNCH_SPEED * (2.0 * AIR_DROP_HEIGHT / gravity.max(0.1)).sqrt()
}
pub const WATER_DEPTH: f32 = 12.0;

fn default_travel() -> [f32; 3] {
    [0.0, 1.0, 0.0]
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EpisodeResult {
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
    /// Sustained locomotion speed, m/s: horizontal progress over the second
    /// half of the episode, after any toppling or sinking has played out. A
    /// body that falls over once and rolls scores zero here.
    pub sustained_speed: f32,
    /// Mean yaw rate of the trunk over the second half, rad/s (signed).
    pub yaw_rate: f32,
    /// Which way the body travels, as a unit vector in the trunk's own frame
    /// (its axis is +Y): forward, backward, sideways, whatever the gait does.
    /// A controller that wants to go somewhere steers this, not the axis.
    #[serde(default = "default_travel")]
    pub travel_local: [f32; 3],
    /// Vertical speed of the centre of mass over the second half, m/s (signed).
    pub climb_speed: f32,
    /// Shore test: metres up the dry beach the body got under its own power.
    pub shore_progress: f32,
    /// Reach as swept: the farthest any part's end got from the centre of
    /// mass during the episode, metres. Measured, not read off the rest pose.
    pub swept_reach: f32,
    /// Share of settled steps in which the trunk itself touched the ground: 0
    /// for a body carried on its limbs, 1 for one dragging its belly.
    pub trunk_contact_frac: f32,
    /// Footfalls: ground-contact onsets of parts other than the trunk during
    /// the settled time. A gait is made of these; a tumble or a slide is not.
    pub footfalls: u32,
    /// How many different parts made footfalls: one limb hopping, or several
    /// limbs stepping in turn.
    pub stepping_limbs: u32,
    /// Mean clearance of the trunk's lowest point above the ground, metres:
    /// how tall the body stands on whatever carries it.
    pub trunk_clearance: f32,
    /// Mean angular speed of the trunk, rad/s: low for a carried trunk, high
    /// for a body that rolls or tumbles its way forward.
    pub trunk_spin: f32,
    /// Mechanical work the joints did during the settled time, joules (the
    /// absolute value of torque times angular displacement, summed).
    pub work: f32,
    /// Cost of transport: work per kilogram per metre of progress, J/(kg m).
    /// Walking mammals 1 to 5; a thrashing body far more.
    pub cost_of_transport: f32,
    /// Energy audit: mechanical energy (kinetic plus potential) gained over
    /// the settled time minus the work the motors did, joules. Friction and
    /// drag only remove energy, so anything above zero is a leak.
    pub energy_leak: f32,
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
    /// Bed gradient toward +X (shore test only).
    pub slope: f32,
    /// Farthest the centre of mass got up the dry beach, metres past the waterline.
    shore_best: f32,

    settled: bool,
    start_com: Vector3<f32>,
    pub time_aloft: f32,
    pub airborne_steps: u32,
    pub total_steps: u32,
    landed: bool,
    pub exploded: bool,
    pub landing_distance: Option<f32>,
    com_height_sum: f32,
    /// Centre of mass at the episode's midpoint, for sustained speed.
    mid_com: Option<Vector3<f32>>,
    yaw_sum: f32,
    yaw_n: u32,
    /// Travel over the window in the trunk's frame, summed per step.
    travel_sum: Vector3<f32>,
    prev_com: Option<Vector3<f32>>,
    swept_reach: f32,
    trunk_contact_steps: u32,
    part_down: Vec<bool>,
    part_falls: Vec<u32>,
    footfalls: u32,
    clearance_sum: f32,
    trunk_spin_sum: f32,
    work: f32,
    energy_start: Option<f32>,
}

fn part_bounding_radius(p: &crate::decode::Part) -> f32 {
    (p.len * 0.5 + p.radius * (1.0 - p.flat)).max(p.r0 * p.flat)
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

/// Ground: flat, or for the shore a plane tilted so its top face passes
/// through the origin and rises with +X.
pub fn add_ground(bodies: &mut RigidBodySet, colliders: &mut ColliderSet, ground_y: f32, slope: f32) {
    let theta = slope.atan();
    let ground = RigidBodyBuilder::fixed()
        .translation(vector![theta.sin(), ground_y - theta.cos(), 0.0])
        .rotation(vector![0.0, 0.0, theta])
        .build();
    let gh = bodies.insert(ground);
    colliders.insert_with_parent(ColliderBuilder::cuboid(500.0, 1.0, 500.0).friction(0.9).build(), gh, bodies);
}

/// Everything the fluid and ground forces need to know about the world.
#[derive(Clone, Copy, Debug)]
pub struct WorldCtx {
    pub rho_air: f32,
    pub rho_water: f32,
    pub g: f32,
    pub water_level: Option<f32>,
    pub ground_y: f32,
    pub slope: f32,
    pub dt: f32,
}

/// Motor drive for one step.
#[derive(Clone, Copy, Debug)]
pub struct Drive {
    pub active: bool,
    pub steer: f32,
    pub bias_shift: f32,
    pub amp_scale: f32,
}

/// A creature's bodies, colliders and joints inserted into a physics world.
pub struct Built {
    pub handles: Vec<RigidBodyHandle>,
    pub joint_handles: Vec<Option<ImpulseJointHandle>>,
    pub fatigue: f32,
}

/// Insert one body into a world: parts placed from the root (which lies
/// along +X, turned by `yaw` about the vertical), set down where it would
/// rest for the environment (on the ground, floating, on the bed, mid-water,
/// or at drop height), shifted to (`origin_x`, `origin_z`), with its joints
/// and motors. Shared by the single-body episode and the two-body arena.
#[allow(clippy::too_many_arguments)]
pub fn build_creature(
    bodies: &mut RigidBodySet,
    colliders: &mut ColliderSet,
    impulse_joints: &mut ImpulseJointSet,
    pheno: &Phenotype,
    env: Environment,
    derived: &Derived,
    ground_y: f32,
    slope: f32,
    spawn_y: f32,
    origin_x: f32,
    origin_z: f32,
    yaw: f32,
    launch: Vector3<f32>,
) -> Built {
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
        // Place the lowest point on the ground (1 cm solver clearance, no drop:
        // a fall would hand the body kinetic energy and a topple for free), or
        // at spawn depth / drop height.
        let min_y = (0..n)
            .map(|i| isos[i].translation.y - part_bounding_radius(&pheno.parts[i]))
            .fold(f32::INFINITY, f32::min);
        // In water, start where the body would rest: a buoyant body at the
        // surface, a dense one on the bottom, a neutral one mid-depth. A body
        // released mid-water would otherwise rise or sink for the whole
        // episode and a tilted fin would turn that into free travel.
        let max_y = (0..n)
            .map(|i| isos[i].translation.y + part_bounding_radius(&pheno.parts[i]))
            .fold(f32::NEG_INFINITY, f32::max);
        let volume: f32 = pheno.parts.iter().map(|p| p.mass / p.density.max(1.0)).sum();
        let mean_density = pheno.total_mass / volume.max(1e-6);
        // Shore: start where the water is 1.2 body heights deep, 1-4 m out.
        let x_off = if env == Environment::Shore { -((max_y - min_y) * 1.2 / SHORE_SLOPE).clamp(1.0, -SHORE_START_X) } else { 0.0 };
        let bed_here = ground_y + slope * x_off;
        let lift = match env {
            Environment::Land => ground_y + 0.01 - min_y,
            Environment::Shore => {
                if mean_density < 0.98 * derived.water_density {
                    (0.0 - 0.5 * (min_y + max_y)).max(bed_here + 0.01 - min_y)
                } else {
                    bed_here + 0.01 - min_y
                }
            }
            Environment::Water => {
                if mean_density < 0.98 * derived.water_density {
                    // Float: centre of the body at the waterline.
                    0.0 - 0.5 * (min_y + max_y)
                } else if mean_density > 1.02 * derived.water_density {
                    ground_y + 0.01 - min_y
                } else {
                    spawn_y - min_y
                }
            }
            Environment::Air => spawn_y - min_y,
        };
        // Turn the whole body about the vertical to face `yaw`, then set it down.
        let turn = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), yaw);
        for iso in isos.iter_mut() {
            iso.translation.vector = turn * iso.translation.vector;
            iso.rotation = turn * iso.rotation;
            iso.translation.y += lift;
            iso.translation.x += x_off + origin_x;
            iso.translation.z += origin_z;
        }

        let fatigue = fatigue_factor(pheno, derived);

        let mut handles: Vec<RigidBodyHandle> = Vec::with_capacity(n);
        for (i, p) in pheno.parts.iter().enumerate() {
            let rb = RigidBodyBuilder::dynamic()
                .position(isos[i])
                .linvel(launch)
                .linear_damping(0.05)
                .angular_damping(0.2)
                .ccd_enabled(true)
                .build();
            let h = bodies.insert(rb);
            // Friction from the surface gene, a little more for round skin than a
            // sheet. The collider is a rounded box whose inner box vanishes at
            // flat = 0 (then it is exactly a capsule) and whose rounding vanishes at
            // flat = 1 (then it is a thin plate of chord 2*r0).
            let friction = 0.5 + 0.3 * (1.0 - p.flat) + 0.6 * p.grip;
            // Contact shape is the nearest of the two (a rounded box with a zero
            // inner box has no mass in rapier); everything else about the part is
            // the continuous blend, and its mass is set from that blend.
            let col = if p.flat > 0.5 {
                ColliderBuilder::cuboid(p.r0, p.len * 0.5, (p.thickness * 0.5).max(MEMBRANE_THICKNESS * 0.5))
            } else {
                ColliderBuilder::capsule_y(p.len * 0.5, p.radius.max(0.002))
            }
            .mass(p.mass)
            .friction(friction)
            .collision_groups(InteractionGroups::new(Group::GROUP_1, Group::ALL))
            .build();
            colliders.insert_with_parent(col, h, bodies);
            handles.push(h);
        }

        // Joints.
        let mut joint_handles = vec![None; n];
        for i in 1..n {
            let p = &pheno.parts[i];
            let parent = p.parent.unwrap();
            let frame1 = Isometry3::from_parts(p.anchor_parent.into(), p.rot_from_parent);
            let frame2 = Isometry3::from_parts(Vector3::new(0.0, -p.len * 0.5, 0.0).into(), UnitQuaternion::identity());
            // One joint, three driven axes: the hinge (X), the sideways swing (Z)
            // and the twist about the part's own length (Y: a shoulder's
            // pronation, which is how a wing is pitched). `stiffness` adds a
            // passive spring holding the rest pose (welded at 1); `freedom` opens
            // the swing and the twist (a hinge at 0, a ball at 1). All continuous.
            let strength = p.max_torque * fatigue;
            let rigid = strength * RIGID_GAIN * p.stiffness;
            let stiffness = strength * 3.0 + rigid;
            let damping = strength * 0.3 + rigid * 0.1;
            let mask = JointAxesMask::LIN_X | JointAxesMask::LIN_Y | JointAxesMask::LIN_Z;
            let mut b = GenericJointBuilder::new(mask)
                .local_frame1(frame1)
                .local_frame2(frame2)
                .contacts_enabled(false);
            let lim_x = p.limit * (1.0 - p.stiffness) + 0.02;
            let lim_z = (p.limit * p.freedom * (1.0 - p.stiffness)).max(0.02);
            b = b
                .limits(JointAxis::AngX, [-lim_x, lim_x])
                .motor_model(JointAxis::AngX, MotorModel::ForceBased)
                .motor_position(JointAxis::AngX, p.cpg.bias.clamp(-lim_x, lim_x), stiffness, damping)
                .motor_max_force(JointAxis::AngX, strength + rigid)
                .limits(JointAxis::AngZ, [-lim_z, lim_z])
                .motor_model(JointAxis::AngZ, MotorModel::ForceBased)
                .motor_position(JointAxis::AngZ, 0.0, stiffness, damping)
                .motor_max_force(JointAxis::AngZ, strength + rigid)
                .limits(JointAxis::AngY, [-lim_z, lim_z])
                .motor_model(JointAxis::AngY, MotorModel::ForceBased)
                .motor_position(JointAxis::AngY, 0.0, stiffness, damping)
                .motor_max_force(JointAxis::AngY, strength + rigid);
            let jh = impulse_joints.insert(handles[parent], handles[i], b.build(), true);
            joint_handles[i] = Some(jh);
        }

        Built { handles, joint_handles, fatigue }
}

/// Set every joint's motor target for this step from the oscillators.
#[allow(clippy::too_many_arguments)]
pub fn update_motors(
    bodies: &RigidBodySet,
    joints: &mut ImpulseJointSet,
    parts: &[Part],
    handles: &[RigidBodyHandle],
    joint_handles: &[Option<ImpulseJointHandle>],
    base_freq: f32,
    fatigue: f32,
    time: f32,
    dr: Drive,
) -> f32 {
    let mut power = 0.0f32;
    let t = time;
    let f = base_freq;
    for i in 1..parts.len() {
        let Some(jh) = joint_handles[i] else { continue };
        let p = &parts[i];
        if p.stiffness >= 0.999 {
            continue; // welded: the passive spring set at build holds it
        }
        // Hill force-velocity limit: a muscle's torque falls linearly with
        // the joint's angular speed and is gone at OMEGA_MAX, so power is
        // bounded. Without it a joint delivers full torque at any speed and
        // a body can be spun at ten revolutions a second by one motor.
        let omega_rel = match (bodies.get(handles[i]), p.parent.and_then(|q| bodies.get(handles[q]))) {
            (Some(c), Some(q)) => (c.angvel() - q.angvel()).norm(),
            _ => 0.0,
        };
        let hill = (1.0 - omega_rel / OMEGA_MAX).clamp(0.05, 1.0);
        let rigid = p.max_torque * fatigue * RIGID_GAIN * p.stiffness;
        let strength = p.max_torque * fatigue * hill;
        let stiffness = strength * 3.0 + rigid;
        let damping = strength * 0.3 + rigid * 0.1;
        let drive = 1.0 - p.stiffness;
        let lim_x = p.limit * drive + 0.02;
        let lim_z = (p.limit * p.freedom * drive).max(0.02);
        let w = 2.0 * PI * f * p.cpg.freq_mult * t;
        let gain = 1.0 + if p.reflected { -dr.steer } else { dr.steer };
        let (t1, t2, t3) = if dr.active {
            let a1 = p.cpg.amp * dr.amp_scale;
            let a2 = p.cpg.amp2 * dr.amp_scale;
            let a3 = p.cpg.amp3 * dr.amp_scale;
            (
                (p.cpg.bias + (dr.bias_shift + a1 * gain * (w + p.cpg.phase).sin()) * drive).clamp(-lim_x, lim_x),
                (a2 * gain * p.freedom * drive * (w + p.cpg.phase2).sin()).clamp(-lim_z, lim_z),
                (a3 * gain * p.freedom * drive * (w + p.cpg.phase3).sin()).clamp(-lim_z, lim_z),
            )
        } else {
            (p.cpg.bias.clamp(-lim_x, lim_x), 0.0, 0.0)
        };
        if let Some(j) = joints.get_mut(jh) {
            j.data.set_motor_position(JointAxis::AngX, t1, stiffness, damping);
            j.data.set_motor_max_force(JointAxis::AngX, strength + rigid);
            j.data.set_motor_position(JointAxis::AngZ, t2, stiffness, damping);
            j.data.set_motor_max_force(JointAxis::AngZ, strength + rigid);
            j.data.set_motor_position(JointAxis::AngY, t3, stiffness, damping);
            j.data.set_motor_max_force(JointAxis::AngY, strength + rigid);
            // The torque this motor model applies, and the power it spends:
            // the joint's angles and rates in its own frame against the targets.
            if let (Some(b1), Some(b2)) = (bodies.get(j.body1), bodies.get(j.body2)) {
                let r1 = b1.position().rotation * j.data.local_frame1.rotation;
                let r2 = b2.position().rotation * j.data.local_frame2.rotation;
                let rel = r1.inverse() * r2;
                let ang = rel.scaled_axis();
                let w = r1.inverse() * (b2.angvel() - b1.angvel());
                let maxf = strength + rigid;
                for (target, theta, omega) in [(t1, ang.x, w.x), (t2, ang.z, w.z), (t3, ang.y, w.y)] {
                    let tau = (stiffness * (target - theta) - damping * omega).clamp(-maxf, maxf);
                    power += (tau * omega).abs();
                }
            }
        }
    }
    power
}

/// Fluid, spin, rolling and buoyancy forces on every part of one body.
pub fn apply_fluid_forces(ctx: &WorldCtx, bodies: &mut RigidBodySet, parts: &[Part], handles: &[RigidBodyHandle]) {
    let (rho_air, rho_water, g, water_level) = (ctx.rho_air, ctx.rho_water, ctx.g, ctx.water_level);
    for i in 0..parts.len() {
        let p = &parts[i];
        let h = handles[i];
        let Some(rb) = bodies.get_mut(h) else { continue };
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

        let flat = p.flat;
        if flat > 0.0 {
            {
                // Sheet share: 2x2 sub-panels of a plate of chord 2*r0.
                let normal = iso.rotation * Vector3::z();
                let chord = iso.rotation * Vector3::x();
                let area = p.len * 2.0 * p.r0 * 0.25 * flat;
                let full_area = p.len * 2.0 * p.r0;
                for (sy, sx) in [(-0.25f32, -0.5f32), (-0.25, 0.5), (0.25, -0.5), (0.25, 0.5)] {
                    let point = center + axis * (sy * p.len) + chord * (sx * p.r0);
                    let v = rb.velocity_at_point(&Point3::from(point));
                    let speed = v.norm();
                    if speed < 1e-4 {
                        continue;
                    }
                    let vhat = v / speed;
                    let s = vhat.dot(&normal); // sin(alpha), signed
                    let q = 0.5 * rho * area * speed * speed;
                    // Two regimes, blended across the stall. Attached flow at
                    // small incidence: lift perpendicular to the relative wind
                    // (thin-aerofoil slope 2*pi, reduced for the panel's aspect
                    // ratio along the wind), plus induced drag along it. This is
                    // what makes a flapping wing push: the lift vector tilts with
                    // the stroke. Separated flow past the stall: the flat-plate
                    // normal force (Lu et al.), which is what a parachute, a paddle
                    // and a leaf in a gale feel.
                    let alpha = s.clamp(-1.0, 1.0).asin();
                    let wind_in_plane = vhat - normal * s;
                    let wip_n = wind_in_plane.norm();
                    let f_attached = if wip_n > 1e-4 {
                        let w = wind_in_plane / wip_n;
                        let chord_eff = (w.dot(&chord).abs() * 2.0 * p.r0 + w.dot(&axis).abs() * p.len).max(1e-3);
                        let ar = (full_area / chord_eff).powi(2) / full_area; // span² / area
                        let cl = 2.0 * PI * alpha.sin() * ar / (ar + 2.0);
                        let cdi = cl * cl / (PI * ar * 0.9);
                        // Lift is perpendicular to the wind, in the plane of wind
                        // and normal, on the same side as the plate force.
                        let perp = normal - vhat * s;
                        let lift_dir = perp / perp.norm().max(1e-6);
                        lift_dir * (-q * cl) - vhat * (q * cdi)
                    } else {
                        Vector3::zeros()
                    };
                    let f_stalled = normal * (-q * C_P_ALPHA * s);
                    let stall = ((alpha.abs() - STALL_ANGLE) / STALL_WIDTH).clamp(0.0, 1.0);
                    let f_lift = f_attached * (1.0 - stall) + f_stalled * stall;
                    let f_para = v * (-0.5 * rho * area * CD0_PANEL * speed);
                    // Explicit drag must not exceed what brings this sample's share
                    // of the part to rest in one step, or light parts in dense
                    // fluid blow up numerically.
                    let mut f = f_lift + f_para;
                    let f_max = 0.25 * p.mass * flat.max(0.05) * speed / ctx.dt;
                    let fm = f.norm();
                    if fm > f_max {
                        f *= f_max / fm;
                    }
                    rb.add_force_at_point(f, Point3::from(point), true);
                }
            }
        }
        if flat < 1.0 {
            {
                // Round share: three samples along the axis with anisotropic drag.
                let r_cap = 0.5 * (p.r0 + p.r1);
                let a_axial = PI * r_cap * r_cap * (1.0 - flat);
                let a_lat = 2.0 * r_cap * (p.len + 2.0 * r_cap) * (1.0 - flat);
                for sy in [-0.4f32, 0.0, 0.4] {
                    let point = center + axis * (sy * p.len);
                    let v = rb.velocity_at_point(&Point3::from(point));
                    let speed = v.norm();
                    if speed < 1e-4 {
                        continue;
                    }
                    let va = axis * v.dot(&axis);
                    let vl = v - va;
                    let mut f = (va * (CD_CAPSULE_AXIAL * a_axial) + vl * (CD_CAPSULE_LATERAL * a_lat))
                        * (-0.5 * rho * speed / 3.0);
                    let f_max = p.mass / 3.0 * (1.0 - flat).max(0.05) * speed / ctx.dt;
                    let fm = f.norm();
                    if fm > f_max {
                        f *= f_max / fm;
                    }
                    rb.add_force_at_point(f, Point3::from(point), true);
                }
            }
        }

        // Spin drag about the part's own axis: the point samples above lie on
        // the axis and see no velocity from axial rotation, so without this a
        // disc could spin up without limit. A rotating cylinder of radius r
        // and length L in a fluid: torque ~ 0.5 rho C r^4 L w|w| (plus the
        // caps), opposed to the spin and capped so it cannot reverse it in one
        // step (inertia about the axis ~ 0.5 m r^2).
        {
            let w = *rb.angvel();
            let wa = w.dot(&axis);
            if wa.abs() > 1e-3 {
                let r_eff = (1.0 - p.flat) * p.radius + p.flat * p.r0;
                let l_eff = p.len + 2.0 * p.radius * (1.0 - p.flat);
                let mut tau = 0.5 * rho * C_SPIN * r_eff.powi(4) * l_eff * wa * wa.abs();
                let inertia = 0.5 * p.mass * r_eff * r_eff;
                let tau_max = inertia * wa.abs() / ctx.dt;
                if tau.abs() > tau_max {
                    tau = tau.signum() * tau_max;
                }
                rb.add_torque(axis * (-tau), true);
            }
        }

        // Rolling resistance: a part resting on the ground loses horizontal
        // momentum at C_ROLLING * g, which stops passive rolling after a
        // topple while costing a walker little.
        if sub <= 0.0 {
            let half = axis * (p.len * 0.5);
            let low = (center + half).y.min((center - half).y) - p.radius;
            if low < ctx.ground_y + ctx.slope * center.x + 0.02 {
                let v = *rb.linvel();
                let vh = Vector3::new(v.x, 0.0, v.z);
                let sp = vh.norm();
                if sp > 1e-4 {
                    let f = (C_ROLLING * p.mass * g).min(p.mass * sp / ctx.dt);
                    rb.add_force(-vh / sp * f, true);
                }
            }
        }

        // Buoyancy and submersion damping.
        if sub > 0.0 {
            // A membrane's collider is 2 cm thick for the solver; the tissue
            // it stands for is a millimetre of skin, so it displaces its mass
            // in water, not its collider volume.
            let volume = p.mass / p.density;
            let fb = Vector3::new(0.0, rho_water * volume * g * sub, 0.0);
            let v = *rb.linvel();
            let damp = -v * (0.8 * p.mass * sub);
            rb.add_force(fb + damp, true);
        }
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
            Environment::Shore => (0.0, Some(0.0), 0.0),
        };
        let slope = if env == Environment::Shore { SHORE_SLOPE } else { 0.0 };
        add_ground(&mut bodies, &mut colliders, ground_y, slope);
        let launch = if env == Environment::Air { vector![AIR_LAUNCH_SPEED, 0.0, 0.0] } else { vector![0.0, 0.0, 0.0] };
        let built = build_creature(&mut bodies, &mut colliders, &mut impulse_joints, &pheno, env, &derived, ground_y, slope, spawn_y, 0.0, 0.0, 0.0, launch);
        let Built { handles, joint_handles, fatigue } = built;
        let hydration = hydration_factor(&pheno, &derived, env);

        let mut params = IntegrationParameters::default();
        params.dt = cfg.dt;
        params.num_solver_iterations = std::num::NonZeroUsize::new(8).unwrap();

        let start_com = Vector3::zeros();
        let n_parts = pheno.parts.len();
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
            slope,
            shore_best: 0.0,
            settled: env == Environment::Air,
            start_com,
            time_aloft: 0.0,
            airborne_steps: 0,
            total_steps: 0,
            landed: false,
            exploded: false,
            landing_distance: None,
            com_height_sum: 0.0,
            mid_com: None,
            yaw_sum: 0.0,
            yaw_n: 0,
            travel_sum: Vector3::zeros(),
            prev_com: None,
            swept_reach: 0.0,
            trunk_contact_steps: 0,
            part_down: vec![false; n_parts],
            part_falls: vec![0; n_parts],
            footfalls: 0,
            clearance_sum: 0.0,
            trunk_spin_sum: 0.0,
            work: 0.0,
            energy_start: None,
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

    /// Kinetic plus gravitational potential energy of the body, joules.
    pub fn mechanical_energy(&self) -> f32 {
        let g = -self.gravity.y;
        let mut e = 0.0;
        for (i, h) in self.handles.iter().enumerate() {
            if let Some(rb) = self.bodies.get(*h) {
                let m = self.pheno.parts[i].mass;
                let v = rb.linvel();
                let w = rb.angvel();
                let inertia = rb.mass_properties().local_mprops.principal_inertia();
                let w_local = rb.position().rotation.inverse() * w;
                let rot = 0.5 * (inertia.x * w_local.x * w_local.x + inertia.y * w_local.y * w_local.y + inertia.z * w_local.z * w_local.z);
                e += 0.5 * m * v.norm_squared() + rot + m * g * rb.translation().y;
            }
        }
        e
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

    /// Mass-weighted mean angular speed of the parts, rad/s (rolling shows here).
    pub fn angular_speed(&self) -> f32 {
        let mut sum = 0.0;
        let mut m = 0.0;
        for (i, h) in self.handles.iter().enumerate() {
            if let Some(rb) = self.bodies.get(*h) {
                let mass = self.pheno.parts[i].mass;
                sum += rb.angvel().norm() * mass;
                m += mass;
            }
        }
        if m > 0.0 { sum / m } else { 0.0 }
    }

    pub fn lowest_point(&self) -> f32 {
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

    /// Position and linear velocity of part `i` (debugging and audits).
    pub fn part_state(&self, i: usize) -> (Vector3<f32>, Vector3<f32>) {
        match self.bodies.get(self.handles[i]) {
            Some(rb) => (*rb.translation(), *rb.linvel()),
            None => (Vector3::zeros(), Vector3::zeros()),
        }
    }

    /// Angular velocity of part `i` (debugging and audits).
    pub fn part_angvel(&self, i: usize) -> Vector3<f32> {
        self.bodies.get(self.handles[i]).map(|rb| *rb.angvel()).unwrap_or_else(Vector3::zeros)
    }

    /// Ground height under horizontal position x (sloped on the shore).
    pub fn ground_at(&self, x: f32) -> f32 {
        self.ground_y + self.slope * x
    }

    pub fn is_airborne(&self) -> bool {
        self.lowest_point() > self.ground_at(self.com().x) + 0.08
    }

    fn update_motors(&mut self) {
        let drive = Drive { active: self.motors_active(), steer: self.cfg.steer, bias_shift: self.cfg.bias_shift, amp_scale: self.cfg.amp_scale };
        let power = update_motors(&self.bodies, &mut self.impulse_joints, &self.pheno.parts, &self.handles, &self.joint_handles, self.pheno.base_freq, self.fatigue, self.time, drive);
        if self.settled {
            self.work += power * self.cfg.dt;
        }
    }

    pub fn world_ctx(&self) -> WorldCtx {
        WorldCtx { rho_air: self.derived.air_density, rho_water: self.derived.water_density, g: -self.gravity.y, water_level: self.water_level, ground_y: self.ground_y, slope: self.slope, dt: self.cfg.dt }
    }

    fn apply_fluid_forces(&mut self) {
        let ctx = self.world_ctx();
        apply_fluid_forces(&ctx, &mut self.bodies, &self.pheno.parts, &self.handles);
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
        if self.cfg.motors {
            self.update_motors();
        }
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
            if self.energy_start.is_none() {
                self.energy_start = Some(self.mechanical_energy());
            }
            let c = self.com();
            self.com_height_sum += c.y - self.ground_at(c.x);
            // Which parts touch the ground (lowest point within 2 cm of it)?
            // The trunk touching is belly contact; a limb's touch after being
            // up is a footfall. The trunk's angular speed is its tumble.
            for i in 0..self.pheno.parts.len() {
                let Some(rb) = self.bodies.get(self.handles[i]) else { continue };
                let p = &self.pheno.parts[i];
                let iso = rb.position();
                let half = iso.rotation * Vector3::new(0.0, p.len * 0.5, 0.0);
                let r = if p.flat > 0.5 { (p.thickness * 0.5).max(MEMBRANE_THICKNESS * 0.5) } else { p.radius };
                let low = (iso.translation.vector + half).y.min((iso.translation.vector - half).y) - r;
                let down = low <= self.ground_at(iso.translation.x) + 0.02;
                if i == 0 {
                    if down {
                        self.trunk_contact_steps += 1;
                    }
                    self.clearance_sum += (low - self.ground_at(iso.translation.x)).max(0.0);
                    self.trunk_spin_sum += rb.angvel().norm();
                } else if down && !self.part_down[i] {
                    self.footfalls += 1;
                    self.part_falls[i] += 1;
                }
                self.part_down[i] = down;
            }
            // Reach is what the body sweeps, not what it is drawn as.
            if self.total_steps % 4 == 0 {
                for (i, h) in self.handles.iter().enumerate() {
                    if let Some(rb) = self.bodies.get(*h) {
                        let p = &self.pheno.parts[i];
                        let iso = rb.position();
                        let half = iso.rotation * Vector3::new(0.0, p.len * 0.5, 0.0);
                        let tip = p.radius * (1.0 - p.flat) + p.r0 * p.flat;
                        let centre = iso.translation.vector;
                        let d = ((centre + half) - c).norm().max((centre - half - c).norm()) + tip;
                        if d > self.swept_reach {
                            self.swept_reach = d;
                        }
                    }
                }
            }
            if self.env == Environment::Shore && self.lowest_point() > 0.0 && c.x > self.shore_best {
                self.shore_best = c.x;
            }
            if self.mid_com.is_none() && self.time >= self.settle_duration() + 0.5 * self.cfg.episode_time {
                self.mid_com = Some(self.com());
            }
            if self.mid_com.is_some() {
                if let Some(rb) = self.bodies.get(self.handles[0]) {
                    self.yaw_sum += rb.angvel().y;
                    self.yaw_n += 1;
                    let com = self.com();
                    if let Some(prev) = self.prev_com {
                        let mut v = com - prev;
                        v.y = 0.0;
                        self.travel_sum += rb.position().rotation.inverse() * v;
                    }
                    self.prev_com = Some(com);
                }
            }
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
        EpisodeResult {
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
            swept_reach: self.swept_reach,
            trunk_contact_frac: if self.total_steps > 0 { self.trunk_contact_steps as f32 / self.total_steps as f32 } else { 0.0 },
            footfalls: self.footfalls,
            stepping_limbs: self.part_falls.iter().filter(|&&n| n >= 2).count() as u32,
            trunk_clearance: if self.total_steps > 0 { self.clearance_sum / self.total_steps as f32 } else { 0.0 },
            trunk_spin: if self.total_steps > 0 { self.trunk_spin_sum / self.total_steps as f32 } else { 0.0 },
            work: self.work,
            energy_leak: match self.energy_start {
                Some(e0) => (self.mechanical_energy() - e0) - self.work,
                None => 0.0,
            },
            cost_of_transport: {
                let g = -self.gravity.y;
                let d = distance.max(0.05);
                self.work / (self.pheno.total_mass * g * d)
            },
            air_distance: self.landing_distance.unwrap_or(distance),
            sustained_speed: match self.mid_com {
                Some(m) => {
                    let d = self.com() - m;
                    let half = 0.5 * self.cfg.episode_time;
                    if half > 0.0 { (d.x * d.x + d.z * d.z).sqrt() / half } else { 0.0 }
                }
                None => 0.0,
            },
            yaw_rate: if self.yaw_n > 0 { self.yaw_sum / self.yaw_n as f32 } else { 0.0 },
            travel_local: if self.travel_sum.norm() > 1e-4 { let t = self.travel_sum.normalize(); [t.x, t.y, t.z] } else { default_travel() },
            shore_progress: self.shore_best,
            climb_speed: match self.mid_com {
                Some(m) => {
                    let half = 0.5 * self.cfg.episode_time;
                    if half > 0.0 { (self.com().y - m.y) / half } else { 0.0 }
                }
                None => 0.0,
            },
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

    /// A bat-like body: trunk, two mirrored arm bones swung by the chest
    /// (the flap) and twisted at the shoulder (the feather, a quarter cycle
    /// off the flap), each carrying a welded horizontal sheet. The mounting
    /// was found by construction in examples/flap_probe3.
    fn bat(feather_amp: f32, feather_phase: f32, freq: f32, sheet: bool, mount: f32) -> Genome {
        use crate::genome::{Connection, Cpg, JointKind, Node, NodeKind};
        let flap = Cpg { amp: 1.0, phase: 0.0, bias: 0.0, amp2: 0.0, phase2: 0.0, amp3: feather_amp, phase3: feather_phase, freq_mult: 1.0 };
        let still = Cpg { amp: 0.0, phase: 0.0, bias: 0.0, amp2: 0.0, phase2: 0.0, amp3: 0.0, phase3: 0.0, freq_mult: 1.0 };
        let trunk = Node {
            kind: NodeKind::Capsule, len: 0.25, r0: 0.03, r1: 0.021, joint: JointKind::Rigid,
            flat: 0.0, grip: 0.0, stiffness: 1.0, freedom: 0.0, limit: 0.3, recursive_limit: 1, cpg: still,
            connections: vec![Connection { target: 1, along: 0.6, around: mount, pitch: 0.0, roll: mount, scale: 1.0, reflect: true, terminal_only: false, phase_flip: false }],
        };
        let arm = Node {
            kind: NodeKind::Capsule, len: 0.5, r0: 0.012, r1: 0.008, joint: JointKind::Ball,
            flat: 0.0, grip: 0.0, stiffness: 0.0, freedom: 1.0, limit: 1.2, recursive_limit: 1, cpg: flap,
            connections: vec![Connection { target: 2, along: 1.0, around: -PI, pitch: 0.0, roll: -PI, scale: 1.0, reflect: false, terminal_only: false, phase_flip: false }],
        };
        let (flat, r0, r1) = if sheet { (1.0, 0.5, 0.1) } else { (0.0, 0.0056, 0.0056) };
        let wing = Node {
            kind: if sheet { NodeKind::Membrane } else { NodeKind::Capsule }, len: 0.5, r0, r1, joint: JointKind::Rigid,
            flat, grip: 0.0, stiffness: 1.0, freedom: 0.0, limit: 0.3, recursive_limit: 1, cpg: still, connections: vec![],
        };
        let mut g = Genome::random(1);
        g.nodes = vec![trunk, arm, wing];
        g.root = 0;
        g.base_freq = freq;
        g.lung_frac = 0.1;
        g.water_store = 0.0;
        g.motility = 1.0;
        g
    }

    #[test]
    fn flapping_and_feathering_sheets_add_lift() {
        // The claim "flapping makes it fly" with its controls: the same body
        // holding its wings still (what posture alone does), and the same
        // body with the sheets replaced by rods of the same mass (what the
        // stroke does without a wing). Air launch at 3 m/s from 10 m, 4 s.
        //
        // A single run is not evidence: the dynamics are chaotic enough that
        // one float rounding in the mounting angle moves a run's mean height
        // by half a metre either way. So the claim is made over an ensemble
        // of mountings, frequencies and feather phases, and judged by its
        // mean, its share of positive cases and its best case.
        let planet = PlanetParams::earth();
        let t = 4.0;
        let run = |g: &Genome, amp_scale: f32| CreatureSim::run_episode(decode(g), &planet, Environment::Air, SimConfig { episode_time: t, amp_scale, ..Default::default() });
        let mut gains = Vec::new();
        let mut rod_drop = Vec::new();
        for &mount in &[PI / 2.0, PI / 2.0 + 1e-4, PI / 2.0 - 1e-4] {
            for &freq in &[1.5f32, 2.5, 4.0] {
                let hold = run(&bat(0.7, 0.0, freq, true, mount), 0.0);
                assert!(!hold.exploded);
                for &phase in &[PI / 2.0, -PI / 2.0, PI, 0.0] {
                    let flap = run(&bat(0.7, phase, freq, true, mount), 1.0);
                    assert!(!flap.exploded);
                    gains.push(flap.mean_height - hold.mean_height);
                }
                let rod = run(&bat(0.7, PI / 2.0, freq, false, mount), 1.0);
                assert!(!rod.exploded);
                rod_drop.push(hold.mean_height - rod.mean_height);
            }
        }
        let n = gains.len() as f32;
        let mean = gains.iter().sum::<f32>() / n;
        let positive = gains.iter().filter(|g| **g > 0.0).count() as f32 / n;
        let best = gains.iter().cloned().fold(f32::MIN, f32::max);
        // The stroke keeps the body higher than holding the same wing: on
        // average, in most cases, and by a lot in the best case.
        assert!(mean > 0.2, "mean gain {:.2} m over {} runs: {:?}", mean, gains.len(), gains);
        assert!(positive >= 0.7, "only {:.0}% of runs gained height: {:?}", positive * 100.0, gains);
        assert!(best > 0.5, "best gain {:.2} m", best);
        // Rods fall like a stone (mean height of a 10 m drop ~2.6 m): the
        // sheet is what the stroke works with.
        assert!(rod_drop.iter().all(|d| *d > 1.0), "rod twin not far below the held wing: {:?}", rod_drop);
    }

    #[test]
    fn episodes_run_and_are_deterministic() {
        let planet = PlanetParams::earth();
        let cfg = SimConfig { episode_time: 2.0, ..Default::default() };
        for seed in 0..6u64 {
            let g = Genome::random(seed);
            let a = CreatureSim::run_episode(decode(&g), &planet, Environment::Land, cfg);
            let b = CreatureSim::run_episode(decode(&g), &planet, Environment::Land, cfg);
            assert!(a.distance.is_finite());
            assert_eq!(a.distance.to_bits(), b.distance.to_bits(), "non-deterministic for seed {seed}");
        }
    }

    #[test]
    fn membrane_in_dense_air_falls_slower() {
        // A single large light membrane dropped in Earth air vs near-vacuum.
        let mut g = Genome::random(1);
        g.nodes.truncate(1);
        g.nodes[0].connections.clear();
        g.nodes[0].kind = NodeKind::Membrane;
        g.nodes[0].flat = 1.0;
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
