//! Plant decoder: the same graph genome as animals, read as a plant.
//!
//! Capsule nodes are stems and branches, Membrane nodes are leaves (their
//! spare radius gene `r1` is leaf thickness), Pad nodes are roots. The root
//! node stands vertically. Everything above ground counts toward height and
//! light capture; everything below ground is root mass.
//!
//! A static cantilever check gives a gravity-dependent growth cap: the plant
//! can only reach the fraction of its target size at which no stem exceeds
//! its strength under the weight above it.

use crate::decode::{decode, Phenotype};
use crate::genome::{Genome, NodeKind};
use rapier3d::na::{Isometry3, UnitQuaternion, Vector3};
use serde::Serialize;

/// Wood strength constant (N·m per m^2 of section at unit lever); tuned so
/// a 0.05 m radius trunk carries a few hundred kilograms a couple of metres up.
pub const STEM_STRENGTH: f32 = 60_000.0;
pub const LEAF_DENSITY: f32 = 250.0;

#[derive(Clone, Debug, Serialize)]
pub struct PlantPart {
    pub kind: &'static str,
    pub len: f32,
    pub radius: f32,
    pub r0: f32,
    pub thickness: f32,
    pub mass: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub qx: f32,
    pub qy: f32,
    pub qz: f32,
    pub qw: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlantBody {
    pub parts: Vec<PlantPart>,
    /// Full-grown height above ground, metres.
    pub height: f32,
    /// Full-grown total dry mass, kg.
    pub target_mass: f32,
    pub leaf_mass: f32,
    pub stem_mass: f32,
    pub root_mass: f32,
    /// Full-grown leaf area above ground, m^2.
    pub leaf_area: f32,
    /// Specific leaf area, m^2/kg.
    pub sla: f32,
    /// Fraction of linear size the stems can support (1 = all of it).
    pub growth_cap: f32,
    pub part_count: usize,
}

fn placements(pheno: &Phenotype) -> Vec<Isometry3<f32>> {
    let n = pheno.parts.len();
    let mut isos: Vec<Isometry3<f32>> = Vec::with_capacity(n);
    // Trunk base at the origin, axis up: the part's base is at local -len/2.
    let root_len = pheno.parts[0].len;
    isos.push(Isometry3::from_parts(Vector3::new(0.0, root_len * 0.5, 0.0).into(), UnitQuaternion::identity()));
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

pub fn decode_plant(g: &Genome, gravity: f32) -> PlantBody {
    let pheno = decode(g);
    let isos = placements(&pheno);
    let n = pheno.parts.len();
    let mut parts = Vec::with_capacity(n);
    let mut masses = vec![0.0f32; n];
    let mut tops = vec![0.0f32; n];
    let (mut leaf_mass, mut stem_mass, mut root_mass, mut leaf_area) = (0.0, 0.0, 0.0, 0.0);
    let mut height: f32 = 0.0;
    for (i, p) in pheno.parts.iter().enumerate() {
        let iso = isos[i];
        let axis = iso.rotation * Vector3::y();
        let top = iso.translation.y + axis.y * p.len * 0.5 + p.radius;
        let below_ground = iso.translation.y < 0.0;
        let (kind, mass, thickness) = match p.kind {
            NodeKind::Membrane => {
                let thickness = (p.r1 * 0.01).clamp(0.0001, 0.004);
                let area = p.len * 2.0 * p.r0;
                let m = area * thickness * LEAF_DENSITY;
                if !below_ground {
                    leaf_area += area;
                    leaf_mass += m;
                } else {
                    root_mass += m;
                }
                ("leaf", m, thickness)
            }
            NodeKind::Pad => {
                let m = p.mass / p.density * g.density;
                root_mass += m;
                ("root", m, 0.0)
            }
            NodeKind::Capsule => {
                let m = p.mass / p.density * g.density;
                if below_ground {
                    root_mass += m;
                } else {
                    stem_mass += m;
                }
                ("stem", m, 0.0)
            }
        };
        masses[i] = mass;
        tops[i] = top;
        if !below_ground {
            height = height.max(top);
        }
        let t = iso.translation.vector;
        let q = iso.rotation;
        parts.push(PlantPart {
            kind,
            len: p.len,
            radius: p.radius,
            r0: p.r0,
            thickness,
            mass,
            x: t.x,
            y: t.y,
            z: t.z,
            qx: q.i,
            qy: q.j,
            qz: q.k,
            qw: q.w,
        });
    }
    // Subtree mass and subtree top for the cantilever check.
    let mut sub_mass = masses.clone();
    let mut sub_top = tops.clone();
    for i in (1..n).rev() {
        let parent = pheno.parts[i].parent.unwrap();
        sub_mass[parent] += sub_mass[i];
        sub_top[parent] = sub_top[parent].max(sub_top[i]);
    }
    // Stress at full size: weight above times lever over section times strength.
    // Linear scaling f: mass ~ f^3, lever ~ f, section ~ f^2 => stress ~ f^2.
    let mut worst: f32 = 0.0;
    for (i, p) in pheno.parts.iter().enumerate() {
        if p.kind != NodeKind::Capsule || isos[i].translation.y < 0.0 {
            continue;
        }
        let base_y = isos[i].translation.y - (isos[i].rotation * Vector3::y()).y * p.len * 0.5;
        let lever = (sub_top[i] - base_y).max(0.05);
        let section = std::f32::consts::PI * p.radius * p.radius;
        let stress = sub_mass[i] * gravity * lever / (section * STEM_STRENGTH);
        worst = worst.max(stress);
    }
    let growth_cap = if worst <= 1.0 { 1.0 } else { (1.0 / worst).sqrt().clamp(0.05, 1.0) };
    let target_mass = (leaf_mass + stem_mass + root_mass).max(1e-4);
    PlantBody {
        part_count: parts.len(),
        parts,
        height,
        target_mass,
        leaf_mass,
        stem_mass,
        root_mass,
        leaf_area,
        sla: if leaf_mass > 0.0 { leaf_area / leaf_mass } else { 0.0 },
        growth_cap,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn plants_decode_with_leaves_and_roots() {
        let mut rng = StdRng::seed_from_u64(9);
        let mut with_leaves = 0;
        for _ in 0..50 {
            let g = Genome::random_plant(&mut rng);
            let b = decode_plant(&g, 9.81);
            assert!(b.target_mass > 0.0 && b.height > 0.0);
            if b.leaf_area > 0.0 {
                with_leaves += 1;
            }
        }
        assert!(with_leaves > 40);
    }

    #[test]
    fn heavier_gravity_lowers_growth_cap() {
        let mut rng = StdRng::seed_from_u64(2);
        let mut lower = 0;
        for _ in 0..40 {
            let mut g = Genome::random_plant(&mut rng);
            // Make it tall and thin so the check bites.
            g.nodes[0].len = 3.0;
            g.nodes[0].r0 = 0.03;
            g.nodes[0].r1 = 0.02;
            g.nodes[0].recursive_limit = 3;
            let a = decode_plant(&g, 9.81).growth_cap;
            let b = decode_plant(&g, 30.0).growth_cap;
            if b < a {
                lower += 1;
            }
        }
        assert!(lower > 20, "{lower}");
    }
}
