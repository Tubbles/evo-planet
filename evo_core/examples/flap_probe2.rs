//! Second flapping probe: the best mounting from flap_probe, with a stronger
//! chest, bigger wings, a feather that can go nearly edge-on, and several
//! frequencies. Also a hover test: launched at zero forward speed, does
//! flapping slow the fall (drag-based rowing, the insect and hummingbird
//! mode)? Reports what flapping adds over holding the same posture.
use evo_core::decode::decode;
use evo_core::genome::{Connection, Cpg, Genome, JointKind, Node, NodeKind};
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
use std::f32::consts::PI;

fn bat(trunk_r: f32, wing_len: f32, half_span: f32, flap_amp: f32, feather_amp: f32, feather_phase: f32, feather_limit: f32, freq: f32, sheet: bool) -> Genome {
    let flap = Cpg { amp: flap_amp, phase: 0.0, bias: 0.0, amp2: 0.0, phase2: 0.0, freq_mult: 1.0 };
    let feather = Cpg { amp: feather_amp, phase: feather_phase, bias: 0.0, amp2: 0.0, phase2: 0.0, freq_mult: 1.0 };
    let still = Cpg { amp: 0.0, phase: 0.0, bias: 0.0, amp2: 0.0, phase2: 0.0, freq_mult: 1.0 };
    let trunk = Node {
        kind: NodeKind::Capsule, len: 0.25, r0: trunk_r, r1: trunk_r * 0.7, joint: JointKind::Rigid,
        flat: 0.0, grip: 0.0, stiffness: 1.0, freedom: 0.0, limit: 0.3, recursive_limit: 1, cpg: still,
        connections: vec![Connection { target: 1, along: 0.6, around: PI, pitch: 0.0, roll: PI, scale: 1.0, reflect: true, terminal_only: false, phase_flip: false }],
    };
    let arm = Node {
        kind: NodeKind::Capsule, len: 0.5, r0: 0.012, r1: 0.008, joint: JointKind::Hinge,
        flat: 0.0, grip: 0.0, stiffness: 0.0, freedom: 0.0, limit: 1.2, recursive_limit: 1, cpg: flap,
        connections: vec![Connection { target: 2, along: 1.0, around: 0.0, pitch: 0.0, roll: PI / 2.0, scale: 1.0, reflect: false, terminal_only: false, phase_flip: false }],
    };
    let (flat, r0, r1) = if sheet { (1.0, half_span, 0.1) } else { (0.0, 0.0056, 0.0056) };
    let wing = Node {
        kind: if sheet { NodeKind::Membrane } else { NodeKind::Capsule }, len: wing_len, r0, r1, joint: JointKind::Hinge,
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

fn main() {
    let planet = PlanetParams::earth();
    let t = 4.0;
    for &(trunk_r, wing_len, half_span) in &[(0.03f32, 0.5f32, 0.5f32), (0.05, 0.6, 0.5), (0.05, 0.4, 0.4)] {
        let ph = decode(&bat(trunk_r, wing_len, half_span, 1.0, 0.0, 0.0, 1.4, 2.0, true));
        println!("\nbody: trunk r {:.2} wing {:.1}x{:.1} m | mass {:.2} kg area {:.2} m² loading {:.1} kg/m² | arm torque {:.1} N·m", trunk_r, wing_len, 2.0 * half_span, ph.total_mass, ph.membrane_area, ph.total_mass / ph.membrane_area, ph.parts[1].max_torque);
        for &freq in &[1.5f32, 2.5, 4.0] {
            let hold = CreatureSim::run_episode(decode(&bat(trunk_r, wing_len, half_span, 1.0, 0.7, 0.0, 1.4, freq, true)), &planet, Environment::Air, SimConfig { episode_time: t, amp_scale: 0.0, ..Default::default() });
            let flap = CreatureSim::run_episode(decode(&bat(trunk_r, wing_len, half_span, 1.0, 0.0, 0.0, 1.4, freq, true)), &planet, Environment::Air, SimConfig { episode_time: t, ..Default::default() });
            println!(" f {:.1} | hold: h {:.2} dist {:.2} aloft {:.2} | flap only: h {:.2} dist {:.2} aloft {:.2} gain {:+.2}", freq, hold.mean_height, hold.air_distance, hold.airborne_frac, flap.mean_height, flap.air_distance, flap.airborne_frac, flap.mean_height - hold.mean_height);
            for &phase in &[PI / 2.0, -PI / 2.0, 0.0, PI, PI / 4.0, -PI / 4.0] {
                let ff = CreatureSim::run_episode(decode(&bat(trunk_r, wing_len, half_span, 1.0, 0.7, phase, 1.4, freq, true)), &planet, Environment::Air, SimConfig { episode_time: t, ..Default::default() });
                let rod = CreatureSim::run_episode(decode(&bat(trunk_r, wing_len, half_span, 1.0, 0.7, phase, 1.4, freq, false)), &planet, Environment::Air, SimConfig { episode_time: t, ..Default::default() });
                println!("    feather {:+.2}: h {:.2} dist {:.2} aloft {:.2} expl {} | gain {:+.2} m | rod: h {:.2}", phase, ff.mean_height, ff.air_distance, ff.airborne_frac, ff.exploded, ff.mean_height - hold.mean_height, rod.mean_height);
            }
        }
    }
}
