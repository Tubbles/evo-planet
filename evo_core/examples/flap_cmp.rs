//! Compare the test's bat with flap_probe3's best mounting, episode by episode.
use evo_core::decode::decode;
use evo_core::genome::{Connection, Cpg, Genome, JointKind, Node, NodeKind};
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
use std::f32::consts::PI;
fn bat(feather_amp: f32, feather_phase: f32, freq: f32, sheet: bool, sheet_limit: f32, a: f32) -> Genome {
    let flap = Cpg { amp: 1.0, phase: 0.0, bias: 0.0, amp2: 0.0, phase2: 0.0, amp3: feather_amp, phase3: feather_phase, freq_mult: 1.0 };
    let still = Cpg { amp: 0.0, phase: 0.0, bias: 0.0, amp2: 0.0, phase2: 0.0, amp3: 0.0, phase3: 0.0, freq_mult: 1.0 };
    let trunk = Node { kind: NodeKind::Capsule, len: 0.25, r0: 0.03, r1: 0.021, joint: JointKind::Rigid, flat: 0.0, grip: 0.0, stiffness: 1.0, freedom: 0.0, limit: 0.3, recursive_limit: 1, cpg: still,
        connections: vec![Connection { target: 1, along: 0.6, around: a, pitch: 0.0, roll: a, scale: 1.0, reflect: true, terminal_only: false, phase_flip: false }] };
    let arm = Node { kind: NodeKind::Capsule, len: 0.5, r0: 0.012, r1: 0.008, joint: JointKind::Ball, flat: 0.0, grip: 0.0, stiffness: 0.0, freedom: 1.0, limit: 1.2, recursive_limit: 1, cpg: flap,
        connections: vec![Connection { target: 2, along: 1.0, around: -PI, pitch: 0.0, roll: -PI, scale: 1.0, reflect: false, terminal_only: false, phase_flip: false }] };
    let (flat, r0, r1) = if sheet { (1.0, 0.5, 0.1) } else { (0.0, 0.0056, 0.0056) };
    let wing = Node { kind: if sheet { NodeKind::Membrane } else { NodeKind::Capsule }, len: 0.5, r0, r1, joint: JointKind::Rigid, flat, grip: 0.0, stiffness: 1.0, freedom: 0.0, limit: sheet_limit, recursive_limit: 1, cpg: still, connections: vec![] };
    let mut g = Genome::random(1);
    g.nodes = vec![trunk, arm, wing]; g.root = 0; g.base_freq = freq; g.lung_frac = 0.1; g.water_store = 0.0; g.motility = 1.0; g
}
fn main() {
    let planet = PlanetParams::earth();
    let grid = 6.0 * (PI / 4.0) - PI;
    for &a in &[PI / 2.0, grid, PI / 2.0 + 1e-6, PI / 2.0 - 1e-6, PI / 2.0 + 1e-4, PI / 2.0 - 1e-4, PI / 2.0 + 0.01] {
        for &freq in &[1.5f32, 2.5, 4.0] {
            let hold = CreatureSim::run_episode(decode(&bat(0.7, 0.0, freq, true, 0.3, a)), &planet, Environment::Air, SimConfig { episode_time: 4.0, amp_scale: 0.0, ..Default::default() });
            let mut line = format!("a {:+.7} f {:.1} | hold h {:.2} |", a, freq, hold.mean_height);
            for &ph in &[PI / 2.0, -PI / 2.0, PI, 0.0] {
                let flap = CreatureSim::run_episode(decode(&bat(0.7, ph, freq, true, 0.3, a)), &planet, Environment::Air, SimConfig { episode_time: 4.0, ..Default::default() });
                line += &format!(" ph {:+.2}: {:+.2}", ph, flap.mean_height - hold.mean_height);
            }
            let rod = CreatureSim::run_episode(decode(&bat(0.7, PI / 2.0, freq, false, 0.3, a)), &planet, Environment::Air, SimConfig { episode_time: 4.0, ..Default::default() });
            line += &format!(" | rod h {:.2}", rod.mean_height);
            println!("{line}");
        }
    }
}
