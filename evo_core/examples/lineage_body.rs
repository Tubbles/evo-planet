//! Describe the bodies of a checkpoint's largest lineages: genome nodes with
//! their attachment symmetries, and the decoded parts.
//!
//!     cargo run --release --example lineage_body -- WORLD.bin [LINEAGE...]
use evo_core::decode::decode;
use evo_core::eco::EcoWorld;
use evo_core::genome::Symmetry;
use std::collections::HashMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let w = EcoWorld::load(&args[0]).expect("checkpoint");
    let wanted: Vec<u32> = args[1..].iter().filter_map(|s| s.parse().ok()).collect();
    let mut by_lineage: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, a) in w.animals.iter().enumerate() {
        by_lineage.entry(a.lineage).or_default().push(i);
    }
    let mut order: Vec<(u32, usize)> = by_lineage.iter().map(|(k, v)| (*k, v.len())).collect();
    order.sort_by(|a, b| b.1.cmp(&a.1));
    let chosen: Vec<u32> = if wanted.is_empty() { order.iter().take(4).map(|x| x.0).collect() } else { wanted };
    println!("day {} animals {}", w.day, w.animals.len());
    for lin in chosen {
        let Some(members) = by_lineage.get(&lin) else { continue };
        // The modal body of the lineage: the most common genome key.
        let mut keys: HashMap<u64, usize> = HashMap::new();
        for &i in members {
            *keys.entry(w.animals[i].key).or_default() += 1;
        }
        let (key, n) = keys.iter().max_by_key(|(_, n)| **n).map(|(k, n)| (*k, *n)).unwrap();
        let a = &w.animals[*members.iter().find(|&&i| w.animals[i].key == key).unwrap()];
        let g = &a.genome;
        let ph = decode(g);
        println!(
            "\nlineage {lin}: {} members, modal body in {n}; parts {} mass {:.3} mg membrane {:.2} mm² motility {:.2} heterotrophy {:.2} repro_frac {:.2} offspring_frac {:.2}",
            members.len(),
            ph.parts.len(),
            ph.total_mass * 1e6,
            ph.membrane_area * 1e6,
            g.motility,
            g.heterotrophy,
            g.repro_frac,
            g.offspring_frac
        );
        for (i, node) in g.nodes.iter().enumerate() {
            println!(
                "  node {i}: {:?} len {:.2} mm r0 {:.2} mm r1 {:.2} mm flat {:.2} joint {:?} recursive_limit {}",
                node.kind,
                node.len * 1e3,
                node.r0 * 1e3,
                node.r1 * 1e3,
                node.flat,
                node.joint,
                node.recursive_limit
            );
            for c in &node.connections {
                let sym = match c.symmetry {
                    Symmetry::Single => "single".to_string(),
                    Symmetry::Bilateral => "bilateral".to_string(),
                    Symmetry::Radial(k) => format!("radial({k})"),
                };
                println!(
                    "    -> node {} {sym} along {:.2} around {:.2} pitch {:.2} roll {:.2} scale {:.2} reflect {} terminal_only {}",
                    c.target, c.along, c.around, c.pitch, c.roll, c.scale, c.reflect, c.terminal_only
                );
            }
        }
        let mut kinds: HashMap<String, usize> = HashMap::new();
        for p in &ph.parts {
            *kinds.entry(format!("{:?} node {}", p.kind, p.node)).or_default() += 1;
        }
        let mut kinds: Vec<_> = kinds.into_iter().collect();
        kinds.sort();
        println!("  decoded parts: {}", kinds.iter().map(|(k, n)| format!("{n}×{k}")).collect::<Vec<_>>().join(", "));
        for p in ph.parts.iter().take(6) {
            println!(
                "    part node {} parent {:?} len {:.2} mm r0 {:.2} r1 {:.2} thickness {:.3} mm mass {:.4} mg",
                p.node, p.parent, p.len * 1e3, p.r0 * 1e3, p.r1 * 1e3, p.thickness * 1e3, p.mass * 1e6
            );
        }
    }
}
