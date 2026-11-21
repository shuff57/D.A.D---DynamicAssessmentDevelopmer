//! MyOpenMath (IMathAS) questions: the file format, static health checks, and the D.A.D engine's
//! wire types. Pure logic, no I/O; builds for wasm32. See `docs/dad/PLAN.md`.

pub mod author;
pub mod engine;
pub mod format;
#[cfg(not(target_arch = "wasm32"))]
pub mod harness;
pub mod health;

pub use format::MomQuestion;
