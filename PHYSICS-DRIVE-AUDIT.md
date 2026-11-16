# Physics movement blocker: resolved on Windows

## Root cause and minimal fix

The acceleration diagnostic incorrectly selected gear index 1 and described it as
first gear. Original rbr-recording/src/hud.rs:6 defines 0=reverse, 1=neutral,
2=first. Original standalone physics/layout.rs::drive_gear and live/picture.rs
read the same CDrive state, and pure/playback.rs::emit forwards CarControl to
CCar::UpdateSeed. The sole movement fix is Some(1) -> Some(2) in the app-owned
run_inner acceleration payload. No equations, initial coordinates or velocities,
collision switches, ignition flags or imported sources were changed.

The earlier suspected missing Game transition was not the cause of this failure.
Source comparison: standalone physics.rs::boot_playback uses boot::build,
Playback::from_image, untrack_allocator, UpdateA720 and ngp_init, as the probe does.
live/start.rs::boot/settle additionally orchestrates Game::frame_with for controls,
clock, assist settings, stage transitions and events; physics.rs::Pure::step reaches
the same Playback::step. Our explicit Stage transition reaches Driving and the
original dispatcher accepts the correct gear. The probe enables direct dispatch;
the standalone boot defaults to the fact-check dispatch. No change to this switch
was necessary. The complete standalone executable was not built/run in this batch.
This source comparison and controlled gear-only experiment are not an oracle run.

## Actual private Windows evidence

With neutral index 1: one-second and ten-second tests gave 0.0m displacement in
both independent boots. With index 2 and the otherwise identical ten-second
full-throttle test: 95.26619791691061m displacement in both boots; identical final
FRM1 hash 2db087cee45359a2f3e602d7f2581544ffc8b62ebc8083fdb9bf734be21d01ea.

The final diagnostic accelerates for five simulated seconds, then brakes for five
with clutch pedal 1.0. Each of two independent boots runs the original 1,520 load
steps followed by 7,200 measured steps at dt bits 0x3ab60b61. These are actual
original physics steps, unpaced on the host, not frame-counted animation.

| Sample | 5s acceleration | 6s (braking) | 10s |
| --- | --- | --- | --- |
| Physics world position | (-403.1034,1432.4366,75.250786) | (-400.8283,1423.318,75.18672) | (-400.05008,1420.1818,75.16819) |
| Velocity, m/s | (2.9354134,-11.790696,-0.10792282) | (1.4985442,-6.0102615,-0.013572549) | (0,0,0) |
| Engine RPM | 5632.161 | 2451.4001 | 1929.9417 |
| Actual/commanded gear | 2/2 | 2/2 | 2/2 |
| Live engine pedal / drive brake | 1/0 | 0/1 | 0/1 |
| Emitted clutch pedal | 0 | 1 | 1 |
| Wheel spin, rad/s | (+41.1401,-41.1401,+41.1401,-41.1401) | (+17.6117,-17.6117,+17.6117,-17.6117) | (0,0,0,0) |
| Ground probe flags | (1,1,1,1) | (1,1,1,1) | (1,1,1,1) |

The report stores initial state and every simulated second: position, quaternion,
linear/angular velocity, RPM, both gear fields, live engine pedal, drive brake,
emitted clutch, drive clutch-slip lane, drive engine speed, brake torque, legacy
engine torque field, wheel spin and ground-probe flags. They use original typed
Drive/DriveState/EngineState APIs and exported body/wheel offsets. The engine pedal
uses ngp/engine.rs::input's engine+0x20; the legacy drive throttle lane stays zero
and must not be mistaken for lost NGP throttle. The legacy engine torque field
also remains zero here; no claim of measured NGP output shaft torque is made.
Brake torque is an internal scaled value (11900000), not asserted to be Nm.
Clutch payload and stored drive clutch slip are distinct quantities.

Two sampled trajectories compare exactly, including all diagnostic fields, and
both end with FRM1 hash 2bcdc516d6e6d55c02393b8d1801dde151e82cb47c123e55d4bc6ad9ed9d1cba.
Allocated page payload remains 38313984 bytes. The unchanged stationary test still
ends at c7e1409b515dd69276132cde2c4992be166d35da262ababf49288ed5b73c454b.
The stationary probe historically labelled its gear-0/braked payload neutral;
that label was inaccurate (0 is reverse). Its actual payload remains unchanged.

## Regression checks and limits

The ignored private test asserts >5m/s after acceleration, actual first gear,
live engine pedal >0.5, brake/clutch receipt, positive brake output, wheel spin,
finite diagnostic state, final speed <0.1m/s, identical sampled trajectories and
identical endpoint hashes. Private Resources stay outside the project: set
PHYSICS_DOCUMENTS to the existing 16-file Documents layout. Set
CARGO_PROFILE_RELEASE_PANIC=unwind for Release and run:

cargo test --release --test physics_probe --features physics-probe -- --ignored --nocapture

Formatting, Clippy with -D warnings and app feature tests are checked on Windows.
Source provenance hashes remain intact; diagnostics.rs is new app-owned GPL code.
No new private asset was needed. No original engine, RBR installation, GitHub clone,
workflow, graphics, bundle ID or IPA was changed. No commit/push/Actions invocation.

Remaining: full original Game lifecycle comparison/oracle recordings, steering,
collision interaction beyond the initial straight, recovery, higher gears,
long-duration memory/finite-state checks, renderer interpolation and device driving.
The observed ground flags and braking are not a whole-stage collision proof.
No new iPad movement/performance result is claimed. No new selectable mode yet.
