//! MyOpenMath (IMathAS) questions: the file format, static health checks, and the D.A.D engine's
//! wire types. Pure logic, no I/O; builds for wasm32. See `docs/dad/PLAN.md`.

pub mod author;
#[cfg(not(target_arch = "wasm32"))]
pub mod browserd;
#[cfg(not(target_arch = "wasm32"))]
pub mod chrome;
#[cfg(not(target_arch = "wasm32"))]
pub mod desktop_browser;
#[cfg(not(target_arch = "wasm32"))]
pub mod livecdp;
#[cfg(not(target_arch = "wasm32"))]
pub mod localengine;
pub mod engine;
pub mod form;
pub mod format;
#[cfg(not(target_arch = "wasm32"))]
pub mod harness;
pub mod health;
pub(crate) mod line_variant;
#[cfg(not(target_arch = "wasm32"))]
pub mod poster;
pub mod reference;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod student_text;
pub mod worksheet;
pub(crate) mod question_help;

pub use format::MomQuestion;
