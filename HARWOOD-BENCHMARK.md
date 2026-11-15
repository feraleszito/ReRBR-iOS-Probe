# Harwood Forest: automatic graphics benchmark, stage 4

`harwood-benchmark` is a separate visual mode that reuses `harwood-neighbors`.
The owner confirms stages 3 and 4 rendered on the iPad mini A17 Pro; stage 4
reported 60 FPS average with V-sync. That does not establish GPU utilization or
timing. The new original-car mode is separate and still awaits Apple/device
validation: see [CAR-CHASE-BENCHMARK.md](CAR-CHASE-BENCHMARK.md).

## Workload and controls

The same private five files are read from `Documents/Harwood/Maps`, via the existing
worker and original imported readers. See [HARWOOD-BLOCK.md](HARWOOD-BLOCK.md) for
their filenames and transfer. The application has no alternate installation path,
asset download or bundled game resource. The Windows diagnostic takes an explicit
owner-supplied Maps path for read-only testing. No source vertices, route arrays,
textures, archive entries or other resource contents are written to logs or disk.
Logs contain aggregate diagnostics and the existing asset identifiers/metadata.

Selection, materials and BC upload are unchanged: the measured installation loads
the same 20 blocks, 18,983 vertices, 9,539 triangles, 124 terrain draws and eight
shared DDS. The benchmark requires the complete 20-block workload and covered LBS
sample window; it refuses a reduced/unsupported set. All terrain draws are submitted
each frame, without visibility culling. The original diffuse shader, left-handed
Y-up world, clockwise faces, depth testing and reverse-Z remain unchanged.

The new HUD adds one alpha-overlay draw, an original generated 5x7 ASCII atlas and
a bounded vertex buffer. It uses no font files or new dependencies. Its requested
font/vertex payload is 3,072 + 163,840 bytes, excluding driver/state overhead. Route
tables and CPU triangle/grid structures also add memory; resident/peak memory is
not measured. The HUD and camera/ground queries are included in the timed workload.

After loading/uploading, the app automatically holds the start camera for a
**two-second warmup**, then runs for **20 seconds of monotonic real time**.

- Tap anywhere to discard the current/result metrics and restart warmup and the
  benchmark, using cached geometry/textures. This also works after an interruption.
- Running uses textured mode throughout. Existing diagnostic-view/background touch
  controls remain unchanged in `harwood-block` and `harwood-neighbors` builds.
- The HUD shows state, rolling FPS, elapsed time, distance and last CPU preparation
  time. After completion it keeps a final summary and stops camera movement.
- Suspension, occlusion or resolution changes invalidate an active/warming run;
  tap after returning/rotating to start a fresh run. There is no hidden pause that
  turns an interrupted run into a supposedly continuous 20-second result.
- Missing/invalid assets, unsupported BC, camera-ground failure or scoped GPU
  errors show an error/log instead of a fabricated result. File/GPU errors require
  relaunch after correcting the cause; this step adds no asset hot reload.

`triangle` remains default. Each of `rerbr-3d`, `harwood-block` and
`harwood-neighbors` remains independently selectable. With multiple features,
priority is benchmark, neighbors, block, procedural 3D, then triangle.

## Real route and camera safety

The route is the existing LBS driveline window used to guide stage-3 selection.
No TRK file, artificial waypoint set, physics or vehicle is introduced. Windows
read 71 original points and validated an interpolated camera path of about
**588.53 m**, giving **29.43 m/s** for the 20-second run.

Catmull-Rom interpolation makes the horizontal position curve continuous with
continuous tangents at its knots. An arc-length lookup sampled at approximately
0.25 m removes the original points' uneven spacing. Distance is
`length * clamp(elapsed / 20, 0, 1)`, independent of frame count. Constant speed
is numerical to that lookup's resolution, not an exact analytic arc-length solve.
Heading comes from a continuous chord from 2 m behind to 12 m ahead, following
the road's direction/grade without Euler-angle wrapping or frame-rate-dependent
filtering. The existing original camera mathematics produces the matrix.

Ground height uses actual triangle-plane intersections with a vertical ray at the
camera XZ, indexed by an 8 m CPU grid. It checks both selected road-material
triangles and the highest selected terrain/road triangle. This is stricter than
the stage-3 road coverage test: there is no 3 m edge tolerance for camera support,
only tiny barycentric tolerance at shared triangle edges. A second surface more
than 0.75 m above the road is considered ambiguous and rejected.

Elevation is smoothed over a short symmetric neighborhood, interpolated, then
clamped each rendered frame to at least **1.4 m above the highest actual ground**.
The loading worker validates the interpolation densely, including checks every
0.1 m of distance. Each live camera query repeats exact ground/road support tests;
a missed surface aborts the mode instead of interpolating over an unsupported gap.
Original-point gaps over 30 m, invalid points and paths shorter than 50 m are
rejected. A curve leaving the road is rejected; there is no alternate invented route.

The real-file Windows check measured 1.400..1.432 m eye clearance over the loaded
ground along this path. That is CPU geometric evidence, not an executed Metal view.

This ensures point-eye clearance at every rendered pose. It is not a vehicle
collision solver or proof that the entire near plane can never clip a steep face.
Mesh facets and the safety clamp can still cause small vertical derivative changes.
The road, corners, elevation, seams and endpoint view need physical inspection.
The 20-block boundary remains visible; there are no trees, props, sky, car or physics.

## Timing definitions

The original post-frame 16.7 ms scheduler delay is disabled **only for benchmark**.
Winit requests redraws with `ControlFlow::Poll`; the existing FIFO presentation and
UIKit/Metal pace drawing. Thus V-sync, OS scheduling, acquisition/backpressure and
CPU work can all limit the result. FPS is not GPU utilization or uncapped throughput.

The measurement starts just before preparing the first timed scene frame, after
drawable acquisition and warmup. Completion timestamps are taken after encoder
submission and the presentation request return. These are host timestamps, not
confirmed display scanout or GPU-completion timestamps.

| Metric | Definition |
| --- | --- |
| Total frames | Successfully submitted/presentation-requested scene frames in the active run |
| Actual duration | Last completion timestamp minus timed-run start |
| Average FPS | Total frames divided by actual duration |
| Current FPS | Completion count in the last one-second rolling window; shorter elapsed window at startup |
| Minimum/maximum FPS | Lowest/highest completion count in the 20 fixed full one-second windows `(0,1]` through `(19,20]` |
| Mean frametime | Mean interval between completion timestamps; the first interval starts at timed-run start |
| Frametime P95/P99 | Nearest-rank percentiles of those intervals; stalls are retained |
| CPU preparation mean/P95 | CPU elapsed after drawable acquisition through `encoder.finish()`, including camera, HUD and command preparation; excludes acquisition/submit/present waits |
| GPU time | **Unavailable**: no timestamp queries requested or inferred |

The stop decision uses real elapsed time at the first completed frame at/after
20 seconds. Duration can exceed 20 by a frame/stall; the reported duration retains
that overshoot. The last interval/frame is included in mean FPS and percentiles;
completions after 20 s are excluded from the twenty full-window extrema. The final
camera distance is clamped to the route end. No 1,200-frame stopping rule exists.
Summary calculation/rendering after completion is outside the timed run.

The HUD and `Documents/rerbr-probe.log` show the final numbers. Logs include
`ROUTE PASS`, `WORKLOAD`, `START`, `SUMMARY`, `RESTART` or `ABORTED`. Startup retains
the existing device/BC, surface-resolution, pipeline, upload and render diagnostics.
No raw game data is included in the summary. A bounded sample store invalidates a
run rather than silently dropping frames if its capacity is exceeded.

## Build and validation

From the standalone repository root (local `ios-probe` contents):

```powershell
cargo run --locked --example harwood-benchmark-diagnostic --no-default-features --features harwood-benchmark -- 'C:\Richard Burns Rally\Maps'
cargo test --locked --workspace --no-default-features --features harwood-benchmark
cargo clippy --locked --workspace --all-targets --no-default-features --features harwood-benchmark -- -D warnings
cargo fmt --all -- --check
$env:HARWOOD_MAPS = 'C:\Richard Burns Rally\Maps'
cargo test --locked --workspace --all-features -- --ignored --nocapture
```

Normal CI uses synthetic route/terrain and clock inputs, never private assets.
The ignored real-route test checks the unchanged workload, supported camera path,
clearance, finite smooth camera samples and heading. Unit tests cover distance
interpolation, 30/60 FPS and much lower simulated rates, duration overshoot, stalls,
window extrema, percentiles, restart, invalid clocks, gaps, ground planes and HUD.
Naga validates the HUD shader independently of a GPU backend.

The local script/template accept `harwood-benchmark`. On macOS:

```sh
PROBE_MODE=harwood-benchmark bash scripts/build-ios.sh
```

The feature expands to the existing neighbor dependencies. The script passes
`--no-default-features --features harwood-benchmark` to host tests, iOS Clippy and
the release static library. Rust versions/lockfile, Xcode paths, ARM64/lipo checks,
signing requirements and unsigned IPA packaging remain unchanged. Only the local
secondary template was updated: the active workflow and GitHub clone were not
touched or synchronized, and no Actions run was started.

Pending physical checks: Apple compilation, original BC uploads, moving view and
ground clearance, HUD readability in both orientations, 20-second wall-clock
duration/overshoot, summary and repeated restart, interruption handling, regression
builds of all previous modes, and device temperature/memory/frame pacing. Record
resolution and thermal conditions when comparing repeated runs. Windows CPU tests
do not establish iPad performance, Metal compatibility or visible correctness.
