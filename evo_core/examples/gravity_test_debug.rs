use evo_core::genome::{Genome, NodeKind};
use evo_core::plant::decode_plant;
use rand::rngs::StdRng;
use rand::SeedableRng;
fn main() {
    let mut rng = StdRng::seed_from_u64(2);
    for k in 0..12 {
        let mut g = Genome::random_plant(&mut rng);
        g.nodes[0].len = 3.0; g.nodes[0].r0 = 0.02; g.nodes[0].r1 = 0.02; g.nodes[0].recursive_limit = 3; g.density = 800.0;
        let a = decode_plant(&g, 9.81); let b = decode_plant(&g, 30.0);
        println!("{k}: root {:?} parts {} height {:.1} stem {:.2} kg leaf {:.3} kg | cap 9.81 {:.3} stress {:.2} | cap 30 {:.3} stress {:.2}", g.nodes[0].kind, a.part_count, a.height, a.stem_mass, a.leaf_mass, a.growth_cap, a.stress, b.growth_cap, b.stress);
    }
    let _ = NodeKind::Capsule;
}
