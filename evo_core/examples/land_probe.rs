//! Where the plants and animals are: sea, shore, inland; by day.
use evo_core::eco::EcoWorld;
use evo_core::planet::PlanetParams;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let days: u64 = args.get(0).and_then(|s| s.parse().ok()).unwrap_or(1000);
    let seed: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(4);
    let mut w = EcoWorld::new_with_area(PlanetParams::earth(), 600, seed, 100.0);
    let n = w.patches.len();
    let (mut sea, mut shore, mut inland) = (0, 0, 0);
    let mut moist_in = 0.0f32;
    for p in &w.patches {
        if p.water { sea += 1 } else if p.submersion > 0.0 { shore += 1 } else { inland += 1; moist_in += p.moisture }
    }
    println!("cells: sea {sea} shore {shore} inland {inland} (mean inland moisture {:.2})", moist_in / inland.max(1) as f32);
    for day in 1..=days {
        w.measure_pending(64);
        w.tick();
        if day % 200 == 0 {
            let mut pb = [0.0f32; 3];
            let mut pc = [0usize; 3];
            let mut ac = [0usize; 3];
            let mut am = [0.0f32; 3];
            let class = |i: usize| -> usize { let p = &w.patches[i]; if p.water { 0 } else if p.submersion > 0.0 { 1 } else { 2 } };
            for p in &w.plants { let c = class(p.patch); pb[c] += p.biomass; pc[c] += 1; }
            for a in &w.animals { let c = class(a.patch); ac[c] += 1; am[c] += a.mass; }
            let mut green_in = 0; let mut veg = vec![0.0f32; n];
            for p in &w.plants { veg[p.patch] += p.biomass; }
            for i in 0..n { if class(i) == 2 && veg[i] > 0.05 { green_in += 1; } }
            let mut tol = [0.0f32; 3]; let mut age = [0.0f32; 3];
            for p in &w.plants { let c = class(p.patch); tol[c] += p.genome.land_tolerance / pc[c].max(1) as f32; age[c] += p.age / pc[c].max(1) as f32; }
            println!("      land tolerance: sea {:.2} shore {:.2} inland {:.2} | mean age shore {:.0} inland {:.0} d", tol[0], tol[1], tol[2], age[1], age[2]);
            println!("day {day:>5} | plants sea {:>6.1} kg ({:>5}) shore {:>6.1} kg ({:>5}) inland {:>6.1} kg ({:>5}, {green_in} cells green) | animals sea {} shore {} inland {} (inland mean {:.3} kg)",
                pb[0], pc[0], pb[1], pc[1], pb[2], pc[2], ac[0], ac[1], ac[2], am[2] / ac[2].max(1) as f32);
        }
    }
}
