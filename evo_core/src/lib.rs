//! evo_core: planet-aware evolution of articulated virtual creatures.
//!
//! * `planet`  — planet dials and derived physics/ecology quantities
//! * `genome`  — Sims-style directed-graph genome with continuous form genes
//! * `decode`  — graph -> rigid part list with masses and joints
//! * `plant`   — the same graph read as a plant body
//! * `sim`     — rapier simulation with fluid forces, buoyancy, O2 fatigue
//! * `terrain` — a sphere of cells with elevation, climate and tide
//! * `eco`     — the planet ecology: no score, energy earned and paid
//!
//! There is one mode. Organisms are never scored on a task; the physics
//! only measures what a body can do and the ecology decides what that is
//! worth.

pub mod arena;
pub mod decode;
pub mod eco;
pub mod genome;
pub mod planet;
pub mod plant;
pub mod sim;
pub mod terrain;

pub use decode::{decode, Part, Phenotype};
pub use genome::{Genome, JointKind, NodeKind};
pub use planet::{Derived, PlanetParams};
pub use sim::{CreatureSim, Environment, EpisodeResult, SimConfig};
