//! Genome graph -> phenotype part list.
//!
//! Each part is a rigid body. The part's local frame has its long axis along
//! +Y with the attachment (joint) point at the base, y = -len/2. Membranes
//! span Y, have chord along X and their panel normal along Z.

use crate::genome::{Cpg, Genome, JointKind, NodeKind};
use rapier3d::na::{UnitQuaternion, UnitVector3, Vector3};
use std::f32::consts::PI;

pub const MAX_PARTS: usize = 24;
pub const MAX_DEPTH: usize = 14;
pub const MEMBRANE_THICKNESS: f32 = 0.02;
/// Membrane areal density: 1.5 kg per m^2 of panel (thin skin and spars).
pub const MEMBRANE_AREAL_DENSITY: f32 = 1.5;
/// Muscle strength per unit cross-section (N*m per m^2 of limb section).
pub const STRENGTH_PER_AREA: f32 = 4000.0;

#[derive(Clone, Debug)]
pub struct Part {
    pub node: usize,
    pub parent: Option<usize>,
    pub kind: NodeKind,
    pub len: f32,
    pub r0: f32,
    pub r1: f32,
    /// Mean radius used for the collider.
    pub radius: f32,
    pub mass: f32,
    pub density: f32,
    pub joint: JointKind,
    pub limit: f32,
    /// Anchor on the parent, in the parent's local frame.
    pub anchor_parent: Vector3<f32>,
    /// Rotation of this part's frame relative to the parent's frame.
    pub rot_from_parent: UnitQuaternion<f32>,
    pub cpg: Cpg,
    /// Maximum motor torque before fatigue scaling.
    pub max_torque: f32,
    /// Cross-section area used for strength.
    pub section_area: f32,
    pub reflected: bool,
}

#[derive(Clone, Debug)]
pub struct Phenotype {
    pub parts: Vec<Part>,
    pub lung_frac: f32,
    pub water_store: f32,
    pub base_freq: f32,
    pub total_mass: f32,
    pub membrane_area: f32,
    pub torso_volume: f32,
    pub total_section_area: f32,
}

impl Phenotype {
    pub fn part_count(&self) -> usize {
        self.parts.len()
    }
    pub fn membrane_count(&self) -> usize {
        self.parts.iter().filter(|p| p.kind == NodeKind::Membrane).count()
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
}

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

        let len = (node.len * scale).clamp(0.04, 3.0);
        let r0 = (node.r0 * scale).clamp(0.02, 0.8);
        let r1 = (node.r1 * scale).clamp(0.02, 0.8);
        let radius = match node.kind {
            NodeKind::Membrane => r0,
            _ => 0.5 * (r0 + r1),
        };
        let (mass, density) = match node.kind {
            NodeKind::Membrane => {
                let area = len * 2.0 * r0;
                let m = area * MEMBRANE_AREAL_DENSITY;
                (m, m / (area * MEMBRANE_THICKNESS))
            }
            _ => {
                let v = capsule_volume(len, radius);
                (v * self.g.density, self.g.density)
            }
        };
        let section_area = match node.kind {
            NodeKind::Membrane => PI * 0.03 * 0.03 + len * 0.004, // thin spar
            _ => PI * radius * radius,
        };
        let max_torque = STRENGTH_PER_AREA * section_area;

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
            mass,
            density,
            joint: if parent.is_none() { JointKind::Rigid } else { node.joint },
            limit: node.limit,
            anchor_parent,
            rot_from_parent,
            cpg,
            max_torque,
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
            let surface_r = match node.kind {
                NodeKind::Membrane => 0.0,
                _ => radius,
            };
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

pub fn decode(g: &Genome) -> Phenotype {
    let mut ctx = Ctx { g, parts: Vec::new(), counts: vec![0; g.nodes.len()] };
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

    // Lungs lighten the torso; stored water adds mass.
    let torso_volume = capsule_volume(parts[0].len, parts[0].radius);
    let lung = g.lung_frac.clamp(0.0, 0.6);
    let water = g.water_store.clamp(0.0, 0.5);
    let torso_density = g.density * (1.0 - 0.9 * lung) * (1.0 + water);
    parts[0].density = torso_density;
    parts[0].mass = torso_volume * torso_density;

    // Clamp neighbouring mass ratios to about 10:1 for solver stability by
    // raising the lighter part's density.
    for i in 1..parts.len() {
        let p = parts[i].parent.unwrap();
        let pm = parts[p].mass;
        if parts[i].mass * 12.0 < pm {
            let m = pm / 12.0;
            let vol = parts[i].mass / parts[i].density;
            parts[i].mass = m;
            parts[i].density = m / vol;
        }
    }

    let total_mass = parts.iter().map(|p| p.mass).sum();
    let membrane_area: f32 = parts
        .iter()
        .filter(|p| p.kind == NodeKind::Membrane)
        .map(|p| p.len * 2.0 * p.r0)
        .sum::<f32>()
        .max(0.0);
    let total_section_area = parts.iter().skip(1).map(|p| p.section_area).sum();
    Phenotype {
        parts,
        lung_frac: lung,
        water_store: water,
        base_freq: g.base_freq,
        total_mass,
        membrane_area,
        torso_volume,
        total_section_area,
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
