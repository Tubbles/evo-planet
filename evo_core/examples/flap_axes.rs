//! Which way does the probe's wing sheet lie at rest? Prints the world
//! directions of each part's local axes (y = length, x = chord, z = normal)
//! for the mounting flap_probe2 uses, so the feather hinge can be checked:
//! a feather is a pitch about the span, which needs the sheet's x along the
//! span and its y (the hinge swings the y end) along the flight direction.
use evo_core::decode::{decode, placements};
use evo_core::genome::{Connection, Cpg, Genome, JointKind, Node, NodeKind};
use rapier3d::na::Vector3;
use std::f32::consts::PI;

fn main() {
    let still = Cpg { amp: 0.0, phase: 0.0, bias: 0.0, amp2: 0.0, phase2: 0.0, freq_mult: 1.0 };
    for &(sheet_around, sheet_pitch, sheet_roll) in &[(0.0f32, 0.0f32, PI / 2.0), (0.0, 0.0, 0.0), (PI / 2.0, 0.0, 0.0), (0.0, 1.2, 0.0), (0.0, 1.2, PI / 2.0)] {
        let trunk = Node {
            kind: NodeKind::Capsule, len: 0.25, r0: 0.03, r1: 0.02, joint: JointKind::Rigid,
            flat: 0.0, grip: 0.0, stiffness: 1.0, freedom: 0.0, limit: 0.3, recursive_limit: 1, cpg: still,
            connections: vec![Connection { target: 1, along: 0.6, around: PI, pitch: 0.0, roll: PI, scale: 1.0, reflect: true, terminal_only: false, phase_flip: false }],
        };
        let arm = Node {
            kind: NodeKind::Capsule, len: 0.5, r0: 0.012, r1: 0.008, joint: JointKind::Hinge,
            flat: 0.0, grip: 0.0, stiffness: 0.0, freedom: 0.0, limit: 1.2, recursive_limit: 1, cpg: still,
            connections: vec![Connection { target: 2, along: 1.0, around: sheet_around, pitch: sheet_pitch, roll: sheet_roll, scale: 1.0, reflect: false, terminal_only: false, phase_flip: false }],
        };
        let wing = Node {
            kind: NodeKind::Membrane, len: 0.5, r0: 0.5, r1: 0.1, joint: JointKind::Hinge,
            flat: 1.0, grip: 0.0, stiffness: 0.0, freedom: 0.0, limit: 1.4, recursive_limit: 1, cpg: still, connections: vec![],
        };
        let mut g = Genome::random(1);
        g.nodes = vec![trunk, arm, wing];
        g.root = 0;
        let ph = decode(&g);
        let isos = placements(&ph);
        // The sim rotates the root so its +Y points along world +X (flight).
        let root_rot = rapier3d::na::UnitQuaternion::rotation_between(&Vector3::y(), &Vector3::x()).unwrap();
        println!("sheet around {:.2} pitch {:.2} roll {:.2}:", sheet_around, sheet_pitch, sheet_roll);
        for (i, iso) in isos.iter().enumerate() {
            let r = root_rot * iso.rotation;
            let x = r * Vector3::x();
            let y = r * Vector3::y();
            let z = r * Vector3::z();
            println!("  part {i} (flat {:.0}) pos {:?} | x(chord/hinge) ({:+.2},{:+.2},{:+.2}) y(len) ({:+.2},{:+.2},{:+.2}) z(normal) ({:+.2},{:+.2},{:+.2})",
                ph.parts[i].flat, (root_rot * iso.translation.vector).map(|v| (v * 100.0).round() / 100.0).as_slice(), x.x, x.y, x.z, y.x, y.y, y.z, z.x, z.y, z.z);
        }
    }
}
