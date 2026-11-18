//! DraftCraft's 3D solids: a thin facade over solidcore, a pure-Rust B-rep kernel (a port of
//! NVIDIA's SMLib, Apache-2.0).
//!
//! This is the only crate that names the kernel (`solidcore-ops`, `solidcore-record`,
//! `solidcore-tess`), so the kernel can be pinned, patched or replaced without touching the
//! document model or the engine. Vectors are converted between `draftcraft-geom` and the
//! kernel's here and nowhere else.
//! Built without the `kernel` feature the crate is empty (docs/solids.md).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

#[cfg(feature = "kernel")]
pub mod acis;
#[cfg(feature = "kernel")]
mod body;
#[cfg(feature = "kernel")]
mod views;

#[cfg(feature = "kernel")]
pub use body::*;
#[cfg(feature = "kernel")]
pub use views::{MAX_VIEW_BODIES, Seen, View, ViewPiece, project, section};

#[cfg(all(test, feature = "kernel"))]
mod tests;
