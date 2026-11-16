# Expanded cockpit and session experience — 2026-11-16

Subsequent descriptor/loading API and Cargo changes are documented in
[CONTENT-MANAGER-ARCHITECTURE.md](CONTENT-MANAGER-ARCHITECTURE.md).

This continues the earlier manual-transmission milestone in
`C:\Users\adria\Downloads\rerbr-main\rerbr-main\ios-probe`.
It is not a rewrite or a complete implementation of every requested phase.

The updated development philosophy is recorded in `AGENTS.md`: authentic,
complete systems take priority over preserving a prior 60 FPS result. Physics
timing remains independent of rendering. Optimize after physical iPad
measurements; do not remove features based on an assumed performance limit.

## Physics and transmission

The earlier interpolation timestamp, HUD mutex, worker ownership/restart and
manual-transmission fixes remain. No original equations, physics source hashes,
mass, inertia, suspension, tyre coefficients or gear ratios were changed.
The original fixed timestep and collision paths remain intact. The subjective
floaty feeling still needs an identical-device camera/physics comparison and
an original-game/standalone PC oracle run.

New read-only telemetry uses original `CarStore::steer_target` and
`steering_rack_ratio`: physical steering-wheel angle is rack travel divided by
the original ratio. This follows `car_body/controls.rs`, which sets rack travel
to physical steering slew times `MaxSteeringLock` times the rack ratio. The
animation therefore reflects the physical actuator, not raw device tilt or an
invented steering multiplier. A private trial measured -0.7, +0.7 and zero
radians for settled steering inputs -0.5, +0.5 and zero with this setup.

Manual R/N/1–6, clutch and actual gear/RPM remain implemented. The touch controls
now occupy the screen's side edges, leaving the cockpit centre clear. Hit tests
and drawing share the same regions. Pedal role is captured at touch start and
cannot change when dragged across another control. Brake/throttle still allow
partial pressure by vertical drag; handbrake/clutch are held controls. Gear
changes are touch-start events and use the original physics payload.

Automatic transmission remains unavailable: the original loaded VCU's RPM
shift thresholds are empty. The capability guard remains; no replacement
thresholds, speed writes or fabricated RPM changes were introduced.

## Authentic cockpit and steering wheel

`physics-stage` now converts and renders the original interior as its own draw
set. Original face/material bindings, texture coordinates, node hierarchy,
door transforms and interior glass states are used. Fifteen original meshes
converted on the private Corolla, including `i_body`, `i_steeringwheel`, windows,
doors, mirrors, bonnet and wipers. Parts excluded by the original interior loader
remain excluded; this is not an arbitrary performance reduction.

The real `i_steeringwheel` node supplies its pivot and local Z axis. Its animated
matrix is `node * rotation_z(actual_angle) * inverse(node)`, the same node-axis
composition used by upstream `characters/incar.rs::wheel_turn`. The angle is
interpolated on the same physical sample pair as the body. Nothing writes back
to the simulation. A sixth GPU model matrix animates the wheel without changing
the mesh buffers every frame. Cockpit mode selects interior draws; chase/bonnet
select exterior draws. The original `Cam_internal` mount is retained.

Missing/invalid interior assets leave exterior driving available. The provisional
view is explicitly labeled as having no interior and shows an unavailable-asset
message; the exact failure is logged. An interior is only advertised as ready
after its geometry, steering node and referenced DDS files pass CPU validation.

**Additional private imports required for the cockpit:**

| Exact Windows source | Exact iPad destination |
| --- | --- |
| `C:\Richard Burns Rally\Cars\COROLLA\textures\corolla_internal.dds` | `Documents/RBR/Cars/COROLLA/textures/corolla_internal.dds` |
| `C:\Richard Burns Rally\Cars\COROLLA\textures\corolla_internal_transparent.dds` | `Documents/RBR/Cars/COROLLA/textures/corolla_internal_transparent.dds` |

Existing Corolla SGC/INI/shader files and the three exterior DDS remain in their
existing Documents paths. No extra car geometry file is required. Scrape and
alternate windscreen textures inventoried previously are not required by the
current undamaged interior. No private assets were copied into either repository
or bundled into an IPA.

This is a real geometry/material implementation, not complete original renderer
parity: dashboard needles, crew/hands, moving wipers, damage/decal layers, mirror
reflections and full cabin lighting are not implemented. Windows cannot establish
on-device appearance, clipping, transparency, FOV comfort or rotation direction
as perceived in the actual cockpit; capture these on the iPad.

## Main menu, settings, HUD and sessions

The app now starts in a main menu with DRIVE, SETTINGS, CONTROLS and ABOUT.
DRIVE names only Harwood Forest, the Toyota Corolla WRC and manual transmission.
Starting/resuming requires ready stage/car/physics, valid landscape steering and
no scene/physics error. No unsupported car, stage or automatic mode is selectable.

SETTINGS exposes supported steering sensitivity/dead zone, FPS and diagnostic
HUD toggles. Values persist in `Documents/rerbr-driving-settings.txt`; invalid,
nonfinite or out-of-range steering preferences fall back to valid bounded values.
Calibration remains explicit rather than persisting a device pose across sessions.
CONTROLS explains tilt, pedals/clutch and shifting, with calibration access.
ABOUT shows package version, available GitHub build number, upstream attribution
and GPL licensing. No copyrighted logos, menu art or music were added.

Pause opens RESUME, RESTART / RESET TO START, SETTINGS, CONTROLS and MAIN MENU.
Returning to the main menu prepares a fresh original physics session while
retaining already-loaded stage/car GPU resources. Restart resets to original
spawn and stays paused; it is not an invented roadside recovery teleport.
Backgrounding clears held inputs and requires explicit resume. Permanent
asset/GPU errors remain distinct from resettable physics errors, so a physics
reset cannot accidentally permit driving without a valid scene.

The driving HUD contains compact actual gear/speed/RPM, optional FPS, camera,
pause and telemetry controls. Pedals, clutch, handbrake and shifting are at the
side edges so they do not cover the centre of the steering wheel. Calibration
and steering adjustment are in the menus. Advanced telemetry is collapsible.
Actual safe-area/rotation, label size and multi-touch comfort remain device checks.
The menus use the existing Metal HUD, introducing no new platform UI dependency.

## Audio status and private paths

Engine audio playback remains unfinished, not omitted to protect frame rate.
The existing original bank audit and silent fallback remain. Completing playback
still requires the original granular voice, actual EngineNoise/load/cylinder
telemetry, an iOS output backend and interruption/pause/resume integration.
There is no nonfunctional volume slider or generic synthesized replacement.

The engine samples are embedded PCM16 mono plus firing markers in the original
ENG bank, with six RPM banks, twelve load parts and both 44.1/48 kHz parts:

| Windows source | iPad destination |
| --- | --- |
| `C:\Richard Burns Rally\Audio\Cars\Cars.ini` | `Documents/RBR/Audio/Cars/Cars.ini` |
| `C:\Richard Burns Rally\Audio\Cars\subaru.eng` | `Documents/RBR/Audio/Cars/subaru.eng` |

Retail Car6 maps to `subaru`; this is the original assigned engine bank. These
files remain optional for the current silent candidate and need no separate WAV
imports for the engine samples. They must stay outside the public repository/IPA.

## Performance, memory and extensibility

No feature was rejected because of an assumed 60 FPS ceiling. No new frame-rate
cap or dependency was introduced. Existing full-stage static geometry, culling,
shared textures, build modes, bundle identifier `org.rerbr.probe`, private roots
and GitHub Actions/IPA packaging remain unchanged. The interior is uploaded once
and shares the car's draw pipeline; only its matrices change while driving.
Menus reuse the scene resources, and physics workers are stopped/joined before
replacement. The existing fixed-step backlog behavior remains separate from
render cadence; a slow frame never changes physics dt or causes skipped steps.

The mesh-set converter, interior flag and steering transform are reusable building
blocks. The current private-file profile still explicitly supports only the
verified Corolla; future cars need their own verified profiles rather than
pretending that an arbitrary car is already supported.

Physical FPS, thermals, audio latency, memory across repeated session changes,
Metal validation and visible captures remain pending. No previous 60 FPS result
is asserted for this candidate.

## Validation and delivery

Windows verification:

| Check | Result |
| --- | --- |
| Rust formatting | PASS |
| All-feature/all-target Clippy, `-D warnings` | PASS |
| Partial `physics-drive` Clippy, `-D warnings` | PASS |
| Original car-chase mode Clippy and private benchmark | PASS; original 20-block workload unchanged |
| All-feature public tests | PASS, 67 tests |
| Release all-feature private suite | PASS, 16 tests |
| Final side-control layout: private driving, transmission and cockpit recheck | PASS, 7 tests |
| Default triangle regression | PASS |
| Original physics source/reference integrity | PASS, 623 entries with only pre-existing patch exceptions; flat-memory reference intact |
| Stationary and acceleration/braking fingerprints | MATCH previous milestone |
| Source diff, secret-pattern and private-asset scans | PASS |
| Manifest, lockfile, build/verifier scripts and native Xcode files | UNCHANGED |

Private checks cover actual original interior conversion/material dependencies,
index/model slots, original steering pivot invariance, physical rack sign/angle,
R/N/all forward gear acceptance and repeatability, backward movement, a loaded
manual shift, touch acceleration/braking, sampled COL contact/suspension response,
opposite steering responses, sixty-second repeated stability, camera/car
alignment, full-stage geometry and audio-bank parsing. The side-control change
was followed by the seven relevant private rechecks rather than treating the
earlier layout's passes as proof of the new hit regions.

Both original endpoint fingerprints remain:

- Stationary: `c7e1409b515dd69276132cde2c4992be166d35da262ababf49288ed5b73c454b`.
- Acceleration/braking: `2bcdc516d6e6d55c02393b8d1801dde151e82cb47c123e55d4bc6ad9ed9d1cba`.

These fingerprint selected car/wheel state, not complete runtime parity. Local
logs are ignored files under `.validation/expanded-*` and
`.validation/cockpit-*`. Release private tests use the existing unwind profile
override and existing private fixture/install paths; no proprietary fixtures
were created. The Python `scripts/verify.py` entry point remains pending because
this host has no working Python interpreter; the source checks above were
performed separately. No new IPA exists for package validation.

Exact files changed relative to the clean GitHub clone, including the earlier
manual-transmission work that has not yet been synchronized:

```text
AGENTS.md
EXPANDED-EXPERIENCE.md
MILESTONE-TRANSMISSION-AUDIT.md
src/car/cockpit.rs
src/car/gpu.rs
src/car/mesh.rs
src/car/mod.rs
src/car/runtime.rs
src/car/selection.rs
src/car/shader.wgsl
src/driving/controls.rs
src/driving/menu.rs
src/driving/mod.rs
src/driving/worker.rs
src/harwood/gpu.rs
tests/car_chase.rs
tests/cockpit.rs
tests/transmission.rs
vendor/rerbr-sim/src/diagnostics.rs
vendor/rerbr-sim/src/drive.rs
```

The GitHub clone remains unchanged. No commit, push, Actions run, IPA generation
or installed-app replacement was performed. Source is a candidate for reviewed
synchronization and Apple compilation, followed by physical iPad validation.

Remaining work includes original audio output, authentic automatic-transmission
integration, dynamic scenery/sky/water systems, the cockpit details listed above,
controlled engine-braking and material-force parity, and full PC/device oracles.
None is declared complete by source hashes or Windows tests alone.
