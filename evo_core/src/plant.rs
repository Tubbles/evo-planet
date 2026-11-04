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

use crate::decode::{decode_with_caps, Phenotype, PLANT_MAX_LEN, PLANT_MAX_RADIUS};
use crate::genome::Genome;
use rapier3d::na::{Isometry3, UnitQuaternion, Vector3};
use serde::Serialize;

/// Wood strength constant (N·m per m^2 of section at unit lever); tuned so
/// a 0.05 m radius trunk carries a few hundred kilograms a couple of metres up.
pub const STEM_STRENGTH: f32 = 60_000.0;
/// Young's modulus of green stem tissue, Pa (wood 10 GPa, herbs ~1 GPa).
pub const STEM_MODULUS: f32 = 3.0e9;
pub const LEAF_DENSITY: f32 = 250.0;

#[derive(Clone, Debug, Serialize)]
pub struct PlantPart {
    pub kind: &'static str,
    #[serde(default)]
    pub flat: f32,
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
    /// Effective light-capturing area: leaf area projected onto the ground
    /// plane with overlap removed (self-shading), m^2.
    pub leaf_area: f32,
    /// Total leaf area regardless of overlap or tilt, m^2.
    pub leaf_area_total: f32,
    /// Sum over leaves of leaf mass x stem path length to the base, kg*m.
    pub transport: f32,
    /// Specific leaf area, m^2/kg.
    pub sla: f32,
    /// Fraction of linear size the stems can support (1 = all of it).
    pub growth_cap: f32,
    /// Worst stem load ratio at `gravity` (loads scale linearly with g).
    pub stress: f32,
    pub gravity: f32,
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

/// Light-capturing area of a set of leaves, m^2.
///
/// Light is sampled from nine directions (zenith and eight azimuths at 45
/// degrees). For each, every leaf's exact projected area is summed and the
/// union of their rasterised silhouettes measures how much overlaps; the
/// overlap fraction scales the exact sum, so thin leaves are not inflated by
/// the raster. Normalised so a lone horizontal leaf captures its full area;
/// a lone vertical blade gets about 0.6 of its area, as under a diffuse sky.
fn effective_leaf_area(leaves: &[(Isometry3<f32>, f32, f32)], total: f32) -> f32 {
    if leaves.is_empty() || total <= 0.0 {
        return 0.0;
    }
    const G: usize = 32;
    let mut dirs: Vec<Vector3<f32>> = vec![Vector3::y()];
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        dirs.push(Vector3::new(a.cos() * 0.7071, 0.7071, a.sin() * 0.7071));
    }
    // World extent for the raster cell size (shared across directions).
    let mut lo = Vector3::repeat(f32::MAX);
    let mut hi = Vector3::repeat(f32::MIN);
    for (iso, hc, hl) in leaves {
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let w = iso.translation.vector + iso.rotation * Vector3::new(sx * hc, sy * hl, 0.0);
            lo = lo.inf(&w);
            hi = hi.sup(&w);
        }
    }
    let extent = (hi - lo).max().max(0.02);
    let cell = extent / G as f32;
    let mut counts = vec![0u16; G * G];
    let mut touched: Vec<usize> = Vec::new();
    let mut sum_eff = 0.0;
    for d in &dirs {
        // Basis perpendicular to the light direction.
        let u = if d.x.abs() < 0.9 { Vector3::x() } else { Vector3::z() };
        let u = (u - d * u.dot(d)).normalize();
        let v = d.cross(&u);
        let ou = lo.dot(&u) - extent;
        let ov = lo.dot(&v) - extent;
        let span = 3.0 * extent;
        let g_cell = span / G as f32;
        counts.iter_mut().for_each(|c| *c = 0);
        let mut exact = 0.0;
        let mut raster_sum = 0usize;
        for (iso, hc, hl) in leaves {
            let normal = iso.rotation * Vector3::z();
            exact += 4.0 * hc * hl * normal.dot(d).abs();
            let nx = ((2.0 * hc / cell).ceil() as usize + 1).clamp(2, 24);
            let ny = ((2.0 * hl / cell).ceil() as usize + 1).clamp(2, 24);
            touched.clear();
            for a in 0..nx {
                for b in 0..ny {
                    let lx = (-1.0 + 2.0 * a as f32 / (nx - 1) as f32) * hc;
                    let ly = (-1.0 + 2.0 * b as f32 / (ny - 1) as f32) * hl;
                    let w = iso.translation.vector + iso.rotation * Vector3::new(lx, ly, 0.0);
                    let cu = (((w.dot(&u) - ou) / g_cell) as usize).min(G - 1);
                    let cv = (((w.dot(&v) - ov) / g_cell) as usize).min(G - 1);
                    touched.push(cv * G + cu);
                }
            }
            touched.sort_unstable();
            touched.dedup();
            raster_sum += touched.len();
            for &c in &touched {
                counts[c] += 1;
            }
        }
        let union = counts.iter().filter(|c| **c > 0).count();
        let overlap_frac = if raster_sum > 0 { union as f32 / raster_sum as f32 } else { 1.0 };
        sum_eff += exact * overlap_frac;
    }
    // Mean over directions, normalised so a lone horizontal leaf scores 1.0.
    let norm = (1.0 + 8.0 * 0.7071) / 9.0;
    (sum_eff / dirs.len() as f32 / norm).min(total)
}

pub fn growth_cap_for(stress: f32) -> f32 {
    if stress <= 1.0 { 1.0 } else { (1.0 / stress).sqrt().clamp(0.05, 1.0) }
}

impl PlantBody {
    /// Re-evaluate the structural cap under a different gravity without
    /// decoding again: every load is weight, so stress scales with g.
    pub fn set_gravity(&mut self, gravity: f32) {
        if self.gravity > 0.0 && (gravity - self.gravity).abs() > 1e-4 {
            self.stress *= gravity / self.gravity;
            self.gravity = gravity;
            self.growth_cap = growth_cap_for(self.stress);
        }
    }
}

pub fn decode_plant(g: &Genome, gravity: f32) -> PlantBody {
    decode_plant_with_caps(g, gravity, PLANT_MAX_LEN, PLANT_MAX_RADIUS)
}

/// The plant body at chosen part caps (the structural tests draw metre-scale stems).
pub fn decode_plant_with_caps(g: &Genome, gravity: f32, max_len: f32, max_radius: f32) -> PlantBody {
    let pheno = decode_with_caps(g, max_len, max_radius);
    let isos = placements(&pheno);
    let n = pheno.parts.len();
    let mut parts = Vec::with_capacity(n);
    let mut masses = vec![0.0f32; n];
    let mut tops = vec![0.0f32; n];
    let (mut leaf_mass, mut stem_mass, mut root_mass, mut leaf_area_total) = (0.0, 0.0, 0.0, 0.0);
    let mut height: f32 = 0.0;
    // Above-ground leaves (placement, half-chord, half-length) for the light model.
    let mut leaves: Vec<(Isometry3<f32>, f32, f32)> = Vec::new();
    for (i, p) in pheno.parts.iter().enumerate() {
        let iso = isos[i];
        let axis = iso.rotation * Vector3::y();
        let top = iso.translation.y + axis.y * p.len * 0.5 + p.radius;
        let below_ground = iso.translation.y < 0.0;
        // A part is `flat` leaf and `1 - flat` stem; whatever is below ground is root.
        let flat = p.flat;
        // Real leaves are 0.1-2 mm thick (SLA 2.5-40 m^2/kg at this density).
        let thickness = (p.r1 * 0.002).clamp(0.0001, 0.002);
        let area = flat * p.len * 2.0 * p.r0;
        let leaf_m = area * thickness * LEAF_DENSITY;
        let r_cap = 0.5 * (p.r0 + p.r1);
        let stem_m = (1.0 - flat) * (std::f32::consts::PI * r_cap * r_cap * p.len + 4.0 / 3.0 * std::f32::consts::PI * r_cap.powi(3)) * g.density;
        let mass = leaf_m + stem_m;
        let kind = if below_ground { "root" } else if flat > 0.5 { "leaf" } else { "stem" };
        if below_ground {
            root_mass += mass;
        } else {
            leaf_area_total += area;
            leaf_mass += leaf_m;
            stem_mass += stem_m;
            if area > 0.0 {
                leaves.push((iso, p.r0 * flat, p.len * 0.5));
            }
        }
        masses[i] = mass;
        tops[i] = top;
        if !below_ground {
            height = height.max(top);
        }
        let t = iso.translation.vector;
        let q = iso.rotation;
        parts.push(PlantPart {
            kind,
            flat,
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
    let _ = tops;
    let leaf_area = effective_leaf_area(&leaves, leaf_area_total);
    // Bending moments: each above-ground stem carries the weight of its
    // subtree acting at the horizontal lever arm from the stem's base. Sprawl
    // costs; a straight column is checked separately for Euler buckling.
    let mut ancestors: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 1..n {
        let mut a = pheno.parts[i].parent;
        while let Some(k) = a {
            ancestors[i].push(k);
            a = pheno.parts[k].parent;
        }
    }
    let mut moment = vec![0.0f32; n];
    // Euler buckling of a fixed-free column: load x height^2 against pi^2 E I / 4.
    let mut buckle = vec![0.0f32; n];
    let base_of = |i: usize| -> Vector3<f32> {
        let p = &pheno.parts[i];
        isos[i].translation.vector - isos[i].rotation * Vector3::new(0.0, p.len * 0.5, 0.0)
    };
    for j in 0..n {
        let pos_j = isos[j].translation.vector;
        for &i in ancestors[j].iter().chain(std::iter::once(&j)) {
            if isos[i].translation.y < 0.0 {
                continue;
            }
            let b = base_of(i);
            let dx = pos_j.x - b.x;
            let dz = pos_j.z - b.z;
            let horizontal = (dx * dx + dz * dz).sqrt();
            let vertical = (pos_j.y - b.y).max(0.0);
            let w = masses[j] * gravity;
            moment[i] += w * horizontal;
            buckle[i] += w * vertical * vertical;
        }
    }
    let mut worst: f32 = 0.0;
    for (i, p) in pheno.parts.iter().enumerate() {
        if isos[i].translation.y < 0.0 {
            continue;
        }
        // A sheet has almost no section to carry load; its share is 1 - flat.
        let r_cap = 0.5 * (p.r0 + p.r1);
        let r_eff = r_cap * (1.0 - p.flat) + 0.002 * p.flat;
        let section = std::f32::consts::PI * r_eff * r_eff;
        let stress = moment[i] / (section * STEM_STRENGTH);
        let second_moment = std::f32::consts::PI * r_eff.powi(4) / 4.0;
        let critical = std::f32::consts::PI.powi(2) * STEM_MODULUS * second_moment / 4.0;
        worst = worst.max(stress).max(buckle[i] / critical);
    }
    let growth_cap = growth_cap_for(worst);
    // Transport: every leaf is fed through the stems on its path to the base.
    let mut transport = 0.0;
    for j in 0..n {
        if pheno.parts[j].flat <= 0.0 || isos[j].translation.y < 0.0 {
            continue;
        }
        let path: f32 = ancestors[j].iter().map(|&k| pheno.parts[k].len * (1.0 - pheno.parts[k].flat)).sum();
        transport += masses[j] * pheno.parts[j].flat * path;
    }
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
        leaf_area_total,
        transport,
        sla: if leaf_mass > 0.0 { leaf_area_total / leaf_mass } else { 0.0 },
        growth_cap,
        stress: worst,
        gravity,
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
        let mut tall = 0;
        for _ in 0..40 {
            let mut g = Genome::random_plant(&mut rng);
            // Make it tall and thin so the check bites.
            g.nodes[0].len = 3.0;
            g.nodes[0].r0 = 0.02;
            g.nodes[0].r1 = 0.02;
            g.nodes[0].recursive_limit = 3;
            g.density = 800.0; // dense wood: a 9 m, 4 cm column is near its limit on Earth, buckles at 3 g
            let pa = decode_plant_with_caps(&g, 9.81, 3.0, 0.8);
            let pb = decode_plant_with_caps(&g, 30.0, 3.0, 0.8);
            // Only genomes whose recursion actually stacks the column are tall
            // enough to load it; short ones stand at any gravity.
            if pa.height < 6.5 {
                continue;
            }
            tall += 1;
            if pb.growth_cap < pa.growth_cap {
                lower += 1;
            }
        }
        assert!(tall >= 8, "too few tall columns: {tall}");
        assert!(lower * 10 >= tall * 8, "{lower} of {tall} tall columns lost cap at 3 g");
    }
}
