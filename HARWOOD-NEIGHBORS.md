# Harwood Forest: connected terrain, stage 3

`harwood-neighbors` is an independent compile-time mode. The default is still
triangle; `rerbr-3d` and `harwood-block` remain separate fallback builds. The owner
validated stage 2 (block 269 and BC1/BC2) on the iPad mini A17 Pro. Stage 3 has
only been compiled/tested on Windows; Metal, the Apple target and its image are
pending. No game files are included in the source tree or IPA.

## Selection and rendering

1. The background worker reads `Documents/Harwood/Maps`, using the existing
   imported LBS/INI/filename-map/RBZ/DDS readers. Keep the same five private files
   documented in [HARWOOD-BLOCK.md](HARWOOD-BLOCK.md); no new assets are needed.
2. The existing start/AABB selection chooses the anchor dynamically (269 in this
   installation). The unchanged reRBR-derived mesh converter processes near/always
   terrain lists. Unsupported/invalid blocks are excluded and reported.
3. A spatial graph matches actual triangle boundary segments, not block numbers
   or overlapping boxes. Segments must be at least 0.25 m long; endpoint
   quantization narrows the search and an actual endpoint-distance check requires
   at most 0.02 m residual. Vertices are never welded or moved.
4. Original LBS driveline samples, from 100 m before to 600 m after the sample
   nearest the start, guide connected frontier growth. Road materials are detected
   from INI names containing `road`. Coverage tests actual road triangles in XZ
   with 3 m edge tolerance and 4 m height tolerance. Uncovered samples take priority
   over extra terrain. After coverage, nearby connected terrain fills the target
   of 20 blocks. IDs are only a final tie-breaker after spatial scores.
5. Budgets cap the region at 400 draws, 16 MiB of vertex/index payload and 32 MiB of
   stored compressed mip payload. No requested set can exceed 25 blocks. Mesh/draw
   limits restrict frontier growth; a texture overrun removes the last growth leaf
   until it fits, preserving connectivity. A seed that cannot fit returns an error.
   Fewer than 15 blocks and every budget reduction are explicitly reported.
6. Diffuse requests are deduplicated by normalized resolved archive/DDS path before
   decompression/retention. Logical aliases map to one texture ID; every draw uses
   that canonical ID and the shared GPU pass uploads each DDS only once. No atlas,
   recompression, extracted files or new dependency is needed.
7. Selected meshes are concatenated with vertex-index and draw-range offsets.
   World positions, winding, original colours and both UV sets are unchanged.
   The existing camera frames the combined bounds; original left-handed/Y-up
   coordinates, clockwise faces, back culling and reversed depth
   (`Depth32Float`, `Greater`, clear 0) remain in use.

The existing touch behavior cycles textures, vertex colours and UV checker. There
are no new controls. BC is requested only if advertised, as in `harwood-block`.
No scenery lanes, trees, objects, car, sky, physics or sound are loaded for drawing.

## Actual read-only Windows result

The original installation was read directly, with no extracted resources saved.
The selected IDs are zero-based and listed in connected growth order:

```text
269, 260, 252, 261, 262, 253, 254, 263, 264, 255,
270, 274, 268, 275, 259, 273, 251, 258, 267, 250
```

| Measurement | Result |
| --- | --- |
| Selected / connected source component | 20 / 282 blocks |
| Vertices / triangles | 18,983 / 9,539 |
| Indexed draws per frame | 124 |
| Material bindings / logical diffuse IDs / unique resolved DDS | 248 / 8 / 8 |
| Vertex + index payload | 721,924 bytes |
| Unique stored BC1/BC2 mip payload | 1,827,840 bytes |
| Combined estimated GPU payload | 2,549,764 bytes (about 2.43 MiB) |
| Source LBS / all converted VB+IB payload | 28,014,518 / 7,164,920 bytes |
| Sampled road coverage | 71/71, one run from 0 to 588.12 m relative to nearest start sample |
| Unmatched road samples / internal sampled gaps | 0 / 0 |
| Spanning-tree connections | 19; all have exactly matching endpoints (0 m residual) |

No budget reduction was necessary. Multiple blocks reference the same eight DDS:
248 material slots result in eight image allocations, not 248. A separate real-data
alias test requests IDs 0, 12, 22, 23 and retains two resolved DDS, exercising aliases
that are not needed in this particular region.

World bounds are approximately `(-573.82, 66.34, 1210.47)` to
`(91.70, 88.89, 1647.22)` m. The fixed panoramic eye is approximately
`(-1096.98, 1205.61, 2226.34)`, aimed at `(-241.06, 77.61, 1428.85)`.
All indexed vertices project inside the frustum at tested aspects 0.65, 0.75,
1.55 and 2.0. This validates framing mathematics, not actual visibility/pixels.

Memory figures are payload estimates, not measured resident/peak memory. They
exclude allocation alignment, driver state, bind groups, uniform/fallback pixels,
surface images and depth. Depth is logged on allocation/resize as width*height*4.
The CPU temporarily holds parsed structures, converted meshes, road triangles and
the connectivity graph; those overheads are not measured. The LBS buffer is released
before texture loading. Textures are budgeted before GPU upload, but the worker
initially reads the selected texture set before trimming, so that limit is not a
hard CPU peak-memory cap.

## Build and Windows verification

From the standalone repository root (the contents of local `ios-probe`):

```powershell
cargo run --locked --example harwood-neighbors-diagnostic --no-default-features --features harwood-neighbors -- 'C:\Richard Burns Rally\Maps'
cargo test --locked --workspace --no-default-features --features harwood-neighbors
$env:HARWOOD_MAPS = 'C:\Richard Burns Rally\Maps'
cargo test --locked --workspace --no-default-features --features harwood-neighbors -- --ignored --nocapture
cargo clippy --locked --workspace --all-targets --no-default-features --features harwood-neighbors -- -D warnings
cargo fmt --all -- --check
```

Normal CI tests use synthetic geometry and need no private files. The two
explicitly ignored tests exercise the real connected region and resolved DDS
aliases. See [VALIDATION.md](VALIDATION.md) for the actual offline Windows checks.

The local template `github-actions/ios-probe.yml` adds `harwood-neighbors` to
`build_mode`; the script passes that exact feature to host tests, iOS Clippy and the
release static-library build. On macOS the equivalent is:

```sh
PROBE_MODE=harwood-neighbors bash scripts/build-ios.sh
```

All Cargo/Xcode paths, ARM64/lipo checks and unsigned IPA generation are retained.
The active workflow outside this directory and the GitHub Desktop clone were not
edited. A later authorized synchronization must adapt the active workflow before
this choice is available in Actions. No synchronization or Actions run was done.

## Device checks and limitations

The UI continues drawing a loading background during CPU work. Missing files,
unavailable BC, conversion errors or scoped GPU validation errors produce a red
background and a diagnostic; no retry loop or synchronous file load is added.
Existing GPU creation/upload runs once on the rendering thread and is not yet timed
on the iPad. A larger upload can still cause a temporary frame stall.

Logs use the `harwood-neighbors` prefix: DEVICE, READ PASS, CONVERT PASS, SELECT,
CONNECT, GEOMETRY, TEXTURES, MEMORY, ROAD, BOUNDS, CAMERA, PIPELINE, UPLOAD and
RENDER. Inspect every WARNING. Upload/render validation acceptance does not establish
GPU completion or correct pixels.

First run Actions with the new choice, install using the existing signing process,
and retain the five files under Documents. On the iPad verify BC enabled, the 20
IDs/counts, all DDS uploads and first-frame validation. Capture the panoramic road
and terrain in all three views and both orientations; inspect seams, orientation,
UVs, materials, reverse depth and stability after suspend/resume. Recheck the three
fallback builds. Measure frame time and resident memory before expanding further.

Connectivity is conservative: exact segment pairing can miss split/T-junction
boundaries or points in adjacent quantization cells. The measured source graph
connects all 282 blocks, but that is not a watertight-mesh proof. Road coverage is
sampled with tolerances; it cannot prove every metre of road or every terrain seam
is continuous. Outer boundaries deliberately remain open. There are no visibility
cells, frustum/occlusion culling or draw batching yet: all 124 selected draws are
submitted each frame. Visual continuity, panorama readability, performance and
total memory remain unverified physically for stage 3.
