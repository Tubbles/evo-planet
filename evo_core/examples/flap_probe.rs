//! Does flapping a sheet produce lift and thrust in our physics?
//!
//! A bat-like body: trunk, two mirrored arm bones (driven: the flap) each
//! carrying a sheet on its own hinge about the span (driven: the feather,
//! a quarter cycle off the flap, the way a bird pitches its wing between
//! strokes). Our plate force is normal to the plate, so a wing that only
//! flaps up and down cannot make thrust; feathering is what tilts the
//! normal forward on the downstroke.
//!
//! Stage 1 finds mountings where the held wing glides (the sheet is
//! horizontal). Stage 2 runs those mountings four ways in the air launch:
//! held (no swing), flap only, flap with feather at each phase, and the
//! same body with the sheets replaced by thin rods of the same mass. What
//! flapping adds over holding is the powered lift; the rod twin shows the
//! sheet is what does it.
use evo_core::decode::decode;
use evo_core::genome::{Connection, Cpg, Genome, JointKind, Node, NodeKind};
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug)]
pub struct Mount {
    pub arm_around: f32,
    pub arm_roll: f32,
    pub sheet_around: f32,
    pub sheet_pitch: f32,
    pub sheet_roll: f32,
}

pub fn bat(m: Mount, flap_amp: f32, feather_amp: f32, feather_phase: f32, freq: f32, sheet: bool) -> Genome {
    let flap = Cpg { amp: flap_amp, phase: 0.0, bias: 0.0, amp2: 0.0, phase2: 0.0, freq_mult: 1.0, amp3: 0.0, phase3: 0.0 };
    let feather = Cpg { amp: feather_amp, phase: feather_phase, bias: 0.0, amp2: 0.0, phase2: 0.0, freq_mult: 1.0, amp3: 0.0, phase3: 0.0 };
    let still = Cpg { amp: 0.0, phase: 0.0, bias: 0.0, amp2: 0.0, phase2: 0.0, freq_mult: 1.0, amp3: 0.0, phase3: 0.0 };
    let trunk = Node {
        kind: NodeKind::Capsule, len: 0.25, r0: 0.03, r1: 0.02, joint: JointKind::Rigid,
        flat: 0.0, grip: 0.0, stiffness: 1.0, freedom: 0.0, limit: 0.3, recursive_limit: 1, cpg: still,
        connections: vec![Connection { target: 1, along: 0.6, around: m.arm_around, pitch: 0.0, roll: m.arm_roll, scale: 1.0, reflect: true, terminal_only: false, phase_flip: false }],
    };
    let arm = Node {
        kind: NodeKind::Capsule, len: 0.5, r0: 0.012, r1: 0.008, joint: JointKind::Hinge,
        flat: 0.0, grip: 0.0, stiffness: 0.0, freedom: 0.0, limit: 1.0, recursive_limit: 1, cpg: flap,
        connections: vec![Connection { target: 2, along: 1.0, around: m.sheet_around, pitch: m.sheet_pitch, roll: m.sheet_roll, scale: 1.0, reflect: false, terminal_only: false, phase_flip: false }],
    };
    // The sheet's `len` runs along the chord and 2*r0 along the span, so its
    // hinge (about its own x) is a pitch about the span: the feather.
    let (flat, r0, r1) = if sheet { (1.0, 0.5, 0.1) } else { (0.0, 0.0056, 0.0056) };
    let wing = Node {
        kind: if sheet { NodeKind::Membrane } else { NodeKind::Capsule }, len: 0.5, r0, r1, joint: JointKind::Hinge,
        flat, grip: 0.0, stiffness: if feather_amp > 0.0 { 0.0 } else { 1.0 }, freedom: 0.0, limit: 0.7, recursive_limit: 1, cpg: feather, connections: vec![],
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

fn main() {
    let planet = PlanetParams::earth();
    let t = 4.0;
    let stone = evo_core::sim::ballistic_distance(planet.gravity);
    let ph0 = decode(&bat(Mount { arm_around: 0.0, arm_roll: 0.0, sheet_around: 0.0, sheet_pitch: 0.0, sheet_roll: 0.0 }, 0.8, 0.5, 0.0, 2.0, true));
    for (i, p) in ph0.parts.iter().enumerate() {
        println!("part {i} parent {:?} flat {:.1} len {:.2} r0 {:.3} mass {:.3} kg torque {:.1} N·m", p.parent, p.flat, p.len, p.r0, p.mass, p.max_torque);
    }
    println!("mass {:.3} kg membrane area {:.3} m² stone {:.2} m\n", ph0.total_mass, ph0.membrane_area, stone);

    // Stage 1: which mountings glide when held?
    let angles = [0.0f32, PI / 2.0, PI, -PI / 2.0];
    let mut mounts: Vec<(f32, Mount)> = Vec::new();
    for &arm_around in &angles {
        for &arm_roll in &angles {
            for &sheet_around in &angles {
                for &sheet_pitch in &[0.0f32, 1.2] {
                    for &sheet_roll in &[0.0f32, PI / 2.0] {
                        let m = Mount { arm_around, arm_roll, sheet_around, sheet_pitch, sheet_roll };
                        let g = bat(m, 0.8, 0.0, 0.0, 2.0, true);
                        let hold = CreatureSim::run_episode(decode(&g), &planet, Environment::Air, SimConfig { episode_time: t, amp_scale: 0.0, ..Default::default() });
                        if !hold.exploded {
                            mounts.push((hold.air_distance - stone, m));
                        }
                    }
                }
            }
        }
    }
    mounts.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    println!("stage 1: {} mountings; best held glides beyond the stone: {:?}", mounts.len(), mounts.iter().take(5).map(|m| format!("{:.2}", m.0)).collect::<Vec<_>>());

    // Stage 2: flap and feather the best gliders.
    let mut best: Option<(f32, String)> = None;
    for (glide, m) in mounts.iter().take(6) {
        for &freq in &[1.5f32, 3.0] {
            let hold = CreatureSim::run_episode(decode(&bat(*m, 0.8, 0.5, 0.0, freq, true)), &planet, Environment::Air, SimConfig { episode_time: t, amp_scale: 0.0, ..Default::default() });
            let flap = CreatureSim::run_episode(decode(&bat(*m, 0.8, 0.0, 0.0, freq, true)), &planet, Environment::Air, SimConfig { episode_time: t, ..Default::default() });
            println!("mount {:?} glide {:.2} f {:.1}", m, glide, freq);
            println!("   hold:      h {:.2} m dist {:.2} aloft {:.2}", hold.mean_height, hold.air_distance, hold.airborne_frac);
            println!("   flap only: h {:.2} m dist {:.2} aloft {:.2} expl {} | gain {:+.2} m", flap.mean_height, flap.air_distance, flap.airborne_frac, flap.exploded, flap.mean_height - hold.mean_height);
            for &phase in &[PI / 2.0, -PI / 2.0, 0.0, PI] {
                let ff = CreatureSim::run_episode(decode(&bat(*m, 0.8, 0.5, phase, freq, true)), &planet, Environment::Air, SimConfig { episode_time: t, ..Default::default() });
                let rod = CreatureSim::run_episode(decode(&bat(*m, 0.8, 0.5, phase, freq, false)), &planet, Environment::Air, SimConfig { episode_time: t, ..Default::default() });
                let gain = ff.mean_height - hold.mean_height;
                let line = format!(
                    "   flap+feather phase {:+.2}: h {:.2} m dist {:.2} aloft {:.2} expl {} | gain {:+.2} m | rod twin: h {:.2} dist {:.2}",
                    phase, ff.mean_height, ff.air_distance, ff.airborne_frac, ff.exploded, gain, rod.mean_height, rod.air_distance
                );
                println!("{line}");
                if !ff.exploded && best.as_ref().map_or(true, |b| gain > b.0) {
                    best = Some((gain, format!("mount {:?} f {:.1}\n{line}", m, freq)));
                }
            }
        }
    }
    if let Some((g, l)) = best {
        println!("\nbest powered gain over holding {:+.2} m:\n{l}", g);
    }
}
