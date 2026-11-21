//! MyOpenMath (IMathAS) questions: the file format, static health checks, and the D.A.D engine's
//! wire types. Pure logic, no I/O; builds for wasm32. See `docs/dad/PLAN.md`.

pub mod author;
#[cfg(not(target_arch = "wasm32"))]
pub mod browserd;
#[cfg(not(target_arch = "wasm32"))]
pub mod chrome;
#[cfg(not(target_arch = "wasm32"))]
pub mod livecdp;
pub mod engine;
pub mod form;
pub mod format;
#[cfg(not(target_arch = "wasm32"))]
pub mod harness;
pub mod health;
#[cfg(not(target_arch = "wasm32"))]
pub mod poster;
pub mod reference;

pub use format::MomQuestion;
