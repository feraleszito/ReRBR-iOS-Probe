# Manual transmission and integration audit candidate — 2026-11-16

Compatibility correction: see [DRIVING-MILESTONE-STATUS.md](DRIVING-MILESTONE-STATUS.md).
The verified NGP DLL and successful manual gear tests do not certify the stock-format
Corolla data as an NGP-compatible vehicle. The port omits the original car-file CRC guard.

Historical report for the first intermediate candidate. The subsequent cockpit,
menus and development-policy changes are documented in
[EXPANDED-EXPERIENCE.md](EXPANDED-EXPERIENCE.md).

This is an intermediate milestone in the existing `ios-probe`, not completion of
all nine requested phases. The working source is
`C:\Users\adria\Downloads\rerbr-main\rerbr-main\ios-probe`. The GitHub clone,
workflows, bundle identifier `org.rerbr.probe`, private files and installed iPad
application were not changed. No commit, push, Actions dispatch or IPA was made.
This document supersedes the first-gear-only descriptions in earlier milestone
documents for this candidate only. The owner's preceding 60 FPS device result
does not establish this candidate's performance.

## 1. Physics audit and actual integration fixes

No imported physics/gameplay equation, source hash, collision switch, mass,
inertia, spring, tyre coefficient, gear ratio, velocity or RPM was patched.
`Session::boot` still invokes the original boot/setup/NGP initialization and
1,520 load steps. Measured steps retain dt bits `0x3ab60b61` (approximately
1/720 second). Source integrity alone is not evidence of runtime parity.

| Area | Evidence and status |
| --- | --- |
| Mass, inertia, centre of gravity | Original `boot::build`, setup files and `UpdateA720`; no app multipliers. Private `Physics/t_coroll/common.lsp` inputs include inverse body mass 0.000714, inverse inertia diagonal (0.00079, 0.00169, 0.00081), and centre (0, -1.37, 0.22). These are archive inputs, not a new measurement of final assembled mass or weight distribution. |
| Suspension, ride height, anti-roll | Original gravel/default setup and solver retained. Default archive spring stiffness is 50,000 front and 45,000 rear; original dampers, travel, helper springs, mounts and anti-roll bindings are loaded. Diagnostic samples read actual strut travel and wheel centres. No spring tuning was justified. |
| Grip, slip, surfaces and collision | Original complete Harwood COL/MAT/FNC load and dry O weather setup retained. Presentation DDS choices do not set tyre friction. Existing independent contact and visual/COL height tests cover sampled locations, not all stage surfaces or material-force equivalence. No friction fallback was added. |
| Steering | Tilt remains normalized, with default sensitivity 2, dead zone 0.03 rad and 35 ms input smoothing. Original setup/rack mapping remains responsible for physical steering; no extra steering ratio is imposed by the app. New telemetry reads steering delivered to the original control sink. |
| Differential, brake and engine brake | Original drivetrain, differential locks, VCU/gear guard and clutch assistance retained. Pedals and clutch use the original control payload. No replacement torque or engine-braking model. |
| Body and wheel movement | Body quaternion and wheel centre/travel/roll remain sampled from physics. Wheels already move vertically. Exterior wheel steering/camber orientation remains incomplete; this audit does not establish its frame convention. |
| Cameras | Chase target smoothing (60 ms), heading smoothing (100 ms), world-up roll stabilization, 65-degree projection and original bonnet/cockpit mounts remain. They can affect perceived motion; no device A/B recording establishes causality. |
| Timing | Worker retains its deterministic fixed step, catches up without skipping, and pauses at >150 ms debt. Fractional step debt now survives in the published simulation-boundary timestamp. Previously publication time discarded that phase, causing interpolation cadence to depend on wakeups. |

Two further integration fixes: HUD generation copies its snapshot and releases
the physics mutex before text conversion/GPU submission; restart explicitly
stops and joins the previous worker before creating another. Drop also joins,
so detached workers cannot outlive their owning runtime. Joining during a boot
can delay the UI until that boot finishes; cancellation inside boot is not added.
Periodic physics-step logging is now limited to debug builds.

**The subjective floaty feeling is not resolved conclusively.** Timing and
render-lock coupling were real issues; the physical configuration was not
retuned to mask them. No identical original-game or full standalone PC oracle
run was performed. A matched starting state/control stream/timestep capture,
wheel forces/material IDs, dynamic weight distribution, cornering/rough-road
and device camera comparisons remain necessary for a complete fidelity audit.

The new private tests also exposed an initial control inhibition interval after
the load/Driving transition. Stationary gear acceptance tests wait two simulated
seconds before issuing commands; this does not alter app physics or skip steps.

## 2. Transmission

Manual R, N and all six forward gears use the original direct-selection payload.
GEAR - / GEAR + touches change a bounded requested index once per touch start;
the original drive/guard/clutch implements the shift. A selection already
accepted as the commanded gear is not repeatedly reissued every 720 Hz step.
The gear indicator displays actual gear, not the requested selection.

The read-only configured band diagnostic returned:

| R | N | 1 | 2 | 3 | 4 | 5 | 6 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| -2.2857144 | 0 | 2.9230769 | 2.125 | 1.6666666 | 1.3333334 | 1.0869565 | 0.89285713 |

These are original configured gearbox bands; they are not total wheel ratios
including the final drive. The app contains no replacement ratio table.
CLUTCH supplies the original pedal channel; the existing original clutch-help
and gear-guard settings remain enabled. A clutch touch retains its role if
dragged into the handbrake region. Existing pedal capture behavior is preserved.

Automatic mode is **not selectable**. Enabling the original race auto-gears aid
alone launched the car but did not upshift in a ten-second private trial. A
read-only VCU audit found all upshift/downshift thresholds zero, launch flag 1,
and rev limit 7,800 RPM. The missing threshold/NGP automatic integration needs
comparison with the complete upstream loop. No thresholds were invented.
`boot_with_transmission(..., true)` explicitly rejects the incomplete setup
instead of silently promising automatic transmission.

## 3. Cockpit and steering wheel

The provisional cockpit viewpoint is preserved and remains labeled
`COCKPIT VIEW ONLY - NO INTERIOR`. No interior or steering-wheel animation was
implemented in this batch. The assets are present, not missing: original
`Cars/COROLLA/corolla.sgc` and `corolla.ini` contain `i_body`, `i_steeringwheel`
and other interior sections. `selection::exterior` explicitly rejects `i_`.
The converter/GPU path still needs a separately selected interior set, its
materials, original steering pivot/axis/state and view-dependent visibility.

Confirmed private texture files in `C:\Richard Burns Rally\Cars\COROLLA\textures`:
`corolla_internal.dds`, `corolla_internal_transparent.dds`,
`corolla_internal_scrape.dds`, and `e_p206_internal_kwindscreen.dds`.
This is an inventory, not a completed minimum material dependency list.
If needed by the future converter, their corresponding destinations are
`Documents/RBR/Cars/COROLLA/textures/<same filename>`; do not import them for
this candidate. No invented cockpit or visual steering response is claimed.

## 4. Audio and exact private paths

Playback remains silent. Existing Car6 mapping/ENG parsing and silent fallback
are preserved; no audio device, mixer, load telemetry or interruption handler
was added. The audited engine bank contains embedded PCM16 mono samples and
firing markers; it needs no separate WAV files for those engine samples.

| Windows source | iPad destination |
| --- | --- |
| `C:\Richard Burns Rally\Audio\Cars\Cars.ini` | `Documents/RBR/Audio/Cars/Cars.ini` |
| `C:\Richard Burns Rally\Audio\Cars\subaru.eng` | `Documents/RBR/Audio/Cars/subaru.eng` |

Car6 maps to `subaru`; this is the original assigned bank, not a distinct
Corolla-only recording. Existing audit: six RPM banks, twelve load parts,
388,138 samples, 1,141 cycle markers, 44,100/48,000 Hz. No extra private asset
is required for this manual-transmission/HUD candidate. Audio playback still
requires the upstream granular voice, actual engine-load/cylinder telemetry,
bounded callback work and an iOS output/lifecycle implementation. No copyrighted
asset was copied, committed, bundled or exported.

## 5–7. HUD, menu, settings and sessions

The default road-covering telemetry block is removed. Actual gear, speed and
RPM use a compact row; TELEMETRY toggles steering, ground flags, travel, angular
and linear velocity, clutch/slip, commanded/actual gears, CPU/cadence and physics
memory. Diagnostics replace the camera-label region instead of overlapping
pedals/shift buttons. Close telemetry to use the camera control again. Pedals and gear/clutch
touch zones are distinct. Existing calibration, sensitivity/dead-zone, pause,
reset and chase/bonnet/fallback switching remain available.

This is a smaller diagnostic HUD, not final visual polish. Layout uses the
existing normalized inset coordinates; native safe-area/rotation and legibility
must be checked on iPad. No screenshots or GPU capture were produced on Windows.
FPS is available in telemetry, not a separately persistent small counter.

The app still enters the stage paused. Main menu, settings pages, persistent
preferences, return-to-menu, volume controls and a separate safe roadside
recovery are not implemented. RESET STAGE performs an original fresh boot at
spawn and pauses; backgrounding retains the existing pause/input-clear path.
Missing-asset/error messages remain visible even with telemetry closed.

## 8. Performance, resources and preserved build contract

Full Harwood stage geometry, culling, shared textures, GPU pipelines, resource
budgets, Documents roots, Cargo feature/build modes, lockfile, Xcode shell,
bundle identifier and GitHub workflow remain unchanged. This patch adds no
runtime dependency, GPU pass or private asset. Gear state is scalar; diagnostic
samples are fixed-size. Hidden advanced HUD strings are no longer built each
frame. The existing text HUD still allocates vertex/text buffers per frame;
no claim of zero allocations is made. Audio cannot block rendering because
playback is still absent. Worker joins address overlapping physics lifetimes;
long device restart/memory/thermal testing remains pending.

## 9. Validation

Final Windows validation:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --offline --locked --all-features --all-targets -- -D warnings` | PASS |
| Clippy for `physics-drive` without `physics-stage`, all targets, `-D warnings` | PASS |
| `cargo test --offline --locked --all-features` | PASS, 64 public tests; private tests explicitly executed separately |
| `cargo test --offline --locked --no-default-features` | PASS, default triangle regression |
| Release all-feature private tests (`-- --ignored --nocapture`) | PASS, 14 tests |
| Release `car-chase-benchmark` private car test, without physics-stage | PASS, 1 test, original 20-block workload |
| Original physics source/reference integrity | PASS, all 623 SHA-256 entries with only existing patch exceptions; original flat-memory reference intact |
| Source diff, secret patterns and private-asset scans | PASS; clone remains clean |

Release uses `CARGO_PROFILE_RELEASE_PANIC=unwind`, as required by the existing
driving workflow. Private physics tests used the already-existing isolated
16-file Documents layout outside both repositories:
`C:\Users\adria\AppData\Local\ReRBRProbeChecks\physics-minimal-dfe3feaf316b4a4eb70091581d5437e5`.
Stage/audio and car checks read `C:\Richard Burns Rally`; boot/render uses the
existing NGP overlay. No new private fixture was created.

Private coverage includes repeated acceleration/braking; left/right heading,
sampled real COL support and suspension response; repeated 60-second finite-state
and page-budget checks; physics/model/camera alignment; all eight actual gear
indices and 13 transitions with the real clutch payload; backward displacement
in reverse; a manual shift under load with identical repeated samples; explicit
automatic capability rejection; original DDS/car/partial-stage and full-stage
asset diagnostics. The full-stage test retains 282 blocks, 442 objects, 604
placements, 28 textures and 568/568 sampled visual/COL support checks.

Stationary car/wheel fingerprint remains
`c7e1409b515dd69276132cde2c4992be166d35da262ababf49288ed5b73c454b`.
The acceleration/braking endpoint remains
`2bcdc516d6e6d55c02393b8d1801dde151e82cb47c123e55d4bc6ad9ed9d1cba`.
These fingerprint selected recorded state, not the entire runtime.

Logs are local ignored files under `.validation/milestone-*`. The initial
automatic trial failed to upshift; that finding is retained as a capability
guard rather than an exposed feature. The initial aggregate private run also
found the pre-existing partial-car test's 9,539-triangle assertion being applied
to the 427,976-triangle full stage. Its assertions are unchanged and pass when
run in their intended mode; the separate full-stage test passes as well.

Pending: Apple compilation/link/IPA/package verification, actual touch/Metal
captures and safe-area checks, iPad performance/thermal/memory measurements,
all forward gears under sustained load, controlled engine-braking curves,
wheel contact forces/material parity and an identical original-game/standalone
PC oracle run. No before/after device capture was possible on Windows.
The Python `scripts/verify.py` entry point was not run because this host has no
working Python interpreter; source hashes, diff/asset/secret checks and unchanged
build-file comparisons were checked separately. No new IPA was available for
package checks. The complete upstream repository's shell/hybrid/oracle suite
was not executed; no imported upstream source was modified.

## 10. Exact changed source files

- `src/driving/controls.rs`: gear/clutch input, touch capture, pending selection and tests.
- `src/driving/worker.rs`: worker ownership, fixed-step timestamp, selection dispatch and logging.
- `src/driving/mod.rs`: manual controls/readout, collapsible HUD, snapshot lock and restart.
- `vendor/rerbr-sim/src/drive.rs`: original aid boot option, capability guard, read-only ratio access.
- `vendor/rerbr-sim/src/diagnostics.rs`: actual steering and read-only gearbox/VCU audit.
- `tests/transmission.rs`: private deterministic gear/clutch/reverse/load tests and auto capability gate.
- `tests/car_chase.rs`: scope the 20-block private assertion to its original build mode; run that mode separately.
- `MILESTONE-TRANSMISSION-AUDIT.md`: this audit and delivery report.

No file beneath `vendor/rerbr-sim/upstream` or any provenance/hash manifest changed.

## 11. Delivery and next physical validation

The source is ready for reviewed synchronization and a candidate Apple build,
not a complete nine-phase delivery or a device-validated release.
Review this local intermediate milestone before synchronizing selected source
files to the GitHub clone. Apple compilation/link/signing, IPA validation and
physical iPad checks are pending. Use the existing `physics-stage` Actions mode
only after the owner's review and later authorization; preserve Documents and
the installed app. No new build mode or workflow edit is needed.

Device validation should compare the prior candidate and this one on the same
road: calibration, partial/full steering, suspension over bumps, braking/coast,
all manual gears and reverse at rest, shifts with/without clutch, cameras,
telemetry readability, pause/background/restart, frame cadence and memory after
repeated resets. The initial control inhibition and physical gear-guard behavior
must not be confused with lost touch input. Automatic shifting, cockpit/audio
and main-menu completion remain follow-up work, not advertised functionality.
