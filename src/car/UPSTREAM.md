# Corolla adapter provenance

This app code is GPL-3.0-or-later, as the original reRBR source at
`C:\Users\adria\Downloads\rerbr-main\rerbr-main`. No upstream commit ID is
claimed: the source archive has no Git metadata. The reader copies and retained
license are documented in vendor/rerbr-asset-formats/UPSTREAM.md.

Original adapter references, identified by SHA-256:

| Original path under crates/ | SHA-256 |
| --- | --- |
| rbr-render/src/car/parts.rs | `b076c8ebdb027d04f3cb5aa4182aa11b345e0f1c767a859266d400ebb6c85920` |
| rbr-render/src/car/meshes.rs | `85c12e43bc911dcbcc348d63ea775c591080ee1428fe95b6fa3a7300ed27f3f8` |
| rbr-render/src/car/shading.rs | `13d4a2c61d1b660cf7f498eb54b3333d1386a56c8d7e5b9a9c8fb935c54ed158` |
| rbr-render/src/lights.rs | `6605dfd3596866651aa5fcb2ddb3a53892e2dab556408c595b42a1629dc74157` |

selection.rs adapts car/parts.rs::part_in_set/wheel_of/brake_disc_of and
lights.rs::shown_at_start. This mode explicitly chooses the original initial
light switches OFF and gravel wheels; dynamic lamps, interior, ghost, overlays,
shadow planes and other surface variants are excluded. All four original gravel
roots must be distinct; the RSF stacked-wheel repair is deliberately not imported.

mesh.rs adapts car/meshes.rs::build_meshes/placed_mesh: original face ranges,
absolute indices, UVs, matrix placement and inverse-transpose normals. Node worlds
come from the unchanged sgc::chained_worlds, after duplicate/cycle validation.
The unchanged imported coords::part_to_car and hinged_panel_to_car compose the
original transform blocks. Vertices are compacted into one shared buffer instead
of a full desktop Stage/damage representation. A draw retains its original wheel,
list and shader/material-rank order, including opaque draws following a blended
one. Indices are never reversed to hide a mirrored-part issue.

materials.rs adapts car/shading.rs::pass_for/list_for/draw_state. Blend, alpha
rejection, culling and depth-write decisions are preserved. The reader resolves
actual material diffuse paths through car_parts and shares each canonical DDS.
This first pass is unlit diffuse/alpha. Normals are retained, but original
CarPaint reflections/Fresnel, lighting, dirt/scrape, cracks and shadows are not
implemented. No DirectX bytecode is copied or needed. Missing required data fails
instead of generating a stand-in car, material or texture.

gpu.rs shares the existing BC1/BC2 uploader and terrain depth. The new WGSL uses
original UVs and texture alpha; the original car damage-alpha convention is not
misused as opacity. Body and four wheel model uniforms move the original meshes.

placement.rs and runtime.rs are new app integration. They retain the real
Harwood distance lookup, stage-4 clock/statistics/HUD, and reRBR chase parameters
(6m distance, 2.5m height, 0.5m target height, 65-degree horizontal FOV). Ground
support, bounded visual wheel travel and camera visibility queries are geometric
presentation, not NGP/vehicle/suspension simulation. A/B keeps the same prepared
geometry, path, camera and resources, suppressing only car GPU draws in A.

No proprietary files, extracted geometry, textures or commercial INI contents
are included. Runtime private files are read only from Documents/RBR and the
existing Documents/Harwood/Maps in the application, or the explicit diagnostic
installation argument on a host.

