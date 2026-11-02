//! evo_core: planet-aware evolution of articulated virtual creatures.
//!
//! * `planet`  — planet dials and derived physics/ecology quantities
//! * `genome`  — Sims-style directed-graph genome with membrane/pad nodes
//! * `decode`  — graph -> rigid part list with masses and joints
//! * `sim`     — rapier simulation with fluid forces, buoyancy, O2 fatigue
//! * `evolve`  — MAP-Elites archive with innovation protection

pub mod decode;
pub mod eco;
pub mod evolve;
pub mod genome;
pub mod planet;
pub mod plant;
pub mod sim;
pub mod terrain;

pub use decode::{decode, Part, Phenotype};
pub use evolve::{EvolveConfig, Evolver, GenSummary};
pub use genome::{Genome, JointKind, NodeKind};
pub use planet::{Derived, PlanetParams};
pub use sim::{CreatureSim, Environment, EpisodeResult, SimConfig};
