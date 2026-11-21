//! MyOpenMath (IMathAS) questions: the file format, static health checks, and the D.A.D engine's
//! wire types. Pure logic, no I/O; builds for wasm32. See `docs/dad/PLAN.md`.

pub mod engine;
pub mod format;
pub mod health;

pub use format::MomQuestion;
