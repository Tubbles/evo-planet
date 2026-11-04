//! Part counts and symmetries of random genomes.
use evo_core::decode::decode;
use evo_core::genome::{Genome, Symmetry};
fn main() {
    let (mut single, mut bilateral, mut radial) = (0, 0, 0);
    let mut parts = Vec::new();
    for seed in 0..200u64 {
        let g = Genome::random(seed);
        for n in &g.nodes {
            for c in &n.connections {
                match c.symmetry { Symmetry::Single => single += 1, Symmetry::Bilateral => bilateral += 1, Symmetry::Radial(_) => radial += 1 }
            }
        }
        parts.push(decode(&g).parts.len());
    }
    parts.sort();
    println!("attachments: single {single} bilateral {bilateral} radial {radial} | parts per random body: min {} median {} max {}", parts[0], parts[parts.len() / 2], parts[parts.len() - 1]);
}
