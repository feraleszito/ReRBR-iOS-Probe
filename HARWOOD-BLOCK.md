# Harwood Forest: first real block, stage 2

The separate **harwood-block** feature renders one geometry block near the LBS
start location. Default **triangle** and **rerbr-3d** (procedural cube plus stage-1
audit) remain separate configurations. With both optional features enabled,
harwood-block takes priority. No car, complete stage, automatic camera, physics,
audio, scenery lanes or game assets are included in this batch.

The owner reports that stage 1 passed on the physical iPad mini A17 Pro, including
282 blocks/cells, 442 objects, 38/38 required DDS references and adapter BC=true.
Subsequent owner report: stage 2 also passed physically on the same device, with
block 269, original geometry and BC1/BC2 textures. The expanded stage-3 selection
is separate: see [HARWOOD-NEIGHBORS.md](HARWOOD-NEIGHBORS.md).

## What was checked on Windows

The new CPU loader read the real installation without copying or modifying it.
It dynamically selected block **269** (zero-based) out of 282, using distance from
the start to each drawable block's bounds, then centre distance as a tie-breaker.
The number is a measurement, not a hard-coded choice.

```text
start = (-406.9473, 74.991806, 1447.8748)
vertices = 2236
near/always triangles = 1138
draws = 9
far-LOD draws omitted = 5
bad triangles / unsupported draws = 0 / 0
selected diffuse DDS references = 8
```

Texture IDs are 0, 1, 2, 3, 4, 6, 7 and 8. They include BR_07_ROADER materials,
BR_07_GROUND_B, BR_01_FELLING materials and TrackTexturePlateOpaque. The referenced
files are DXT1/BC1 and DXT3/BC2, four mips each, up to 1024 x 2048. Both remapped
archive reads and the CPU upload layout are checked. Windows did not execute a
Metal texture upload, create the native pipeline or render an image.

Run the CPU diagnostic from the standalone root:

```powershell
cargo run --locked --example harwood-block-diagnostic --no-default-features --features harwood-block -- 'C:\Richard Burns Rally\Maps'
$env:HARWOOD_MAPS = 'C:\Richard Burns Rally\Maps'
cargo test --locked -p rerbr-probe --no-default-features --features harwood-block --test harwood_block real_start_block_and_materials -- --ignored --nocapture
```

The opt-in real-data test checks bounds/framing, winding visibility, valid indices,
road/ground materials and the presence of both BC formats. Ordinary tests generate
synthetic data to check slot offsets, RGBA conversion, UVs, absolute indices/view
bases, LOD selection, reverse depth, framing and mip/block copy layouts. Naga validates
the new WGSL shader; host compilation checks the wgpu API use.

## Build and private data

The local workflow template at `github-actions/ios-probe.yml` adds a third manual
choice, **harwood-block**. The local build script accepts `PROBE_MODE=harwood-block`
and passes `--no-default-features --features harwood-block` to tests, Clippy and the
release static library. Existing mode flags, Xcode/lipo/signing and IPA packaging
are retained. Neither the parent workspace's active `.github/workflows/ios-probe.yml`
nor the separate GitHub Desktop repository was changed or synchronized for this step.
The template must be activated in the standalone repo in a later authorized sync.

The feature can also be built on a Mac with the current script:

```sh
PROBE_MODE=harwood-block bash scripts/build-ios.sh
```

Keep the same five private files from stage 1 at:

```text
On My iPad / ReRBRProbe / Harwood / Maps /
  track-20_O.lbs
  track-20_O.ini
  TextureFilenameMap.dat
  track-20_O_textures.rbz
  track-20_N_textures.rbz
```

No files need to be added to Xcode resources or Actions. Retain the full RBZs, since
some dry requests redirect to damp entries. Installation/signing uses the existing
process. Relaunch after transfer; no hot reload is implemented.

## Device behavior and logs

The selected mode requests TEXTURE_COMPRESSION_BC only when the adapter advertises
it. Other modes retain their previous device feature set. Missing BC, missing data,
unsupported layouts or invalid textures lead to a red background and a log entry.
The ordinary loading background stays visible while the CPU worker reads/converts.
GPU creation/upload and rendering stay on the existing main rendering thread.

Expected log phases in `Documents/rerbr-probe.log`:

1. `harwood-block DEVICE`: adapter BC and device BC enabled; both should be true
   on this device, but the new feature request still requires physical verification.
2. `READ PASS` / `CONVERT PASS`: source size, block, counts, bounds and fixed camera.
3. `DDS READ PASS`: request/remap, format, dimensions and stored mips of each image.
4. `PIPELINE validation PASS`: shader/pipeline creation accepted by wgpu.
5. `UPLOAD validation PASS`: queue texture writes accepted without scoped validation
   errors. This does not establish GPU completion or correct pixels.
6. `RENDER validation PASS`: indexed draw encoding, submission and presentation
   request accepted without the scoped validation error. Inspect the image physically.

Touches retain the background toggle and, in this mode only, cycle views:

- 0: textured baked vertex colour and the original two-diffuse blend.
- 1: vertex colour alone, to separate geometry from texture/material problems.
- 2: UV0 checker pattern, to inspect UV continuity and scale.

The camera is fixed, above/behind and to the side of the block using the start
heading. It fits the converted bounding-box corners through the unchanged original
left-handed/Y-up camera, horizontal FOV and reversed projection. The eye stays
fixed across rotation; only projection aspect changes. Front faces are clockwise,
back-face culling is enabled, depth is Depth32Float/Greater, cleared to 0. Colour
inputs use original unorm semantics; the shader compensates if the surface is sRGB.

Specular lighting, original negative mip bias and shadows are deferred. This is
only terrain/road geometry: no full forest, sky, props or animated objects yet.
Source provenance and adaptations are in `src/harwood/UPSTREAM.md`.

## Physical validation and remaining regression checks

The owner confirms a successful stage-2 Actions build and physical rendering of
block 269 with BC1/BC2 textures. This environment did not run Metal or produce a
device capture. After shared-code changes, compare this mode with the established
device result, including all three views, rotation, touch and suspend/resume.
The separate stage-3 region still requires its first Apple build and physical run.
