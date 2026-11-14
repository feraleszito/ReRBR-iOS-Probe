# Upstream source and scope

This first integration batch imports coordinate and camera mathematics from ReRBR's
`rbr-render` crate, licensed **GPL-3.0-or-later** in its package manifest. The full GPLv3
license text is included in `LICENSE`. The source repository named by ReRBR's development
guide is <https://github.com/JodusNodus/rerbr>.

The source snapshot is the owner's local `rerbr-main` checkout, inspected on 2026-11-14.
That checkout has no `.git` metadata and Git is unavailable in the editing environment,
so no commit ID is claimed. The SHA-256 values below identify the imported bytes exactly.
Paths in the middle column are relative to that original checkout, not dependencies on
folders outside this standalone repository.

| Local file | Original source | SHA-256 |
| --- | --- | --- |
| src/coords.rs | crates/rbr-render/src/coords.rs | `652915c9da835806f0beb378e44f46c58f24234dfeb29145e4417e10f0f3d9c2` |
| src/camera.rs | crates/rbr-render/src/camera.rs | `f760a3064b21bbbe2ce3f5f7773fcfe9bda4d481c7f2884881a021b4b265b92f` |
| src/triple.rs | crates/rbr-render/src/triple.rs | `e6874bc61ac9884d7eb15b3783853eb30f015122ef321db37fe7af7c3d0be1be` |
| LICENSE | LICENSE | `3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986` |
| rustfmt.toml | rustfmt.toml | `93ed1668bb2ca659e88711456b72320bf2f53369ecf278c4a61af0cb63cc3a1e` |

All five files are copied byte-for-byte, including the original Rust unit tests. The
source files retain their original comments, identifiers and numeric mathematical test
cases. No game assets, binaries or extracted asset files are included.

`triple.rs` is included because `camera::Lens::views` uses its pure-math `Rig` and
`Screen` types. This does not create multiple windows or enable multiple displays.
The coordinate functions with physics-related names only transform vectors/matrices;
they do not import a physics crate or run a simulation.

The original comments refer to other modules and notes in the complete ReRBR project.
Those references do not imply that these systems are included here. The new `src/lib.rs`
allows the unused internal stage-bound helper and unresolved documentation link on the
`coords` module only, preserving the upstream file rather than changing it to fit a
smaller crate. No upstream implementation or tests were changed.

New integration files are `Cargo.toml` and `src/lib.rs`, also GPL-3.0-or-later. Runtime
dependencies are pinned `glam 0.33.12` and `wgpu 30.0.1`; wgpu is used for the public
front-face enum and no GPU resources are created by this crate. The host app enables
Metal and owns its event loop, surface and rendering. Additional integration tests in
the host's `tests/core_math.rs` verify reverse depth and orbit projection in portrait
and landscape through the optional dependency.

There is no vertex format import, geometry generator, draw pass, new shader, stage
loader, audio, physics or launcher in this batch. The `rerbr-3d` feature only enables
this library; the existing triangle renderer remains unchanged, even with it enabled.
