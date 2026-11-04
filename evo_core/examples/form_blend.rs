//! Mass, thickness and mouth of one part along the form continuum.
use evo_core::decode::{body_metrics, decode};
use evo_core::genome::Genome;
fn main() {
    let mut g = Genome::random(1);
    g.nodes.truncate(1);
    g.nodes[0].connections.clear();
    g.nodes[0].len = 1.0;
    g.nodes[0].r0 = 0.25;
    g.nodes[0].r1 = 0.25;
    for f in [0.0f32, 0.25, 0.5, 0.75, 0.9, 0.95, 0.99, 1.0] {
        g.nodes[0].flat = f;
        let p = decode(&g);
        let bm = body_metrics(&p);
        let part = &p.parts[0];
        println!("flat {:.2}: thickness {:.5} m  mass {:>8.3} kg  radius {:.4} m  mouth {:.6} m²", f, part.thickness, p.total_mass, part.radius, bm.mouth_area);
    }
}
