//! Genome graph -> phenotype part list.
//!
//! Each part is a rigid body. The part's local frame has its long axis along
//! +Y with the attachment (joint) point at the base, y = -len/2. Membranes
//! span Y, have chord along X and their panel normal along Z.

use crate::genome::{Cpg, Genome, JointKind, NodeKind};
use rapier3d::na::{UnitQuaternion, UnitVector3, Vector3};
use std::f32::consts::PI;

pub const MAX_PARTS: usize = 128;
pub const MAX_DEPTH: usize = 14;
pub const MEMBRANE_THICKNESS: f32 = 0.0002;
/// A sheet's thickness comes from its `r1` gene, the same mapping the plant
/// decoder uses for leaves (0.1-2 mm), times tissue density: 0.1-2 kg/m^2.
/// A bird's wing with its bone is about 1 kg/m^2; an alga sheet a tenth.
pub fn sheet_thickness(r1: f32) -> f32 {
    (r1 * 0.1).clamp(0.00001, 0.002)
}
/// Muscle stress, Pa: three times vertebrate muscle's 200 kPa, a strong
/// animal by any measure (the dial was turned up to see what strength buys). Torque is this
/// times the muscle's cross-section times its lever arm, the radius of the
/// segment it sits on, so a body's strength scales with its size as a real
/// body's does (a fixed torque per section made centimetre bodies a
/// hundred times too strong).
pub const MUSCLE_STRESS: f32 = 600_000.0;
/// Animal tissue is water-dense and not evolvable; the genome's density gene
/// is a plant trait (dry mass per fresh volume).
pub const ANIMAL_DENSITY: f32 = 1000.0;

#[derive(Clone, Debug)]
pub struct Part {
    pub node: usize,
    pub parent: Option<usize>,
    pub kind: NodeKind,
    pub len: f32,
    pub r0: f32,
    pub r1: f32,
    /// Radius of the circle with the part's cross-section (the collider's
    /// capsule radius when the part is rounder than flat).
    pub radius: f32,
    /// Thickness of the part, metres: a capsule's diameter at flat = 0, the
    /// sheet's thickness at flat = 1, and the geometric blend between (the
    /// form continuum is geometric in thickness, which spans four decades).
    pub thickness: f32,
    pub mass: f32,
    pub density: f32,
    pub joint: JointKind,
    /// Form continuum (0 capsule .. 1 sheet), grip (0..1), joint stiffness
    /// (0 free .. 1 welded) and freedom (0 one axis .. 1 two). Physics reads
    /// these; `kind` and `joint` are the nearest labels.
    pub flat: f32,
    pub grip: f32,
    pub stiffness: f32,
    pub freedom: f32,
    pub limit: f32,
    /// Anchor on the parent, in the parent's local frame.
    pub anchor_parent: Vector3<f32>,
    /// Rotation of this part's frame relative to the parent's frame.
    pub rot_from_parent: UnitQuaternion<f32>,
    pub cpg: Cpg,
    /// Maximum motor torque before fatigue scaling.
    pub max_torque: f32,
    /// Cross-section of the muscle that moves this part: the segment it hangs
    /// from (a thigh is swung by the hip, a wing by the chest), m².
    pub muscle_section: f32,
    /// Cross-section area used for strength.
    pub section_area: f32,
    pub reflected: bool,
}

#[derive(Clone, Debug)]
pub struct Phenotype {
    pub parts: Vec<Part>,
    pub lung_frac: f32,
    pub water_store: f32,
    /// Gills (0) to lungs (1).
    pub breathing: f32,
    pub base_freq: f32,
    pub total_mass: f32,
    pub membrane_area: f32,
    /// Mass held in sheets (flat share of each part), kg.
    pub membrane_mass: f32,
    pub torso_volume: f32,
    pub total_section_area: f32,
}

impl Phenotype {
    pub fn part_count(&self) -> usize {
        self.parts.len()
    }
    pub fn membrane_count(&self) -> usize {
        self.parts.iter().filter(|p| p.flat > 0.5).count()
    }
    /// Membrane area per kilogram, the wing-loading proxy used as a
    /// behaviour descriptor.
    pub fn area_per_mass(&self) -> f32 {
        if self.total_mass <= 0.0 {
            0.0
        } else {
            self.membrane_area / self.total_mass
        }
    }
}

struct Ctx<'a> {
    g: &'a Genome,
    parts: Vec<Part>,
    counts: Vec<u8>,
    max_len: f32,
    max_radius: f32,
}

#[allow(dead_code)]
fn capsule_volume(len: f32, r: f32) -> f32 {
    PI * r * r * len + 4.0 / 3.0 * PI * r * r * r
}

/// Rotation taking +Y to direction `d`, then rolled about `d`.
fn frame_from_dir(d: &Vector3<f32>, roll: f32) -> UnitQuaternion<f32> {
    let y = Vector3::y();
    let d = UnitVector3::new_normalize(*d);
    let align = UnitQuaternion::rotation_between(&y, &d).unwrap_or_else(|| {
        // Anti-parallel: rotate PI about X.
        UnitQuaternion::from_axis_angle(&Vector3::x_axis(), PI)
    });
    let roll_q = UnitQuaternion::from_axis_angle(&d, roll);
    roll_q * align
}

impl<'a> Ctx<'a> {
    fn expand(
        &mut self,
        node_idx: usize,
        parent: Option<usize>,
        scale: f32,
        anchor_parent: Vector3<f32>,
        rot_from_parent: UnitQuaternion<f32>,
        reflected: bool,
        phase_shift: f32,
        depth: usize,
    ) {
        if self.parts.len() >= MAX_PARTS || depth > MAX_DEPTH {
            return;
        }
        let node = &self.g.nodes[node_idx];
        if self.counts[node_idx] >= node.recursive_limit {
            return;
        }
        self.counts[node_idx] += 1;
        let is_last = self.counts[node_idx] == node.recursive_limit;

        // Size floor of a centimetre: the origin of heterotrophy happened far
        // smaller than the old 4 cm floor allowed (a few hundred grams of body).
        // Animal parts are small: at most 40 cm long and 8 cm in radius (a
        // part of a few kilograms at the largest), so a body is many cheap
        // segments rather than one great one.
        let len = (node.len * scale).clamp(MIN_LEN, self.max_len);
        let r0 = (node.r0 * scale).clamp(MIN_RADIUS, self.max_radius);
        let r1 = (node.r1 * scale).clamp(MIN_RADIUS, self.max_radius);
        let flat = node.flat();
        let grip = node.grip();
        let stiffness = node.stiffness();
        let freedom = node.freedom();
        // A part is a slab of width 2*r (the capsule's diameter) whose
        // thickness runs geometrically from that diameter (flat = 0, a
        // capsule) to the sheet thickness (flat = 1), with the cross-section's
        // shape factor running from a circle's to a rectangle's. Geometric,
        // because thickness spans four decades: a linear blend of mass made a
        // one-metre sheet at flat 0.9 weigh ninety kilograms, a wall between
        // sheets and bodies that no mutation could cross.
        let r_cap = 0.5 * (r0 + r1);
        let t_sheet = sheet_thickness(node.r1 * scale);
        let thickness = (2.0 * r_cap).powf(1.0 - flat) * t_sheet.powf(flat);
        let shape = (PI / 4.0).powf(1.0 - flat);
        // Width: the capsule's mean diameter, or the sheet's chord 2*r0 (its
        // r1 is the thickness gene).
        let half_width = (1.0 - flat) * r_cap + flat * r0;
        let section_area = 2.0 * half_width * thickness * shape;
        let radius = (section_area / PI).sqrt();
        let tissue_vol = section_area * len + (1.0 - flat) * 4.0 / 3.0 * PI * r_cap * r_cap * r_cap;
        let mass = tissue_vol * ANIMAL_DENSITY;
        let density = ANIMAL_DENSITY;
        // The muscle that swings a part sits on its parent (hip, shoulder,
        // chest), so torque scales with the parent's section, not the part's
        // own: a thin wing bone is moved by a thick chest.
        let muscle_section = parent.map(|q| self.parts[q].section_area).unwrap_or(section_area);
        let lever = parent.map(|q| self.parts[q].radius).unwrap_or(radius);
        let max_torque = MUSCLE_STRESS * muscle_section * lever;

        let mut cpg = node.cpg;
        cpg.phase += phase_shift;
        cpg.phase2 += phase_shift;

        let idx = self.parts.len();
        self.parts.push(Part {
            node: node_idx,
            parent,
            kind: node.kind,
            len,
            r0,
            r1,
            radius,
            thickness,
            mass,
            density,
            joint: if parent.is_none() { JointKind::Rigid } else { node.joint },
            flat,
            grip,
            stiffness: if parent.is_none() { 1.0 } else { stiffness },
            freedom,
            limit: node.limit,
            anchor_parent,
            rot_from_parent,
            cpg,
            max_torque,
            muscle_section,
            section_area,
            reflected,
        });

        let connections = node.connections.clone();
        for c in connections {
            if c.terminal_only && !is_last {
                continue;
            }
            let mirror = reflected ^ c.reflect;
            // Surface attachment point on this part.
            let around = if c.reflect { -c.around } else { c.around };
            let roll = if c.reflect { -c.roll } else { c.roll };
            let y = -len * 0.5 + c.along * len;
            let (nx, nz) = (around.cos(), around.sin());
            let surface_r = radius * (1.0 - flat);
            let anchor = Vector3::new(nx * surface_r, y, nz * surface_r);
            let normal = Vector3::new(nx, 0.0, nz);
            // Tilt the child from the outward normal toward this part's tip.
            let dir = normal * c.pitch.cos() + Vector3::y() * c.pitch.sin();
            let rot = frame_from_dir(&dir, roll);
            let child_phase = phase_shift + if c.reflect && c.phase_flip { PI } else { 0.0 };
            let child_scale = (scale * c.scale).clamp(0.15, 2.0);
            // Each connection follows the same recursion counters, so
            // snapshot and restore for sibling branches (Sims-style per-path
            // counting).
            let saved = self.counts.clone();
            self.expand(c.target, Some(idx), child_scale, anchor, rot, mirror, child_phase, depth + 1);
            // Only restore counters for nodes other than those in the chain
            // just expanded? Sims counts per recursive cycle along a path, so
            // restore fully: siblings start from the same counts.
            self.counts = saved;
        }
    }
}

/// Largest part, animal or plant: one centimetre long, 2.7 mm in radius
/// (a part of at most a fifth of a gram), so a body is many cheap
/// segments rather than one great one; the smallest is a millimetre.
pub const ANIMAL_MAX_LEN: f32 = 0.01;
pub const ANIMAL_MAX_RADIUS: f32 = 0.0027;
pub const PLANT_MAX_LEN: f32 = 0.01;
pub const PLANT_MAX_RADIUS: f32 = 0.0027;
pub const MIN_LEN: f32 = 0.001;
pub const MIN_RADIUS: f32 = 0.0004;

pub fn decode(g: &Genome) -> Phenotype {
    decode_with_caps(g, ANIMAL_MAX_LEN, ANIMAL_MAX_RADIUS)
}

pub fn decode_with_caps(g: &Genome, max_len: f32, max_radius: f32) -> Phenotype {
    let mut ctx = Ctx { g, parts: Vec::new(), counts: vec![0; g.nodes.len()], max_len, max_radius };
    ctx.expand(
        g.root,
        None,
        1.0,
        Vector3::zeros(),
        UnitQuaternion::identity(),
        false,
        0.0,
        0,
    );
    let mut parts = ctx.parts;

    // Lungs lighten the torso; stored water adds mass. A membrane root is a
    // sheet, not a trunk: it keeps its membrane mass (a 5 cm sheet is grams,
    // not the 600 g capsule its radius would imply) and has no lung cavity.
    let lung = g.lung_frac.clamp(0.0, 0.6);
    let water = g.water_store.clamp(0.0, 0.5);
    let torso_volume = {
        let f = parts[0].flat;
        let vol = parts[0].mass / ANIMAL_DENSITY;
        // Lungs lighten and stored water weighs the round share of the trunk.
        let torso_density = ANIMAL_DENSITY * (1.0 + (1.0 - f) * (water - 0.9 * lung));
        parts[0].mass = vol * torso_density;
        parts[0].density = torso_density;
        vol
    };

    // Clamp neighbouring mass ratios to 30:1 for solver stability by
    // raising the lighter part's density (12:1 erased a wing's lightness).
    for i in 1..parts.len() {
        let p = parts[i].parent.unwrap();
        let pm = parts[p].mass;
        if parts[i].mass * 30.0 < pm {
            let m = pm / 30.0;
            let vol = parts[i].mass / parts[i].density;
            parts[i].mass = m;
            parts[i].density = m / vol;
        }
    }

    let total_mass = parts.iter().map(|p| p.mass).sum();
    let membrane_area: f32 = parts.iter().map(|p| p.flat * p.len * 2.0 * p.r0).sum::<f32>().max(0.0);
    let total_section_area = parts.iter().skip(1).map(|p| p.muscle_section).sum();
    let membrane_mass: f32 = parts.iter().map(|p| p.mass * p.flat).sum();
    Phenotype {
        parts,
        lung_frac: lung,
        water_store: water,
        breathing: g.breathing,
        base_freq: g.base_freq,
        total_mass,
        membrane_area,
        membrane_mass,
        torso_volume,
        total_section_area,
    }
}

/// World placement of each part with the root at the origin, axis +Y.
pub fn placements(pheno: &Phenotype) -> Vec<rapier3d::na::Isometry3<f32>> {
    use rapier3d::na::Isometry3;
    let n = pheno.parts.len();
    let mut isos: Vec<Isometry3<f32>> = Vec::with_capacity(n);
    isos.push(Isometry3::identity());
    for i in 1..n {
        let p = &pheno.parts[i];
        let parent = p.parent.unwrap();
        let rot = isos[parent].rotation * p.rot_from_parent;
        let base_local = Vector3::new(0.0, -p.len * 0.5, 0.0);
        let pos = isos[parent].translation.vector + isos[parent].rotation * p.anchor_parent - rot * base_local;
        isos.push(Isometry3::from_parts(pos.into(), rot));
    }
    isos
}

/// Functional measures read off the body's geometry (no physics needed).
#[derive(Clone, Copy, Debug, Default)]
pub struct BodyMetrics {
    /// Farthest any part extends from the centre of mass, metres.
    pub reach: f32,
    /// Cross-section of the trunk's front end: the mouth, m^2.
    pub mouth_area: f32,
    /// Pad (claw, tooth) section relative to the trunk section.
    pub claws: f32,
    /// Fraction of body mass in rigidly attached parts: plating.
    pub plating: f32,
}

pub fn body_metrics(pheno: &Phenotype) -> BodyMetrics {
    let isos = placements(pheno);
    let total: f32 = pheno.parts.iter().map(|p| p.mass).sum::<f32>().max(1e-6);
    let mut com = Vector3::zeros();
    for (i, p) in pheno.parts.iter().enumerate() {
        com += isos[i].translation.vector * (p.mass / total);
    }
    let mut reach: f32 = 0.0;
    let mut pad_section = 0.0;
    let mut rigid_mass = 0.0;
    for (i, p) in pheno.parts.iter().enumerate() {
        let d = (isos[i].translation.vector - com).norm();
        let extent = p.len * 0.5 + p.radius * (1.0 - p.flat);
        reach = reach.max(d + extent);
        // Claws: gripping surface; plating: mass held rigid on round parts.
        pad_section += p.section_area * p.grip;
        if i > 0 {
            rigid_mass += p.mass * p.stiffness * (1.0 - p.flat);
        }
    }
    let root = &pheno.parts[0];
    let trunk_section = root.section_area;
    BodyMetrics {
        reach,
        // A mouth is the front of the trunk: its cross-section there (a sheet
        // has only its edge, width by thickness, and nibbles).
        mouth_area: root.section_area * (root.r0 / (0.5 * (root.r0 + root.r1)).max(1e-6)),
        claws: (pad_section / trunk_section.max(1e-6)).min(3.0),
        plating: rigid_mass / total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_bounded() {
        for seed in 0..200u64 {
            let g = Genome::random(seed);
            let p = decode(&g);
            assert!(!p.parts.is_empty());
            assert!(p.parts.len() <= MAX_PARTS);
            assert!(p.total_mass.is_finite() && p.total_mass > 0.0);
            for part in &p.parts {
                assert!(part.rot_from_parent.coords.iter().all(|c| c.is_finite()));
            }
        }
    }
}
