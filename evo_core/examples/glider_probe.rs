//! Can a hand-built glider glide in this physics? Body capsule + two wings
//! (+ optional tail), dropped from 10 m, various incidences and wing positions.
use evo_core::decode::decode;
use evo_core::genome::{Connection, Cpg, Genome, JointKind, Node, NodeKind};
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, SimConfig};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::f32::consts::PI;

fn glider(wing_len: f32, wing_chord: f32, along: f32, incidence_deg: f32, tail: bool) -> Genome {
    let mut rng = StdRng::seed_from_u64(1);
    let mut g = Genome::random_with(&mut rng);
    let cpg = Cpg { amp: 0.0, phase: 0.0, bias: 0.0, amp2: 0.0, phase2: 0.0, freq_mult: 1.0, amp3: 0.0, phase3: 0.0 };
    let body = Node { kind: NodeKind::Capsule, flat: 0.0, grip: 0.0, stiffness: 1.0, freedom: 0.0, len: 0.25, r0: 0.025, r1: 0.02, joint: JointKind::Rigid, limit: 0.0, recursive_limit: 1, cpg, connections: vec![] };
    let wing = Node { kind: NodeKind::Membrane, flat: 1.0, grip: 0.0, stiffness: 1.0, freedom: 0.0, len: wing_len.max(0.04), r0: wing_chord * 0.5, r1: 0.1, joint: JointKind::Rigid, limit: 0.0, recursive_limit: 1, cpg, connections: vec![] };
    let tail_n = Node { kind: NodeKind::Membrane, flat: 1.0, grip: 0.0, stiffness: 1.0, freedom: 0.0, len: 0.15, r0: 0.05, r1: 0.1, joint: JointKind::Rigid, limit: 0.0, recursive_limit: 1, cpg, connections: vec![] };
    let inc = incidence_deg.to_radians();
    let mut conns = vec![
        Connection { target: 1, along, around: PI / 2.0, pitch: 0.0, roll: PI / 2.0 + inc, scale: 1.0, reflect: false, terminal_only: false, phase_flip: false },
        Connection { target: 1, along, around: PI / 2.0, pitch: 0.0, roll: PI / 2.0 + inc, scale: 1.0, reflect: true, terminal_only: false, phase_flip: false },
    ];
    if tail {
        // Horizontal stabiliser at the rear tip, level.
        conns.push(Connection { target: 2, along: 0.02, around: PI / 2.0, pitch: 0.0, roll: PI / 2.0, scale: 1.0, reflect: false, terminal_only: false, phase_flip: false });
        conns.push(Connection { target: 2, along: 0.02, around: PI / 2.0, pitch: 0.0, roll: PI / 2.0, scale: 1.0, reflect: true, terminal_only: false, phase_flip: false });
    }
    let mut body = body;
    body.connections = conns;
    g.nodes = vec![body, wing, tail_n];
    g.root = 0;
    g.lung_frac = 0.0;
    g.water_store = 0.0;
    g.base_freq = 1.0;
    g
}

fn main() {
    let planet = PlanetParams::earth();
    println!("stone: {:.2} m", evo_core::sim::ballistic_distance(9.81));
    for &(wl, wc, along, inc, tail) in &[
        (0.0f32, 0.05f32, 0.5f32, 0.0f32, false),
        (0.5, 0.15, 0.4, 3.0, true),
        (0.5, 0.15, 0.4, 5.0, true),
        (0.6, 0.15, 0.35, 5.0, true),
        (0.6, 0.15, 0.4, 5.0, true),
        (0.8, 0.2, 0.4, 5.0, true),
        (0.8, 0.2, 0.4, 2.0, true),
        (0.6, 0.12, 0.4, 5.0, true),
    ] {
        let g = glider(wl, wc, along, inc, tail);
        let ph = decode(&g);
        for t in [4.0f32] {
            let cfg = SimConfig { episode_time: t, ..Default::default() };
            let r = CreatureSim::run_episode(ph.clone(), &planet, Environment::Air, cfg);
            println!("wing {wl}x{wc} at {along} inc {inc:>4.1} tail {tail} | mass {:.2} kg loading {:.1} kg/m2 | {t:.0} s: air dist {:.2} m total {:.2} airborne {:.2} aloft {:.2} s exploded {}",
                ph.total_mass, ph.total_mass / ph.membrane_area, r.air_distance, r.distance, r.airborne_frac, r.time_aloft, r.exploded);
        }
    }
}
