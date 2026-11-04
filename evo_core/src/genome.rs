//! Sims-style directed-graph genome.
//!
//! A node is one rigid segment type (capsule, membrane panel, or foot pad)
//! with its own size genes, joint type, joint limit, recursive limit and a
//! local CPG (oscillator) controller. Connections say how a child node hangs
//! off its parent: where on the surface, in what direction, at what scale,
//! optionally mirrored, optionally only on the last recursive expansion.
//!
//! Global genes carry planet-facing traits: base oscillation frequency,
//! lung (or gill) volume fraction of the torso, and water-storage mass
//! fraction. Both have physical costs in the simulation.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

pub const MAX_NODES: usize = 10;
pub const MAX_CONNECTIONS_PER_NODE: usize = 4;
pub const MIN_DIM: f32 = 0.04;
pub const MAX_LEN: f32 = 2.0;
pub const MAX_RADIUS: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// Tapered capsule limb or torso segment.
    Capsule,
    /// Thin, light panel spanning `len` by `2*r0`. Produces lift and drag.
    Membrane,
    /// Short high-friction segment. A foot.
    Pad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JointKind {
    /// One rotational degree of freedom about the child's local X axis.
    Hinge,
    /// Two driven axes (local X and Z), twist locked.
    Ball,
    /// Welded to the parent.
    Rigid,
}

/// Local oscillator parameters for one segment's joint.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cpg {
    pub amp: f32,
    pub phase: f32,
    pub bias: f32,
    /// Sideways swing (opened by the freedom gene).
    pub amp2: f32,
    pub phase2: f32,
    /// Twist about the part's own length (opened by the freedom gene): a
    /// shoulder's pronation, which pitches a wing or a fin through its stroke.
    #[serde(default)]
    pub amp3: f32,
    #[serde(default)]
    pub phase3: f32,
    /// Frequency multiplier relative to the genome's base frequency: 0.5, 1 or 2.
    pub freq_mult: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub target: usize,
    /// Position along the parent's axis, 0 = base, 1 = tip.
    pub along: f32,
    /// Angle around the parent's axis, radians.
    pub around: f32,
    /// Tilt of the child away from the parent's outward normal toward the
    /// parent's tip direction, radians in [-PI/2, PI/2].
    pub pitch: f32,
    /// Roll of the child about its own axis, radians.
    pub roll: f32,
    /// Multiplies all dimensions of the child subtree.
    pub scale: f32,
    /// Mirror across the parent's symmetry plane (negates `around` and `roll`).
    pub reflect: bool,
    /// Follow only on the parent's last recursive expansion.
    pub terminal_only: bool,
    /// Mirrored children get a half-cycle phase offset (alternating gait)
    /// instead of moving in phase (bounding, flapping).
    pub phase_flip: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Nearest category of the continuous form genes below; kept for old
    /// genomes and for labels. Physics reads `flat`, `grip`, `stiffness`,
    /// `freedom`, never this.
    pub kind: NodeKind,
    pub len: f32,
    pub r0: f32,
    pub r1: f32,
    /// Nearest category of `stiffness`/`freedom`; see `kind`.
    pub joint: JointKind,
    /// Form continuum: 0 a round capsule, 1 a thin sheet (chord 2*r0).
    #[serde(default = "unset")]
    pub flat: f32,
    /// Surface continuum: 0 smooth skin, 1 a gripping, clawed foot.
    #[serde(default = "unset")]
    pub grip: f32,
    /// Joint continuum: 0 a free joint, 1 welded to the parent.
    #[serde(default = "unset")]
    pub stiffness: f32,
    /// Joint continuum: 0 one driven axis (hinge), 1 two (ball).
    #[serde(default = "unset")]
    pub freedom: f32,
    /// Half range of joint motion, radians.
    pub limit: f32,
    pub recursive_limit: u8,
    pub cpg: Cpg,
    pub connections: Vec<Connection>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Genome {
    pub nodes: Vec<Node>,
    pub root: usize,
    /// Base CPG frequency in Hz.
    pub base_freq: f32,
    /// Fraction of torso volume given to lungs or gills (0..0.6).
    pub lung_frac: f32,
    /// Extra torso mass fraction held as stored water (0..0.5).
    pub water_store: f32,
    /// Tissue density, kg/m^3.
    pub density: f32,
    /// Plant genes (ignored by animals). Capsule nodes are stems, Membrane
    /// nodes are leaves (r1 = leaf thickness), Pad nodes are roots.
    /// 0 = fully aquatic, 1 = adapted to land. Costs upkeep everywhere.
    #[serde(default)]
    pub land_tolerance: f32,
    /// Seed mass in kg.
    #[serde(default = "default_seed_mass")]
    pub seed_mass: f32,
    /// Seed dispersal distance in patches at zero height.
    #[serde(default = "default_dispersal")]
    pub dispersal: f32,
    /// Lifespan in days.
    #[serde(default = "default_lifespan")]
    pub lifespan: f32,
    /// Fraction of full size a plant reaches before it starts seeding.
    #[serde(default = "default_repro_frac")]
    pub repro_frac: f32,
    /// 0 = sessile autotroph (lives as a plant), 1 = fully motile heterotroph
    /// (lives as an animal). Offspring at or above 0.5 are born animals.
    /// Scales muscle use, speed and activity cost; animals with membranes
    /// photosynthesise in proportion to (1 - motility).
    #[serde(default = "default_motility")]
    pub motility: f32,
    /// Gut type: 0 = long fermenting herbivore gut, 1 = short carnivore gut.
    /// Sets assimilation efficiency of plant versus animal tissue.
    #[serde(default)]
    pub gut_type: f32,
    /// Stomach capacity as a fraction of body mass.
    #[serde(default = "default_gut_frac")]
    pub gut_frac: f32,
    /// Energy reserve (fat) capacity as a fraction of body mass.
    #[serde(default = "default_fat_frac")]
    pub fat_frac: f32,
    /// Offspring mass as a fraction of adult mass.
    #[serde(default = "default_offspring_frac")]
    pub offspring_frac: f32,
    /// Lifespan multiplier on the allometric baseline.
    #[serde(default = "default_longevity")]
    pub longevity: f32,
    /// Plant leaf toughness (leaf economics spectrum): tough leaves lose less
    /// to grazers and digest worse, but photosynthesise slower per area.
    #[serde(default)]
    pub leaf_toughness: f32,
    /// Gas exchange medium, 0 = gills (water only) .. 1 = lungs (air only).
    /// Halfway breathes poorly in both: the amphibian's bargain.
    #[serde(default)]
    pub breathing: f32,
    /// Osmotrophy, 0..1: investment in absorbing dissolved and dead organic
    /// matter through the surface, at the expense of photosynthetic machinery
    /// (the mixotroph's trade-off). The first step of heterotrophy, before
    /// any mouth.
    #[serde(default)]
    pub heterotrophy: f32,
}

fn default_seed_mass() -> f32 {
    0.02
}
fn default_dispersal() -> f32 {
    1.5
}
fn default_lifespan() -> f32 {
    400.0
}
fn default_repro_frac() -> f32 {
    0.6
}
fn default_motility() -> f32 {
    1.0
}
fn default_gut_frac() -> f32 {
    0.1
}
fn default_fat_frac() -> f32 {
    0.15
}
fn default_offspring_frac() -> f32 {
    0.1
}
fn default_longevity() -> f32 {
    1.0
}

fn gaussian(rng: &mut StdRng) -> f32 {
    // Box-Muller.
    let u1: f32 = rng.gen_range(1e-6..1.0);
    let u2: f32 = rng.gen_range(0.0..1.0);
    (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
}

/// Relative Gaussian nudge, as in Sims: large values move by large amounts.
fn nudge(rng: &mut StdRng, v: f32, rel: f32, abs: f32, lo: f32, hi: f32) -> f32 {
    let g = gaussian(rng);
    (v + g * (v.abs() * rel + abs)).clamp(lo, hi)
}

fn wrap_angle(a: f32) -> f32 {
    let mut a = a % (2.0 * PI);
    if a > PI {
        a -= 2.0 * PI;
    }
    if a < -PI {
        a += 2.0 * PI;
    }
    a
}

impl Cpg {
    fn random(rng: &mut StdRng) -> Self {
        Self {
            amp: rng.gen_range(0.2..1.0),
            phase: rng.gen_range(-PI..PI),
            bias: rng.gen_range(-0.4..0.4),
            amp2: rng.gen_range(0.0..0.6),
            phase2: rng.gen_range(-PI..PI),
            amp3: rng.gen_range(0.0..0.6),
            phase3: rng.gen_range(-PI..PI),
            freq_mult: *[0.5f32, 1.0, 1.0, 2.0].iter().nth(rng.gen_range(0..4)).unwrap(),
        }
    }

    fn mutate(&mut self, rng: &mut StdRng, p: f32) {
        if rng.gen_bool(p as f64) {
            self.amp = nudge(rng, self.amp, 0.3, 0.05, 0.0, 1.5);
        }
        if rng.gen_bool(p as f64) {
            self.phase = wrap_angle(self.phase + gaussian(rng) * 0.6);
        }
        if rng.gen_bool(p as f64) {
            self.bias = nudge(rng, self.bias, 0.3, 0.05, -1.0, 1.0);
        }
        if rng.gen_bool(p as f64) {
            self.amp2 = nudge(rng, self.amp2, 0.3, 0.05, 0.0, 1.5);
        }
        if rng.gen_bool(p as f64) {
            self.phase2 = wrap_angle(self.phase2 + gaussian(rng) * 0.6);
        }
        if rng.gen_bool(p as f64) {
            self.amp3 = nudge(rng, self.amp3, 0.3, 0.05, 0.0, 1.5);
        }
        if rng.gen_bool(p as f64) {
            self.phase3 = wrap_angle(self.phase3 + gaussian(rng) * 0.6);
        }
        if rng.gen_bool((p * 0.5) as f64) {
            self.freq_mult = *[0.5f32, 1.0, 2.0].iter().nth(rng.gen_range(0..3)).unwrap();
        }
    }
}

impl Connection {
    fn random(rng: &mut StdRng, n_nodes: usize) -> Self {
        Self {
            target: rng.gen_range(0..n_nodes),
            along: rng.gen_range(0.0..1.0),
            around: rng.gen_range(-PI..PI),
            pitch: rng.gen_range(-1.2..1.2),
            roll: rng.gen_range(-PI..PI),
            scale: rng.gen_range(0.5..1.0),
            reflect: rng.gen_bool(0.5),
            terminal_only: rng.gen_bool(0.2),
            phase_flip: rng.gen_bool(0.5),
        }
    }

    /// Returns true if a body-affecting parameter changed.
    fn mutate(&mut self, rng: &mut StdRng, p: f32, n_nodes: usize) -> bool {
        let mut body = false;
        if rng.gen_bool((p * 0.3) as f64) {
            self.target = rng.gen_range(0..n_nodes);
            body = true;
        }
        if rng.gen_bool(p as f64) {
            self.along = nudge(rng, self.along, 0.2, 0.05, 0.0, 1.0);
            body = true;
        }
        if rng.gen_bool(p as f64) {
            self.around = wrap_angle(self.around + gaussian(rng) * 0.5);
            body = true;
        }
        if rng.gen_bool(p as f64) {
            self.pitch = nudge(rng, self.pitch, 0.2, 0.1, -1.5, 1.5);
            body = true;
        }
        if rng.gen_bool(p as f64) {
            self.roll = wrap_angle(self.roll + gaussian(rng) * 0.5);
            body = true;
        }
        if rng.gen_bool(p as f64) {
            self.scale = nudge(rng, self.scale, 0.2, 0.03, 0.3, 1.3);
            body = true;
        }
        if rng.gen_bool((p * 0.4) as f64) {
            self.reflect = !self.reflect;
            body = true;
        }
        if rng.gen_bool((p * 0.4) as f64) {
            self.terminal_only = !self.terminal_only;
            body = true;
        }
        if rng.gen_bool((p * 0.5) as f64) {
            self.phase_flip = !self.phase_flip;
        }
        body
    }
}

fn unset() -> f32 {
    -1.0
}

impl Node {
    /// Form gene, resolved from the category for genomes written before it existed.
    pub fn flat(&self) -> f32 {
        if self.flat >= 0.0 { self.flat } else if self.kind == NodeKind::Membrane { 1.0 } else { 0.0 }
    }
    pub fn grip(&self) -> f32 {
        if self.grip >= 0.0 { self.grip } else if self.kind == NodeKind::Pad { 1.0 } else { 0.0 }
    }
    pub fn stiffness(&self) -> f32 {
        if self.stiffness >= 0.0 { self.stiffness } else if self.joint == JointKind::Rigid { 1.0 } else { 0.0 }
    }
    pub fn freedom(&self) -> f32 {
        if self.freedom >= 0.0 { self.freedom } else if self.joint == JointKind::Ball { 1.0 } else { 0.0 }
    }
    /// Write the resolved continuous genes back and set the nearest labels.
    pub fn sync(&mut self) {
        self.flat = self.flat();
        self.grip = self.grip();
        self.stiffness = self.stiffness();
        self.freedom = self.freedom();
        self.kind = if self.flat > 0.5 { NodeKind::Membrane } else if self.grip > 0.5 { NodeKind::Pad } else { NodeKind::Capsule };
        self.joint = if self.stiffness > 0.8 { JointKind::Rigid } else if self.freedom > 0.5 { JointKind::Ball } else { JointKind::Hinge };
    }

    fn random(rng: &mut StdRng, n_nodes: usize, allow_connections: bool) -> Self {
        // Form genes: mostly round limbs, some sheets, a few feet; joints mostly
        // free, a few stiff. Continuous, so anything in between can arise.
        let flat: f32 = match rng.gen_range(0..10) { 0..=6 => rng.gen_range(0.0..0.2), 7..=8 => rng.gen_range(0.8..1.0), _ => rng.gen_range(0.0..1.0) };
        let grip: f32 = if rng.gen_bool(0.15) { rng.gen_range(0.6..1.0) } else { rng.gen_range(0.0..0.3) };
        let stiffness: f32 = if rng.gen_bool(0.1) { rng.gen_range(0.8..1.0) } else { rng.gen_range(0.0..0.3) };
        let freedom: f32 = rng.gen_range(0.0..1.0);
        let (len, r0, r1) = if flat > 0.5 {
            (rng.gen_range(0.3..1.2), rng.gen_range(0.1..0.5), 0.01)
        } else if grip > 0.5 {
            let r = rng.gen_range(0.06..0.2);
            (rng.gen_range(0.08..0.25), r, r)
        } else {
            let r0 = rng.gen_range(0.04..0.16);
            (rng.gen_range(0.2..0.8), r0, r0 * rng.gen_range(0.5..1.0))
        };
        let n_conn = if allow_connections { rng.gen_range(0..=2) } else { 0 };
        let mut node = Self {
            kind: NodeKind::Capsule,
            len,
            r0,
            r1,
            joint: JointKind::Hinge,
            flat,
            grip,
            stiffness,
            freedom,
            limit: rng.gen_range(0.3..1.4),
            recursive_limit: rng.gen_range(1..=3),
            cpg: Cpg::random(rng),
            connections: (0..n_conn).map(|_| Connection::random(rng, n_nodes)).collect(),
        };
        node.sync();
        node
    }

    /// Returns true if a body-affecting parameter changed.
    fn mutate_params(&mut self, rng: &mut StdRng, p: f32) -> bool {
        let mut body = false;
        if rng.gen_bool(p as f64) {
            self.len = nudge(rng, self.len, 0.2, 0.02, MIN_DIM, MAX_LEN);
            body = true;
        }
        if rng.gen_bool(p as f64) {
            self.r0 = nudge(rng, self.r0, 0.2, 0.01, MIN_DIM * 0.5, MAX_RADIUS);
            body = true;
        }
        if rng.gen_bool(p as f64) {
            self.r1 = nudge(rng, self.r1, 0.2, 0.01, MIN_DIM * 0.5, MAX_RADIUS);
            body = true;
        }
        // Form and joint genes drift; one mutation in five is a larger step.
        // Flatness steps are small: thickness is geometric in it (four
        // decades over the range), so 0.03 is already a third more thickness.
        self.sync();
        for k in 0..4 {
            if rng.gen_bool((p * 0.6) as f64) {
                let step = match (k, rng.gen_bool(0.2)) {
                    (0, true) => 0.15,
                    (0, false) => 0.03,
                    (_, true) => 0.4,
                    _ => 0.1,
                };
                let d = rng.gen_range(-step..step);
                match k {
                    0 => self.flat = (self.flat + d).clamp(0.0, 1.0),
                    1 => self.grip = (self.grip + d).clamp(0.0, 1.0),
                    2 => self.stiffness = (self.stiffness + d).clamp(0.0, 1.0),
                    _ => self.freedom = (self.freedom + d).clamp(0.0, 1.0),
                }
                body = true;
            }
        }
        self.sync();
        if rng.gen_bool(p as f64) {
            self.limit = nudge(rng, self.limit, 0.2, 0.05, 0.1, 1.6);
        }
        if rng.gen_bool((p * 0.4) as f64) {
            let delta: i8 = if rng.gen_bool(0.5) { 1 } else { -1 };
            self.recursive_limit = (self.recursive_limit as i8 + delta).clamp(1, 4) as u8;
            body = true;
        }
        self.cpg.mutate(rng, p);
        body
    }

    pub fn is_membrane(&self) -> bool {
        self.flat() > 0.5
    }
}

impl Genome {
    pub fn random(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        Self::random_with(&mut rng)
    }

    pub fn random_with(rng: &mut StdRng) -> Self {
        let n_nodes = rng.gen_range(2..=4);
        let mut nodes: Vec<Node> = Vec::with_capacity(n_nodes);
        for i in 0..n_nodes {
            let allow = i == 0 || rng.gen_bool(0.5);
            nodes.push(Node::random(rng, n_nodes, allow));
        }
        // Root: a torso-sized capsule with at least one connection out.
        let root = &mut nodes[0];
        root.kind = NodeKind::Capsule;
        root.flat = 0.0;
        root.grip = 0.0;
        root.len = rng.gen_range(0.4..1.0);
        root.r0 = rng.gen_range(0.09..0.2);
        root.r1 = root.r0 * rng.gen_range(0.6..1.0);
        root.recursive_limit = rng.gen_range(1..=2);
        if root.connections.is_empty() {
            root.connections.push(Connection::random(rng, n_nodes));
        }
        // Avoid self-loop-only roots: point first connection to another node.
        if n_nodes > 1 {
            root.connections[0].target = rng.gen_range(1..n_nodes);
        }
        Self {
            nodes,
            root: 0,
            base_freq: rng.gen_range(0.6..2.0),
            lung_frac: rng.gen_range(0.02..0.15),
            water_store: rng.gen_range(0.0..0.1),
            density: 1000.0,
            land_tolerance: 0.0,
            seed_mass: default_seed_mass(),
            dispersal: default_dispersal(),
            lifespan: default_lifespan(),
            repro_frac: default_repro_frac(),
            motility: 1.0,
            gut_type: 0.0,
            gut_frac: default_gut_frac(),
            fat_frac: default_fat_frac(),
            offspring_frac: default_offspring_frac(),
            longevity: 1.0,
            leaf_toughness: 0.0,
            breathing: 0.0,
            heterotrophy: 0.0,
        }
    }

    /// The simplest plant: one thin floating sheet (a Membrane node), no stem,
    /// no roots, fully aquatic. Everything else (stems, roots, leaves on
    /// branches, land tolerance) has to evolve from this by mutation.
    pub fn random_alga(rng: &mut StdRng) -> Self {
        let sheet = Node {
            kind: NodeKind::Membrane,
            len: rng.gen_range(0.5..1.5),
            r0: rng.gen_range(0.2..0.6),
            r1: rng.gen_range(0.1..0.4), // thickness gene (x0.01 m)
            joint: JointKind::Rigid,
            flat: 1.0,
            grip: 0.0,
            stiffness: 1.0,
            freedom: 0.0,
            limit: 0.3,
            recursive_limit: 1,
            cpg: Cpg::random(rng),
            connections: Vec::new(),
        };
        Self {
            nodes: vec![sheet],
            root: 0,
            base_freq: 1.0,
            lung_frac: 0.0,
            water_store: 0.0,
            density: rng.gen_range(100.0..200.0),
            land_tolerance: 0.0,
            seed_mass: rng.gen_range(0.001..0.01),
            dispersal: rng.gen_range(0.5..2.5),
            lifespan: rng.gen_range(40.0..400.0),
            repro_frac: rng.gen_range(0.4..0.9),
            motility: 0.0,
            gut_type: 0.0,
            gut_frac: default_gut_frac(),
            fat_frac: default_fat_frac(),
            offspring_frac: default_offspring_frac(),
            longevity: 1.0,
            leaf_toughness: 0.0,
            breathing: 0.0,
            heterotrophy: 0.0,
        }
    }

    /// A random plant: a vertical trunk (Capsule), one or two leaf nodes
    /// (Membrane) and a root node (Pad) pointing down. The same graph and
    /// mutation operators as animals; only the decoder differs. Used by tests
    /// and tools; the ecology starts from `random_alga`.
    pub fn random_plant(rng: &mut StdRng) -> Self {
        let trunk = Node {
            kind: NodeKind::Capsule,
            len: rng.gen_range(0.3..1.2),
            r0: rng.gen_range(0.02..0.05),
            r1: rng.gen_range(0.02..0.04),
            joint: JointKind::Rigid,
            flat: 0.0,
            grip: 0.0,
            stiffness: 1.0,
            freedom: 0.0,
            limit: 0.3,
            recursive_limit: rng.gen_range(1..=3),
            cpg: Cpg::random(rng),
            connections: Vec::new(),
        };
        let leaf = Node {
            kind: NodeKind::Membrane,
            len: rng.gen_range(0.3..1.0),
            r0: rng.gen_range(0.1..0.5),
            r1: rng.gen_range(0.02..0.12), // leaf thickness gene (x0.01 m)
            joint: JointKind::Rigid,
            flat: 1.0,
            grip: 0.0,
            stiffness: 1.0,
            freedom: 0.0,
            limit: 0.3,
            recursive_limit: rng.gen_range(1..=2),
            cpg: Cpg::random(rng),
            connections: Vec::new(),
        };
        let root = Node {
            kind: NodeKind::Pad,
            len: rng.gen_range(0.1..0.4),
            r0: rng.gen_range(0.02..0.04),
            r1: rng.gen_range(0.02..0.03),
            joint: JointKind::Rigid,
            flat: 0.0,
            grip: 1.0,
            stiffness: 1.0,
            freedom: 0.0,
            limit: 0.3,
            recursive_limit: rng.gen_range(1..=2),
            cpg: Cpg::random(rng),
            connections: Vec::new(),
        };
        let mut nodes = vec![trunk, leaf, root];
        // Trunk grows upward by self-reference; leaves on the upper part;
        // roots at the base pointing down.
        if rng.gen_bool(0.6) {
            nodes[0].connections.push(Connection {
                target: 0,
                along: 1.0,
                around: 0.0,
                pitch: 1.45,
                roll: rng.gen_range(-PI..PI),
                scale: rng.gen_range(0.6..0.95),
                reflect: false,
                terminal_only: false,
                phase_flip: false,
            });
        }
        let n_leaf = rng.gen_range(1..=3);
        for _ in 0..n_leaf {
            nodes[0].connections.push(Connection {
                target: 1,
                along: rng.gen_range(0.4..1.0),
                around: rng.gen_range(-PI..PI),
                pitch: rng.gen_range(0.1..0.9),
                roll: rng.gen_range(-0.6..0.6),
                scale: rng.gen_range(0.6..1.1),
                reflect: rng.gen_bool(0.7),
                terminal_only: rng.gen_bool(0.3),
                phase_flip: false,
            });
        }
        nodes[0].connections.push(Connection {
            target: 2,
            along: 0.0,
            around: rng.gen_range(-PI..PI),
            pitch: -1.3,
            roll: 0.0,
            scale: rng.gen_range(0.6..1.2),
            reflect: rng.gen_bool(0.6),
            terminal_only: false,
            phase_flip: false,
        });
        if rng.gen_bool(0.5) {
            nodes[2].connections.push(Connection {
                target: 2,
                along: 1.0,
                around: 0.0,
                pitch: rng.gen_range(0.6..1.4),
                roll: rng.gen_range(-PI..PI),
                scale: rng.gen_range(0.5..0.9),
                reflect: false,
                terminal_only: false,
                phase_flip: false,
            });
        }
        Self {
            nodes,
            root: 0,
            base_freq: 1.0,
            lung_frac: 0.0,
            water_store: 0.0,
            // Dry-mass density of fresh plant tissue (mostly water): herbs ~50, wood ~300.
            density: rng.gen_range(40.0..200.0),
            land_tolerance: rng.gen_range(0.0..0.2),
            seed_mass: rng.gen_range(0.0005..0.01),
            dispersal: rng.gen_range(0.5..3.0),
            lifespan: rng.gen_range(60.0..1200.0),
            repro_frac: rng.gen_range(0.4..0.9),
            motility: 0.0,
            gut_type: 0.0,
            gut_frac: default_gut_frac(),
            fat_frac: default_fat_frac(),
            offspring_frac: default_offspring_frac(),
            longevity: 1.0,
            leaf_toughness: 0.0,
            breathing: 0.0,
            heterotrophy: 0.0,
        }
    }

    /// Number of nodes plus connections, a cheap structural signature.
    pub fn structural_size(&self) -> usize {
        self.nodes.len() + self.nodes.iter().map(|n| n.connections.len()).sum::<usize>()
    }

    /// Mutate following Sims' sequence: node parameters, maybe add a node,
    /// connection parameters, add/remove connections, garbage collect.
    /// Returns (child, body_changed).
    pub fn mutate(&self, rng: &mut StdRng) -> (Genome, bool) {
        self.mutate_scaled(rng, 1.0)
    }

    /// Mutate with the body's genes (segment sizes, forms, joints, structure,
    /// lungs, water store, motility, density) hit at `body_scale` times the
    /// rate of the rest. Morphology mutates far more rarely than physiology
    /// in life, and every changed body is a new physics measurement.
    pub fn mutate_scaled(&self, rng: &mut StdRng, body_scale: f32) -> (Genome, bool) {
        let mut g = self.clone();
        let mut body = false;
        // Scale per-element probability so that on average at least ~1.5
        // mutations hit the genome regardless of size.
        let n_elems: usize = g.nodes.iter().map(|n| 8 + n.connections.len() * 9).sum::<usize>() + 4;
        let p = (1.5 / n_elems as f32).clamp(0.02, 0.35);
        let pb = p * body_scale;

        for i in 0..g.nodes.len() {
            if g.nodes[i].mutate_params(rng, pb * 3.0) {
                body = true;
            }
        }

        // Global genes.
        if rng.gen_bool((p * 3.0) as f64) {
            g.base_freq = nudge(rng, g.base_freq, 0.2, 0.05, 0.2, 4.0);
        }
        if rng.gen_bool((pb * 3.0) as f64) {
            g.lung_frac = nudge(rng, g.lung_frac, 0.25, 0.01, 0.0, 0.6);
            body = true;
        }
        if rng.gen_bool((pb * 3.0) as f64) {
            g.water_store = nudge(rng, g.water_store, 0.25, 0.01, 0.0, 0.5);
            body = true;
        }
        // Integument against air: a plant's cuticle, an animal's waterproof
        // skin. One gene on both sides of the motility continuum.
        if rng.gen_bool((p * 2.5) as f64) {
            g.land_tolerance = (g.land_tolerance + rng.gen_range(-0.12..0.12f32)).clamp(0.0, 1.0);
            body = true;
        }
        if rng.gen_bool((p * 2.0) as f64) {
            g.seed_mass = nudge(rng, g.seed_mass, 0.3, 0.001, 0.0005, 2.0);
        }
        if rng.gen_bool((p * 2.0) as f64) {
            g.dispersal = nudge(rng, g.dispersal, 0.3, 0.1, 0.1, 12.0);
        }
        if rng.gen_bool((p * 2.0) as f64) {
            g.lifespan = nudge(rng, g.lifespan, 0.2, 5.0, 10.0, 5000.0);
        }
        if rng.gen_bool((p * 2.0) as f64) {
            g.repro_frac = nudge(rng, g.repro_frac, 0.15, 0.03, 0.1, 1.0);
        }
        if rng.gen_bool((pb * 1.5) as f64) {
            // Motility drifts, but one mutation in ten is a flagellum: a jump.
            let step = if rng.gen_bool(0.1) { 0.35 } else { 0.08 };
            g.motility = (g.motility + rng.gen_range(-step..step)).clamp(0.0, 1.0);
            body = true;
        }
        // Anatomy and life history (animal side).
        if rng.gen_bool((p * 2.0) as f64) {
            g.gut_type = (g.gut_type + rng.gen_range(-0.08..0.08f32)).clamp(0.0, 1.0);
        }
        if rng.gen_bool((p * 1.5) as f64) {
            g.gut_frac = nudge(rng, g.gut_frac, 0.2, 0.01, 0.02, 0.4);
        }
        if rng.gen_bool((p * 1.5) as f64) {
            g.fat_frac = nudge(rng, g.fat_frac, 0.2, 0.01, 0.02, 0.6);
        }
        if rng.gen_bool((p * 1.5) as f64) {
            g.offspring_frac = nudge(rng, g.offspring_frac, 0.2, 0.01, 0.01, 0.5);
        }
        if rng.gen_bool((p * 1.5) as f64) {
            g.longevity = nudge(rng, g.longevity, 0.15, 0.03, 0.3, 3.0);
        }
        if rng.gen_bool((p * 1.5) as f64) {
            g.leaf_toughness = (g.leaf_toughness + rng.gen_range(-0.08..0.08f32)).clamp(0.0, 1.0);
        }
        if rng.gen_bool((p * 1.5) as f64) {
            g.breathing = (g.breathing + rng.gen_range(-0.08..0.08f32)).clamp(0.0, 1.0);
        }
        if rng.gen_bool((p * 1.5) as f64) {
            g.heterotrophy = (g.heterotrophy + rng.gen_range(-0.08..0.08f32)).clamp(0.0, 1.0);
        }
        if rng.gen_bool((pb * 1.5) as f64) {
            g.density = nudge(rng, g.density, 0.12, 5.0, 20.0, 1200.0);
            body = true;
        }

        // Duplicate a segment (serial homology): copy a used node with its
        // attachment onto the same host, shifted along the host or mirrored,
        // keeping its oscillator. Two legs become four; a fin becomes a pair.
        // This is how limbs multiply in animals, by duplication of a working
        // module, not by inventing a random one.
        if g.nodes.len() < MAX_NODES && rng.gen_bool((pb * 2.0) as f64) {
            let hosts: Vec<(usize, usize)> = g
                .nodes
                .iter()
                .enumerate()
                .flat_map(|(h, n)| n.connections.iter().enumerate().map(move |(ci, _)| (h, ci)))
                .filter(|&(h, ci)| g.nodes[h].connections[ci].target != h && g.nodes[h].connections.len() < MAX_CONNECTIONS_PER_NODE)
                .collect();
            if !hosts.is_empty() {
                let (h, ci) = hosts[rng.gen_range(0..hosts.len())];
                let src = g.nodes[h].connections[ci];
                let copy = g.nodes[src.target].clone();
                let new_idx = g.nodes.len();
                g.nodes.push(copy);
                let mut c = src;
                c.target = new_idx;
                if rng.gen_bool(0.5) {
                    // Serial: further along the host, same side.
                    c.along = (c.along + rng.gen_range(-0.4..0.4f32)).clamp(0.0, 1.0);
                    c.phase_flip = rng.gen_bool(0.5);
                } else {
                    // Bilateral: the mirror image, in antiphase half the time.
                    c.reflect = !c.reflect;
                    c.phase_flip = rng.gen_bool(0.5);
                }
                g.nodes[h].connections.push(c);
                body = true;
            }
        }

        // Add a node (initially unconnected unless a pointer mutates to it).
        if g.nodes.len() < MAX_NODES && rng.gen_bool((pb * 2.0) as f64) {
            let n = g.nodes.len();
            g.nodes.push(Node::random(rng, n + 1, true));
            // Give it a chance to be used immediately.
            if rng.gen_bool(0.7) {
                let host = rng.gen_range(0..n);
                if g.nodes[host].connections.len() < MAX_CONNECTIONS_PER_NODE {
                    let mut c = Connection::random(rng, n + 1);
                    c.target = n;
                    g.nodes[host].connections.push(c);
                    body = true;
                }
            }
        }

        // Connection parameters and pointers.
        let n_nodes = g.nodes.len();
        for node in g.nodes.iter_mut() {
            for c in node.connections.iter_mut() {
                if c.mutate(rng, pb * 3.0, n_nodes) {
                    body = true;
                }
            }
        }

        // Add / remove connections.
        for node in g.nodes.iter_mut() {
            if node.connections.len() < MAX_CONNECTIONS_PER_NODE && rng.gen_bool((pb * 1.5) as f64) {
                node.connections.push(Connection::random(rng, n_nodes));
                body = true;
            }
            if !node.connections.is_empty() && rng.gen_bool((pb * 1.5) as f64) {
                let i = rng.gen_range(0..node.connections.len());
                node.connections.remove(i);
                body = true;
            }
        }

        g.garbage_collect();
        (g, body)
    }

    /// Remove nodes unreachable from the root and remap connection targets.
    pub fn garbage_collect(&mut self) {
        let n = self.nodes.len();
        let mut reachable = vec![false; n];
        let mut stack = vec![self.root];
        while let Some(i) = stack.pop() {
            if reachable[i] {
                continue;
            }
            reachable[i] = true;
            for c in &self.nodes[i].connections {
                if c.target < n && !reachable[c.target] {
                    stack.push(c.target);
                }
            }
        }
        let mut remap = vec![usize::MAX; n];
        let mut next = 0;
        for i in 0..n {
            if reachable[i] {
                remap[i] = next;
                next += 1;
            }
        }
        let old = std::mem::take(&mut self.nodes);
        for (i, mut node) in old.into_iter().enumerate() {
            if !reachable[i] {
                continue;
            }
            node.connections.retain(|c| c.target < n && reachable[c.target]);
            for c in node.connections.iter_mut() {
                c.target = remap[c.target];
            }
            self.nodes.push(node);
        }
        self.root = remap[self.root];
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_and_mutate_keep_root_reachable() {
        let mut rng = StdRng::seed_from_u64(7);
        for seed in 0..50u64 {
            let g = Genome::random(seed);
            assert_eq!(g.root, 0);
            let mut child = g.clone();
            for _ in 0..20 {
                child = child.mutate(&mut rng).0;
                assert!(child.root < child.nodes.len());
                for n in &child.nodes {
                    for c in &n.connections {
                        assert!(c.target < child.nodes.len());
                    }
                }
            }
        }
    }

    #[test]
    fn json_roundtrip() {
        let g = Genome::random(3);
        let s = g.to_json();
        let g2 = Genome::from_json(&s).unwrap();
        assert_eq!(g, g2);
    }
}
