//! Flapping probe with the geometry chosen by construction, not by glide.
//!
//! The mounting is searched so that at rest: the arm points sideways
//! (world z), its hinge axis lies along the flight direction (so the arm
//! flaps up and down), the sheet is horizontal with its span along the arm
//! and its own hinge about the span (so it feathers, nose up and down).
//! Then the air launch is run held, flapping, flapping with feather at
//! several phases, and with the sheets replaced by rods of the same mass.
use evo_core::decode::{decode, placements};
use evo_core::genome::{Connection, Cpg, Genome, JointKind, Node, NodeKind};
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
use rapier3d::na::{UnitQuaternion, Vector3};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug)]
struct Mount {
    arm_around: f32,
    arm_pitch: f32,
    arm_roll: f32,
    sheet_around: f32,
    sheet_pitch: f32,
    sheet_roll: f32,
}

fn bat(m: Mount, trunk_r: f32, flap_amp: f32, feather_amp: f32, feather_phase: f32, feather_limit: f32, freq: f32, sheet: bool) -> Genome {
    let flap = Cpg { amp: flap_amp, phase: 0.0, bias: 0.0, amp2: 0.0, phase2: 0.0, freq_mult: 1.0 };
    let feather = Cpg { amp: feather_amp, phase: feather_phase, bias: 0.0, amp2: 0.0, phase2: 0.0, freq_mult: 1.0 };
    let still = Cpg { amp: 0.0, phase: 0.0, bias: 0.0, amp2: 0.0, phase2: 0.0, freq_mult: 1.0 };
    let trunk = Node {
        kind: NodeKind::Capsule, len: 0.25, r0: trunk_r, r1: trunk_r * 0.7, joint: JointKind::Rigid,
        flat: 0.0, grip: 0.0, stiffness: 1.0, freedom: 0.0, limit: 0.3, recursive_limit: 1, cpg: still,
        connections: vec![Connection { target: 1, along: 0.6, around: m.arm_around, pitch: m.arm_pitch, roll: m.arm_roll, scale: 1.0, reflect: true, terminal_only: false, phase_flip: false }],
    };
    let arm = Node {
        kind: NodeKind::Capsule, len: 0.5, r0: 0.012, r1: 0.008, joint: JointKind::Hinge,
        flat: 0.0, grip: 0.0, stiffness: 0.0, freedom: 0.0, limit: 1.2, recursive_limit: 1, cpg: flap,
        connections: vec![Connection { target: 2, along: 1.0, around: m.sheet_around, pitch: m.sheet_pitch, roll: m.sheet_roll, scale: 1.0, reflect: false, terminal_only: false, phase_flip: false }],
    };
    let (flat, r0, r1) = if sheet { (1.0, 0.5, 0.1) } else { (0.0, 0.0056, 0.0056) };
    let wing = Node {
        kind: if sheet { NodeKind::Membrane } else { NodeKind::Capsule }, len: 0.5, r0, r1, joint: JointKind::Hinge,
        flat, grip: 0.0, stiffness: if feather_amp > 0.0 { 0.0 } else { 1.0 }, freedom: 0.0, limit: feather_limit, recursive_limit: 1, cpg: feather, connections: vec![],
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

/// How well a mounting matches a bird at rest (1 = perfect).
fn score(m: Mount) -> f32 {
    let ph = decode(&bat(m, 0.03, 0.8, 0.5, 0.0, 1.4, 2.0, true));
    if ph.parts.len() < 3 {
        return -1.0;
    }
    let isos = placements(&ph);
    let root_rot = UnitQuaternion::rotation_between(&Vector3::y(), &Vector3::x()).unwrap();
    let r1 = root_rot * isos[1].rotation;
    let r2 = root_rot * isos[2].rotation;
    let arm_y = r1 * Vector3::y();
    let arm_x = r1 * Vector3::x();
    let sheet_x = r2 * Vector3::x();
    let sheet_z = r2 * Vector3::z();
    let pos2 = root_rot * isos[2].translation.vector;
    let lateral = arm_y.z.abs(); // arm along z
    let flap_axis = arm_x.x.abs(); // hinge along flight
    let horizontal = sheet_z.y.abs(); // normal up
    let feather_axis = sheet_x.z.abs(); // sheet hinge along span
    let out = (pos2.z.abs() / 0.5).min(1.0); // sheet out to the side
    lateral * flap_axis * horizontal * feather_axis * out
}

fn main() {
    let planet = PlanetParams::earth();
    let t = 4.0;
    {
        let m = Mount { arm_around: PI / 2.0, arm_pitch: 0.0, arm_roll: 0.0, sheet_around: 0.0, sheet_pitch: 0.0, sheet_roll: 0.0 };
        let ph = decode(&bat(m, 0.03, 0.8, 0.5, 0.0, 1.4, 2.0, true));
        println!("debug: {} parts, score {:.2}", ph.parts.len(), score(m));
    }
    let a8: Vec<f32> = (0..8).map(|i| i as f32 * PI / 4.0 - PI).collect();
    let mut found: Vec<(f32, Mount)> = Vec::new();
    for &arm_around in &a8 {
        for &arm_pitch in &[0.0f32, -0.5, 0.5] {
            for &arm_roll in &a8 {
                for &sheet_around in &a8 {
                    for &sheet_pitch in &[0.0f32, 0.6, 1.2, -0.6, -1.2] {
                        for &sheet_roll in &a8 {
                            let m = Mount { arm_around, arm_pitch, arm_roll, sheet_around, sheet_pitch, sheet_roll };
                            let s = score(m);
                            if s > 0.3 {
                                found.push((s, m));
                            }
                        }
                    }
                }
            }
        }
    }
    found.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    println!("{} mountings score > 0.3; top: {:?}", found.len(), found.iter().take(3).map(|(s, m)| format!("{:.2} {:?}", s, m)).collect::<Vec<_>>());
    if let Some((_, m)) = found.first() {
        let ph = decode(&bat(*m, 0.03, 0.8, 0.5, 0.0, 1.4, 2.0, true));
        let isos = placements(&ph);
        let root_rot = UnitQuaternion::rotation_between(&Vector3::y(), &Vector3::x()).unwrap();
        for (i, iso) in isos.iter().enumerate() {
            let r = root_rot * iso.rotation;
            let (x, y, z) = (r * Vector3::x(), r * Vector3::y(), r * Vector3::z());
            let p = root_rot * iso.translation.vector;
            println!("  part {i} pos ({:+.2},{:+.2},{:+.2}) x ({:+.2},{:+.2},{:+.2}) y ({:+.2},{:+.2},{:+.2}) z ({:+.2},{:+.2},{:+.2})", p.x, p.y, p.z, x.x, x.y, x.z, y.x, y.y, y.z, z.x, z.y, z.z);
        }
    }
    let stone = evo_core::sim::ballistic_distance(planet.gravity);
    let mut best: Option<(f32, String)> = None;
    for (s, m) in found.iter().take(2) {
        for &trunk_r in &[0.03f32, 0.05] {
            let ph = decode(&bat(*m, trunk_r, 1.0, 0.0, 0.0, 1.4, 2.0, true));
            println!("\nmount {:?} score {:.2} | mass {:.2} kg area {:.2} m² loading {:.1} kg/m² torque {:.1} N·m stone {:.1} m", m, s, ph.total_mass, ph.membrane_area, ph.total_mass / ph.membrane_area, ph.parts[1].max_torque, stone);
            for &freq in &[1.5f32, 2.5, 4.0] {
                let hold = CreatureSim::run_episode(decode(&bat(*m, trunk_r, 1.0, 0.7, 0.0, 1.4, freq, true)), &planet, Environment::Air, SimConfig { episode_time: t, amp_scale: 0.0, ..Default::default() });
                let flap = CreatureSim::run_episode(decode(&bat(*m, trunk_r, 1.0, 0.0, 0.0, 1.4, freq, true)), &planet, Environment::Air, SimConfig { episode_time: t, ..Default::default() });
                println!(" f {:.1} | hold: h {:.2} dist {:.2} aloft {:.2} | flap only: h {:.2} dist {:.2} aloft {:.2} expl {} gain {:+.2}", freq, hold.mean_height, hold.air_distance, hold.airborne_frac, flap.mean_height, flap.air_distance, flap.airborne_frac, flap.exploded, flap.mean_height - hold.mean_height);
                for &phase in &[PI / 2.0, -PI / 2.0, 0.0, PI] {
                    let ff = CreatureSim::run_episode(decode(&bat(*m, trunk_r, 1.0, 0.7, phase, 1.4, freq, true)), &planet, Environment::Air, SimConfig { episode_time: t, ..Default::default() });
                    let rod = CreatureSim::run_episode(decode(&bat(*m, trunk_r, 1.0, 0.7, phase, 1.4, freq, false)), &planet, Environment::Air, SimConfig { episode_time: t, ..Default::default() });
                    let gain = ff.mean_height - hold.mean_height;
                    let line = format!("    feather {:+.2}: h {:.2} dist {:.2} aloft {:.2} expl {} | gain {:+.2} m | rod: h {:.2} dist {:.2}", phase, ff.mean_height, ff.air_distance, ff.airborne_frac, ff.exploded, gain, rod.mean_height, rod.air_distance);
                    println!("{line}");
                    if !ff.exploded && best.as_ref().map_or(true, |b| gain > b.0) {
                        best = Some((gain, format!("mount {:?} trunk r {:.2} f {:.1}\n{line}", m, trunk_r, freq)));
                    }
                }
            }
        }
    }
    if let Some((g, l)) = best {
        println!("\nbest powered gain over holding {:+.2} m:\n{l}", g);
    }
}
