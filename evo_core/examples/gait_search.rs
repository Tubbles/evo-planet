//! Audit: can this physics and this genome produce an upright, multi-segment
//! walking gait at all? A direct search over genomes on the land episode,
//! with the ecology's own measure (sustained speed over the second half,
//! net of the motors-off twin) and an uprightness factor. Not the planet:
//! a tool to find out whether walkers are representable, and if not, why.
//!
//! Usage: gait_search [generations] [population] [seed] [out.json]
use evo_core::decode::{body_metrics, decode};
use evo_core::genome::Genome;
use evo_core::planet::PlanetParams;
use evo_core::sim::{CreatureSim, Environment, EpisodeResult, SimConfig};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

#[derive(Clone)]
struct Scored {
    genome: Genome,
    fitness: f32,
    speed: f32,
    upright: f32,
    parts: usize,
    height: f32,
    contact: f32,
    footfalls: u32,
    spin: f32,
    mass: f32,
    cot: f32,
    leak: f32,
}

fn evaluate(g: &Genome, planet: &PlanetParams) -> Scored {
    let ph = decode(g);
    let bm = body_metrics(&ph);
    let cfg = SimConfig { episode_time: 6.0, ..Default::default() };
    let r: EpisodeResult = CreatureSim::run_episode(ph.clone(), planet, Environment::Land, cfg);
    let sham = CreatureSim::run_episode(ph.clone(), planet, Environment::Land, SimConfig { episode_time: 6.0, motors: false, ..Default::default() });
    let speed = if r.exploded { 0.0 } else { (r.sustained_speed - if sham.exploded { 0.0 } else { sham.sustained_speed }).max(0.0) };
    let upright = (r.mean_height / (0.5 * bm.reach).max(0.01)).clamp(0.0, 1.0);
    // A gait, physically: the trunk is carried clear of the ground (no belly
    // dragging) and does not tumble (low angular speed), and progress is made
    // by repeated footfalls of limbs, at least two a second.
    let carried = (1.0 - r.trunk_contact_frac).powi(2);
    let steady = 1.0 / (1.0 + r.trunk_spin / 2.0);
    let stepping = (r.footfalls as f32 / 6.0 / 2.0).min(1.0);
    let segments = (ph.parts.len() as f32 - 1.0).clamp(0.0, 4.0) / 4.0;
    // Efficiency: energy is food. Walking mammals move for 1 to 5 J per
    // kilogram per metre; a body that thrashes its way forward pays far more.
    let efficiency = 1.0 / (1.0 + r.cost_of_transport / 3.0);
    let fitness = speed * carried * steady * stepping * efficiency * (0.3 + 0.7 * upright) * (0.4 + 0.6 * segments);
    Scored { genome: g.clone(), fitness, speed, upright, parts: ph.parts.len(), height: r.mean_height, contact: r.trunk_contact_frac, footfalls: r.footfalls, spin: r.trunk_spin, mass: ph.total_mass, cot: r.cost_of_transport, leak: r.energy_leak }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let gens: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(60);
    let pop: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(48);
    let seed: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let out = args.get(4).cloned();
    let planet = PlanetParams::earth();
    let planet_ref = &planet;
    let mut rng = StdRng::seed_from_u64(seed);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);

    let eval_all = |genomes: &[Genome]| -> Vec<Scored> {
        let chunk = (genomes.len() + threads - 1) / threads;
        std::thread::scope(|s| {
            let hs: Vec<_> = genomes.chunks(chunk).map(|c| s.spawn(move || c.iter().map(|g| evaluate(g, planet_ref)).collect::<Vec<_>>())).collect();
            hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
        })
    };

    let mut population: Vec<Scored> = eval_all(&(0..pop).map(|_| Genome::random_with(&mut rng)).collect::<Vec<_>>());
    let start = std::time::Instant::now();
    for gen in 0..gens {
        population.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
        let best = &population[0];
        println!(
            "gen {:>3} | best {:.3} | {:.2} m/s upright {:.2} h {:.2} m belly {:.2} footfalls {} spin {:.1} rad/s parts {} mass {:.0} kg COT {:.1} leak {:.0} J | median {:.3} | {:.0} s",
            gen, best.fitness, best.speed, best.upright, best.height, best.contact, best.footfalls, best.spin, best.parts, best.mass, best.cot, best.leak, population[pop / 2].fitness, start.elapsed().as_secs_f32()
        );
        // Elites survive; the rest are mutated children of tournament winners.
        let elite = pop / 6;
        let mut children: Vec<Genome> = Vec::with_capacity(pop - elite);
        while children.len() < pop - elite {
            let a = rng.gen_range(0..pop.min(population.len()));
            let b = rng.gen_range(0..pop.min(population.len()));
            let parent = if population[a].fitness >= population[b].fitness { &population[a] } else { &population[b] };
            let mut child = parent.genome.clone();
            let n = rng.gen_range(1..=3);
            for _ in 0..n {
                child = child.mutate(&mut rng).0;
            }
            children.push(child);
        }
        let scored = eval_all(&children);
        population.truncate(elite);
        population.extend(scored);
    }
    population.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
    let best = &population[0];
    println!(
        "\nbest: fitness {:.3} speed {:.2} m/s upright {:.2} height {:.2} m belly {:.2} footfalls {} spin {:.1} parts {} mass {:.0} kg COT {:.1} leak {:.0} J",
        best.fitness, best.speed, best.upright, best.height, best.contact, best.footfalls, best.spin, best.parts, best.mass, best.cot, best.leak
    );
    let ph = decode(&best.genome);
    for (i, p) in ph.parts.iter().enumerate() {
        println!(
            "  part {i} parent {:?} len {:.2} r {:.3} mass {:.2} flat {:.1} grip {:.1} stiff {:.1} free {:.1} limit {:.2} amp {:.2} f x{} torque {:.1}",
            p.parent, p.len, p.radius, p.mass, p.flat, p.grip, p.stiffness, p.freedom, p.limit, p.cpg.amp, p.cpg.freq_mult, p.max_torque
        );
    }
    if let Some(path) = out {
        std::fs::write(&path, best.genome.to_json()).expect("write");
        println!("saved {path}");
    }
}
