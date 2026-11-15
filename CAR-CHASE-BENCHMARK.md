# Original Toyota Corolla: chase benchmark, stage 5

`car-chase-benchmark` is an independent visual mode, sharing the existing terrain,
BC uploader, reverse-Z depth, clock, statistics and HUD. The owner confirms stage 4
ran on the iPad mini A17 Pro at 60 FPS with V-sync. This new car path has Windows
CPU/compiler checks only: no Apple build, Metal execution or car screenshot is
claimed. All five previous modes remain independently selectable; triangle stays
default. If several features are enabled, car chase takes priority.

## Private inputs: eight car files, no bundled resources

The application reads these files under Foundation's private Documents directory:

```text
Documents/RBR/
  Cars/Cars.ini
  Cars/COROLLA/corolla.sgc
  Cars/COROLLA/corolla.ini
  Cars/COROLLA/corolla_shaders.ini
  Cars/COROLLA/corolla_shader_settings_clear.ini
  Cars/COROLLA/textures/corolla_external.dds
  Cars/COROLLA/textures/corolla_external_dynamic.dds
  Cars/COROLLA/textures/corolla_external_transparent.dds
```

Harwood retains its existing five files at `Documents/Harwood/Maps`. Nothing moves
into the repository or Xcode resources. The car reader accepts only the eight
paths above, resolves DOS separators/case, checks its canonical private root and
bounded file sizes, and requires [Car06] to name the original Corolla. A missing
file, unmatched face/material or extra required resource is an explicit error;
there is no stand-in car, missing-texture substitution or installation-path fallback.

The real Windows loader parsed the original 2,029,757-byte SGC (636 decoded records)
and resolved exactly those eight files. Its three DDS contain BC2 1024x1024/11 mips,
BC1 512x256/10 mips and BC2 512x512/10 mips respectively. Every stored mip passes the
existing CPU upload plan. The earlier audit matched the car folder to the project's
retail SHA-256 manifest. No DirectX shader, physics bank, NGP DLL, sound, skin pack,
light configuration or other commercial resource is needed for this diffuse pass.

To transfer after a future authorized build:

1. Install the feature IPA using the signing/install procedure already used for
   the probe. This Windows session does not create or install an IPA.
2. Through the app's existing Documents file-sharing access, create `RBR/Cars/`
   and `RBR/Cars/COROLLA/textures/` inside **On My iPad / ReRBRProbe**.
3. Copy only the eight original files listed above from the same relative paths
   below `C:\Richard Burns Rally`. Preserve their names and subfolders. Do not copy
   them to GitHub Desktop, a public repo, Actions artifacts or the app bundle.
4. Keep the Harwood files in their existing separate folder, then relaunch. Missing
   files are identified in `Documents/rerbr-probe.log`. No asset hot reload is added.

## Actual geometry and materials

| Category | Referenced vertices | Triangles | Indexed draws | Unique DDS used |
| --- | ---: | ---: | ---: | ---: |
| Existing 20-block terrain | 18,983 | 9,539 | 124 | 8 |
| Car opaque states | 10,851 | 10,756 | 83 | 2 |
| Car blended states | 374 | 410 | 16 | 1 |
| Car total | 11,225 | 11,166 | 99 | 3 |
| HUD | Actual glyph vertices; previous frame shown on HUD | Vertices / 3 | 1 | 1 generated atlas |

The car has 75 converted exterior meshes, 47 skipped meshes and 16 distinct
shader/material pairs. Four original gravel wheel roots are required. Selection
excludes interior, ghost, shadow planes, overlays, unused wheel/spare-wheel surface
variants, the loader's placeholder/unbuilt parts and hidden initial switches.
Lights remain in their original OFF initial state; no lighting animation is added.

The original SGC face ranges, absolute indices, UVs, parent matrices and transform
blocks are used. Duplicate IDs/cycles are rejected before chaining. Normals use
inverse transpose, wheel pivots are retained, and the original side-door rest
transform is applied. Neither indices nor world axes are flipped to mask errors.
The car faces model -Z, with Y up; its movement is a proper rigid transform.

Rendering preserves original wheel/list/shader rank, including opaque draws that
follow an earlier blended one. Per-material states retain opacity selection,
alpha rejection (>0), culling and depth writes. For example, transparent CarPlastic
is blended, two-sided and depth-writing; glass with no opacity channel is opaque.
The terrain depth is loaded, not cleared, by the car pass. The HUD is last.

The first shader is **unlit original diffuse/alpha**. Normals are converted and
uploaded, but reflections/Fresnel, full original lighting, dirt, scrape, damage,
shadows, mirrors and headlamps are deferred. This is authentic model/texture data
with a reduced presentation shader, not complete original CarPaint fidelity.
The original damage-alpha value is not multiplied into window transparency.

The car requests 493,192 bytes of vertex/index payload and 1,835,080 bytes of
stored BC mip payload (about 2.22 MiB combined), excluding state/alignment/driver
overhead. Existing terrain payload is about 2.43 MiB. Surface/depth buffers, HUD,
uniforms and CPU parse/ground tables add memory; no measured resident-memory claim
is made. DDS are allocated once per canonical path and shared across materials.

## Motion, support and camera

The route remains the original 588.53m distance lookup and the stage-4 20-second
monotonic-clock session, after two seconds of warmup. No fixed-frame stop, car
physics, driving input, sound, steering or wheel-spin animation is introduced.

The car uses a separate road pose, never the old camera's +1.4m eye height. Its
wheel-contact centroid follows the route XZ. A plane from four support locations
sets body pitch/roll; low body vertices are checked against actual terrain.
Each wheel/brake group gets bounded vertical presentation travel so its original
vertices remain above the mesh with a nominal 5mm contact margin. Travel over
0.25m fails explicitly. There are no springs, forces or suspension dynamics.

Real-file verification sampled 1,179 poses at 0.5m intervals. Minimum computed
wheel-vertex clearance was about 0.00499m and maximum visual travel about 0.06561m.
The same support checks repeat for every rendered pose. Rigid links/body panels
do not articulate with the wheels. Tests cover vertex support; this is not a full
swept-volume or triangle/triangle vehicle collision solver.

The chase uses reRBR's 6m boom, nominal 2.5m road-relative height, 0.5m target height
and 65-degree horizontal FOV. Actual ground support, segment visibility and four
near-plane corners are checked. A raised/shortened boom (down to 3.5m) is attempted
before rejecting an unsafe view. All measured poses kept the full 6m boom.
Only loaded terrain is tested, not trees/objects that are not in this experiment.

Car support uses cached triangle planes and a 2m candidate grid; the original
route keeps its 8m grid and height calculations. Camera segments use conservative
XZ bounds and exact triangle intersection tests. No sampled heightfield replaces
the original mesh. CPU probe work is included in frame preparation timing.

An early framing test caught a small wide-landscape clip with the old 50-degree
driver FOV. The dedicated chase projection fixes that without changing the old
modes. The real test checks all drawn car vertices in 0.65/1.55/2.0 aspect frusta
at 41 route positions. Physical clipping, transparency and appearance still need
inspection in portrait and landscape.

## HUD, restart and A/B

Initial run is **B: terrain and Corolla**. Tap during a run/warmup/abort to restart
the same selection. After a completed run, tap to switch to **A: terrain only** and
restart. Another completion/tap returns to B. The loaded car, terrain, textures,
support queries, route, FOV and camera remain identical; only car GPU draws are
suppressed in A. There is no reload or resource reinitialization.

HUD rows TRN/OPA/TRA report vertices (V), triangles (T), draws (D) and unique DDS
used (X). A reports zero car submissions while retaining the allocated car assets.
HUD counts describe its previous frame because its own text changes geometry;
the final log records the actual last timed HUD submission. Texture counts are
per-category usage, not a sum of all driver allocations. Font/depth/surface and
the existing fallback pixels are not original DDS.

Stage-4 cadence metrics are retained: average FPS, minimum/maximum full one-second
window FPS, mean/P95/P99 frametimes, total frames, actual duration/stop overshoot,
and separate CPU preparation mean/P95. GPU timing remains explicitly unavailable.
Host present requests are not confirmed scanout. FIFO/V-sync and the HUD remain
part of the workload; 60 FPS does not establish GPU utilization or remaining margin.
The HUD's text/counters differ slightly between A/B and contribute their own cost.

## Build, checks and pending work

From the experiment/standalone root:

```powershell
cargo run --locked --example car-chase-diagnostic --no-default-features --features car-chase-benchmark -- 'C:\Richard Burns Rally'
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
$env:RBR_CAR_DATA = 'C:\Richard Burns Rally'
$env:HARWOOD_MAPS = 'C:\Richard Burns Rally\Maps'
cargo test --locked --workspace --all-features -- --ignored --nocapture
```

Normal CI uses synthetic data. Real data is opt-in and never copied as fixtures.
Reader copies retain original unit tests, license and hashes; adapter sources and
intentional differences are recorded in `src/car/UPSTREAM.md`.

The local script/template add `car-chase-benchmark`. On macOS use
`PROBE_MODE=car-chase-benchmark bash scripts/build-ios.sh`. The script passes the
feature to host tests, device Clippy and the release ARM64 static library. Cargo
paths, Xcode, lipo, signing and IPA packaging are retained. The source/IPA verifier
now also rejects commercial INI and DirectX shader files. The active GitHub clone
workflow has not been edited or synchronized; no Actions run was started.

Still required: first Apple build and IPA checks, physical BC upload/car image,
wheel and camera inspection, all material states, live counters and repeated A/B,
interruptions/rotation, CPU probe cost, frame pacing, temperature and resident
memory. Windows type checks and Naga validation do not establish Metal success.
