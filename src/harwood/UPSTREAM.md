# Stage-2 rendering provenance

All new integration code is GPL-3.0-or-later, as is the host package. The complete
license is retained in both existing vendor libraries. No proprietary data or shader
binaries are included.

Source: the owner's local ReRBR snapshot, inspected 2026-11-15; repository named
by its development guide: <https://github.com/JodusNodus/rerbr>. The source checkout
has no Git metadata, so no commit ID is claimed. Original source hashes:

| Source | SHA-256 |
| --- | --- |
| crates/rbr-render/src/stage/mesh.rs | `3d62e147b4d0f3cf7c7e2e368c2d20dd638700cea28f34e4b7f1711cbfcde2f9` |
| crates/rbr-render/src/renderer/textures.rs | `7f75df588c293db75473a40fe4520d6f3b5d2e3e05343ad339a56797230585ea` |
| crates/rbr-render/src/coords.rs | `652915c9da835806f0beb378e44f46c58f24234dfeb29145e4417e10f0f3d9c2` |

`mesh.rs` is an **adaptation**, not a byte-identical import, of slot_layout,
block_mesh, repack, texcoord, vertex_bytes and f32_at in stage/mesh.rs. It retains
the original slot offsets, per-view vertex bases, absolute u16 source indices,
start_index ranges, bad-triangle checks, D3DCOLOR-to-RGBA conversion, UV0/UV1 and
unflipped winding. Output uses u32 indices as the original conversion does.

The adapted types carry only position, colour, two UV sets and diffuse draw
references. This close view selects lists 1/2, excluding list 3 instead of drawing
both LODs. Unsupported slots or shadow draws are reported as load failures; no
geometry is silently rendered through an invented vertex layout. Specular draw
flags are counted, but only diffuse is drawn. Normals, specular/shadow UVs,
lighting, occlusion, wetness, grass and live state are not carried over.

`texture.rs` and gpu.rs::upload adapt the BC upload portion of dds_texture in
renderer/textures.rs: unorm BC1/BC2 formats, mip-level data, ceil-to-block row sizes
and block-rounded copy extents. Stage 2 rejects unsupported formats/base alignment
instead of importing the full desktop texture-conversion stack. It checks the
device limit and uploads every stored mip with Queue::write_texture. Ordinary
trilinear/wrap sampling is used; the original list-1 negative mip bias is deferred.

The start heading calculation in mod.rs follows stage/mesh.rs::start_pose.
The block selector, fixed framing, worker/channel, GPU error handling and view
diagnostics are new app integration. They call the unchanged imported FreeFly
camera and coordinate functions instead of duplicating view/projection maths.

shader.wgsl is a new small shader, using the original two-diffuse/baked-colour
formula documented in notes/formats.md. It adds colour/UV diagnostic views and
cancels automatic sRGB surface encoding to retain the original unorm colour
convention. It is not the complete stage shader and does not include game VSO/PSO.

Original engine and existing vendor files are unchanged. Reader provenance remains
in vendor/rerbr-asset-formats/UPSTREAM.md and camera provenance in
vendor/rerbr-render-core/UPSTREAM.md. The shared app asset reader now exposes a
selected-diffuse byte path so the audit and the visual mode use the same mapping
and archive/CRC rules; stage-1 requests and required/unused semantics are retained.

Stage 3 adds app-owned neighbors.rs selection and tests. It calls the same
unchanged block converter for each selected LBS geometry block. Connectivity is
measured on triangle boundary edges in original world coordinates; the LBS's
already-parsed DrivelinePoints guide static selection and coverage diagnostics.
No physics, object-lane or tree renderer is added. Merging offsets only vertex
indices and draw ranges, never positions, colours or UVs. The unchanged WGSL,
BC upload and reverse-depth pipeline render the resulting mesh.

The shared asset reader deduplicates normalized resolved archive paths before
inflating/retaining DDS data in neighbor mode. Logical texture IDs are mapped to
canonical IDs in draw records, so the existing GPU texture map uploads each
resolved DDS once. The previous single-block loader retains its non-aliasing
return contract. Existing vendor files and original engine files remain unchanged.
