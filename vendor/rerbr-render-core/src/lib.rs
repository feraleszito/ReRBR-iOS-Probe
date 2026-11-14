//! Original reRBR camera and coordinate mathematics, imported without game systems.
//!
//! See `UPSTREAM.md` for source paths, hashes and licensing. This batch provides no
//! scene geometry, draw pass, asset loader, physics, audio or platform event loop.

pub mod camera;
// The complete upstream file retains an internal stage-bound helper not used in this
// batch and a documentation link to the original crate's playback module.
#[allow(dead_code, rustdoc::broken_intra_doc_links)]
pub mod coords;
// Camera::Lens exposes this pure-math type; keeping the original module avoids stubs.
pub mod triple;

pub use glam::{Mat4, Vec3};
