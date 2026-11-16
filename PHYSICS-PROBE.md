# Pure reRBR physics probe

`physics-probe` is an independent CPU diagnostic, not the playable demo. It uses
the authentic reRBR physics, pure dispatcher and standalone boot. An animated
triangle shows UI liveness; the existing ASCII HUD reports CPU progress/results.
It does not load terrain/car GPU resources, request BC, implement driving controls,
or run NGP DLL instructions. Existing visual features remain separate.

## Private files: 16 required paths

Keep the existing private RBR installation untouched. In the app's Documents:

```text
RBR/
  Plugins/NGP.dll
  physics.rbz
  misc.rbz
  Scripts.rbz
  Cars/Cars.ini
  Cars/COROLLA/corolla.sgc
  Sgs/br_spectator_00.sgs
  Sgs/br_spectator_01.sgs
  Sgs/br_spectator_02.sgs
  Sgs/br_spectator_03.sgs
Harwood/Maps/
  track-20_O.lbs
  track-20_O.col
  track-20_O.trk
  track-20_O.mat
  track-20_O.dls
  track-20_O.fnc
```

Source NGP.dll **and physics.rbz** from your private
`C:\Users\adria\Downloads\RBR_NGP-7.4.778.507` package. The DLL must hash to
`95b6dbf5e8957357d857a8e919cff177a6bfb832f5324da90467d43732edaaa4`.
Do not replace the 7.5 DLL in `C:\Richard Burns Rally\Plugins`.
All other paths come from their matching paths under `C:\Richard Burns Rally`.
Cars.ini, corolla.sgc and the LBS are already present if stage 5 was imported as
documented. The remaining 13 are additional files for this diagnostic.

Transfer through the app's existing Documents file sharing, preserving names and
subfolders. Never copy them into GitHub Desktop, this source tree, Xcode resources
or Actions artifacts. Relaunch after transfer. The app opens only these two private
roots; there is no desktop-path fallback. The explicit Windows example additionally
accepts the existing installation and compatible package as separate read-only roots.

The real Windows preflight resolved all 16 files (36,321,934 bytes) without writing
to the installation. A second Release test used only SHA-256-verified copies of
these 16 files in a private temporary Documents layout, outside both repositories,
with no installation or overlay fallback. Both boots and all steps passed there.
The boot uses the SGC's original collision grids, not the reduced graphical mesh.
DDS, car material INI, interior, audio, shaders, animation clips and spectator
textures are not needed by this CPU probe. The complete existing archives contain
the following required members, read in memory:

- misc.rbz: Misc/prime.lsp, Misc/nice_w.ini and Misc/bad_w.ini.
- physics.rbz: Physics/physics.lsp; Physics/t_coroll/common.lsp, gravel.lsp,
  r_gravel.lsp and setups/d_gravel.lsp; Physics/michelin/tyres.lsp.
- Scripts.rbz: the original load_car_flip scripts and their animation-object counts.
  SGS hierarchies are required to preserve original car-part handle numbering.

The default "good weather" list chooses Harwood N. To keep the O files already in
Documents, the probe selects an actual dry O season-table row using the original
random-weather function with fixed valid draws: row 33 on these files, normal wear.
It does not fabricate weather/material words or relabel the N stage as O.

## What executes

Each of two independent passes:

1. boot::build constructs the original registry, stage collision system, materials,
   car collision grids, vehicle parts and fences from private data.
2. Playback::from_image, UpdateA720 and ngp_init establish the original NGP state.
   The pure dispatcher uses direct Rust calls with keep_going=false: an unsupported
   operation is a reported stop, never a call into the original executable.
3. The original load state consumes 38 StepForward(40) calls: 1,520 settling steps.
   Original Stage::load_tick/go/countdown_tick rules then select the driving state.
4. The probe runs 720 StepForward(1) calls at the loaded 0x3ab60b61 timestep,
   approximately 1/720s: about one second of simulated time. Neutral, full clutch,
   full brake and zero throttle deliberately keep this a stationary contact test.
5. A SHA-256 of the original FRM1 car/wheel bytes is compared across the two boots.

Equations, integration order, tyre model and NGP arithmetic are unchanged. This is
not the full Game/menu/countdown/frame-finish loop, an original-recording oracle
comparison, a moving-car benchmark or proof of whole-state determinism. Endpoint
hashes cover car and wheels, not every heap/stack/counter word. Experimental engine
and tyre models are not selected. Rendering never changes the physical pose.

The boot reported 50 registry entries, 7,625 stage placements, 604 interactive
handles, 76 stage meshes, nine car collision meshes, 13 car placements and 375
fence posts on the owner's files. No raw geometry or private file contents are logged.

## Host adaptations and timing

The probe's copied Pages host replaces the contiguous 4GiB+8MiB mapping with a
16MiB pointer/epoch table and stable zero-filled 4KiB allocations on first write.
Logical u32 addresses and the upstream read/write/copy/snapshot/epoch algorithms
remain unchanged. A 256MiB allocated-page payload limit reports a caught stop.
It is not a promise of available iPad resident memory. The old flat host is retained
only for host tests; it is not compiled into the production device library.

The worker requests a 32MiB stack instead of the desktop wrapper's 256MiB. Both
actual Windows passes succeeded with that stack; ARM64 call depth/stack demand is
still untested. The worker owns its simulation and FPU thread-local state. A cold
boot retains the original logical x86 register/stack addresses inside Mem; these
are data addresses, not native ARM pointers or executable x86 instructions. The
boot is synchronous on that worker, so cancellation is checked after boot and
between settle blocks/individual measured steps. It cannot interrupt a constructor.

The physics build alone sets CARGO_PROFILE_RELEASE_PANIC=unwind. The pure host
reports unsupported calls using Rust unwinding; they are caught inside the worker.
With panic=abort the diagnostic refuses to start. A stack overflow, allocator OOM
or OS termination cannot be recovered by catch_unwind. Old visual build modes keep
their existing release panic policy and archive/Xcode/lipo/signing paths.

Per-step mean/P95/P99/max measure monotonic host wall duration around the actual
Playback::step call on the worker. These are not kernel-accounted CPU time or GPU
timestamps. Real-time factor is simulated duration divided by actual measured-loop
wall time, including progress/timer overhead. Initialization timing and settling
progress are separate from and excluded from that loop. The run is unpaced: slower than
real time is reported honestly, without changing timestep or inventing results.

Memory diagnostics distinguish allocated page payload, table bytes and the app's
TASK_VM_INFO.phys_footprint on iOS. Process footprint includes graphics, Rust,
stack and other allocations; sampled maximum is not a true transient high-water
mark. Native footprint is unavailable on Windows in this implementation.

Tap after completion/error to restart with new simulation state. Taps while busy
are ignored. Suspension/occlusion requests cancellation; wait for the worker to
finish before restarting. Errors and progress appear in Documents/rerbr-probe.log.

## Local and future Actions checks

From this folder, with the existing Rust toolchain/cache:

```powershell
cargo run --locked --example physics-diagnostic --no-default-features --features physics-probe -- 'C:\Richard Burns Rally' 'C:\Users\adria\Downloads\RBR_NGP-7.4.778.507'
$env:CARGO_PROFILE_RELEASE_PANIC = 'unwind'
cargo run --locked --release --example physics-diagnostic --no-default-features --features physics-probe -- 'C:\Richard Burns Rally' 'C:\Users\adria\Downloads\RBR_NGP-7.4.778.507'
cargo test --locked --workspace --no-default-features --features physics-probe
cargo clippy --locked --workspace --all-targets --no-default-features --features physics-probe -- -D warnings
cargo fmt --all -- --check
```

The ignored private integration test accepts PHYSICS_DOCUMENTS pointing to the
private Documents layout, or PHYSICS_RBR and PHYSICS_NGP for the two read-only
Windows roots. Run `cargo test --test physics_probe --features physics-probe --
--ignored --nocapture`; for Release add --release and the unwind override above.
No private data is used in ordinary CI tests.

In the isolated Windows Release run, the two boots took 0.141/0.136s. The 720
measured steps per pass averaged 0.349/0.325ms, with P95 0.703/0.646ms, P99
0.900/0.952ms and real-time factors 3.98/4.27. These are this PC's stationary
test results, not A17 estimates. Allocated page payload was 38,313,984 bytes
(36.54MiB), plus 16MiB table storage; process footprint was unavailable on Windows.
Both final car/wheel FRM1 hashes were
`c7e1409b515dd69276132cde2c4992be166d35da262ababf49288ed5b73c454b`.
Cross-architecture agreement and comparison against original recordings are pending.

The local secondary workflow template includes physics-probe. No GitHub clone or
active workflow has been changed. After an authorized synchronization, add only
physics-probe to the active workflow's build_mode options. It already passes
inputs.build_mode through PROBE_MODE to scripts/build-ios.sh; the new script case
selects --no-default-features --features physics-probe and the unwind override.
Build/archive paths, ARM64 lipo verification, unsigned packaging and IPA checks
remain the same. All 16 files must be imported privately after installing the IPA.

Source provenance is in vendor/rerbr-sim/UPSTREAM.md and UPSTREAM-HASHES.json.
The two 8KiB CPU reciprocal/rsqrt tables are upstream numerical reference data,
not commercial game resources. They are explicitly hash-checked exceptions in the
source verifier; no raw BIN files are bundled by Xcode.

Pending: Apple compilation/link/unwind behavior, native memory API, device boot,
32MiB ARM64 stack, live status/cancellation/restart, resident memory and optimized
A17 timing. Passing Windows CPU checks does not establish iPadOS success.
