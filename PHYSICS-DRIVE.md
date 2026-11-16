# physics-drive: partial-stage playable candidate

This mode connects the original Corolla physics to the existing Metal car/terrain
pass. Windows CPU tests pass; the new native motion bridge, Apple build, image,
controls and performance have NOT been tested on iPad. No new IPA was generated.

## Scope and controls

- Visible region: the existing 20 original Harwood blocks, with deduplicated BC
  textures. Physics loads the original complete collision stage. This is not a
  full-stage release; no finish condition, stage progress or championship clock.
- First forward gear only (original index 2). No invented automatic gearbox.
- Starts paused. Wait for geometry and physics to load, hold the iPad in landscape
  like a steering wheel, tap CALIBRATE, then PAUSE / GO.
- Bottom right THROTTLE, bottom left BRAKE: independent captured multitouch pedals.
  Slide down for more pedal, release for zero. Centre bottom HB is handbrake.
- Tilt steering uses CoreMotion device motion (gyro/accelerometer fusion), with
  calibration, SENS +/- and DEAD +/-. Default sensitivity is 2 per radian, dead
  zone 0.03 radians and exponential input filtering with a 35ms time constant.
  UIKit transforms portrait-fixed sensor gravity into the current view axes.
  Flat on a table, stale/unavailable motion, or portrait pauses safely; touch-drag
  is never used as a steering substitute. Recalibrate after changing how you hold it.
- PAUSE / GO freezes simulation and clears pedals. Resizing, suspension and
  occlusion also pause. RESET STAGE creates a fresh original boot at the actual
  TRK spawn; it is the recovery action for this milestone, not nearest-road rescue.
- Leaving visible terrain or losing a safe terrain-supported chase camera pauses
  with an error; RESET is required. It never teleports or clamps the physical car.

Physical motion controls body position/quaternion and wheel centres/roll; wheel
centres already include actual strut movement, so no fake suspension correction
is added. Visual tyre steering/camber orientation is not yet fully reproduced;
the compact wheel slots also rotate attached brake geometry. Objects/trees are
not rendered, while original stage colliders can be active. Terrain camera checks
do not cover those invisible objects. Inspect images and collisions on device.

## Architecture and coordinate evidence

vendor/rerbr-sim/src/drive.rs uses the same original boot, NGP initialization,
loading steps, Stage transition and Playback::step as the verified probe. Input is
the real CarControl payload; no coordinates/velocities are externally imposed.
The 720Hz worker owns its simulation, native stack and thread-local FPU state.
It loads privately on a 32MiB worker; existing graphics load on their asset thread.
Rendering never writes physics memory and never uses the benchmark route to move.

Only the latest two physical snapshots are retained. Position/wheel centres lerp
and quaternion slerp between fixed steps; roll wraps correctly. The model/body/
stage composition follows original playback/frames.rs: stage = (x,z,y) of physics,
model front -Z maps to physical -Y, and two mirrors preserve proper CW culling.
The body offset is derived from real SGC pivots and initial physical wheel centres.
The camera filters target/heading and reuses existing terrain/near-plane checks.
Metal materials, original DDS uploads, shared reverse-Z depth and pipeline remain.

The worker follows wall-clock debt at the loaded dt bits 0x3ab60b61, never variable
timesteps. Pauses discard paused wall time. More than 150ms simulation debt pauses
with an error rather than silently skipping physics. Rust pure-host panics are
caught on the worker and the original stop reason is logged. Native stack overflow,
allocator OOM and OS termination remain outside Rust recovery.

The HUD reports measured frame cadence, main-thread preparation, worker step plus
diagnostic/publication overhead, simulated time, speed, RPM, allocated pages and
sampled app footprint. GPU timestamps are unavailable. None of these are GPU/CPU
utilization percentages. Footprint sampling is not a transient high-water mark.
No moving-iPad FPS or memory result is asserted.

## Windows checks (Rust 1.97.1 MSVC)

- Straight, left and right real steering: five 20-second trials, repeated left/
  right sampled trajectories identical; opposite signed heading changes and
  different positions; acceleration and braking while steering; finite samples.
- Each sampled grounded wheel is independently checked above a triangle parsed
  from the actual COL file (vertical XY intersection, centre-to-surface gap 0..1m).
  Suspension travel changes from the settled state. This is sampled ground-contact
  evidence, not a whole-stage obstacle/penetration or original-recording oracle.
- Two 60-second simulations (30s throttle 0.4, 30s braking) have identical sampled
  states and finite results. Allocated page payload remains 38,313,984 bytes,
  under 64MiB; metadata adds 16MiB. Progress is not continuous throughout these
  trials: an unsteered car can encounter scenery/terrain and slow substantially.
- Actual touchscreen Controls values drive acceleration, braking to zero,
  acceleration again, and another stop without a separate clutch pedal.
- Original SGC/COL/LBS private CPU integration: actual spawn and five seconds of
  physical poses remain in the selected visible region, with supported chase
  cameras and all drawn body/wheel vertices in the landscape frustum. No GPU used.
- Mathematical tests check basis/winding, interpolation, wheel centres and wrap.
  Multitouch tests cover simultaneous pedals, captured roles, release, handbrake,
  UI isolation, calibration, dead zone and steering response.
- Existing stationary and acceleration/braking probes, ordinary app/shader tests,
  formatting, Clippy and imported-source hashes are checked separately. See
  VALIDATION.md for final counts and commands that could not run here.

Private test variables: PHYSICS_DOCUMENTS is the existing private 16-file Documents
layout. The real-model private library test uses read-only PHYSICS_RBR and
PHYSICS_NGP roots, with the compatible package as overlay. No private fixtures in CI.
Use CARGO_PROFILE_RELEASE_PANIC=unwind for Release CPU tests.

## Private inputs and build/update

Use the existing Documents/Harwood/Maps and Documents/RBR folders. The 16 physics
files listed in PHYSICS-PROBE.md and the existing Corolla eight-file visual profile
are required, plus the original map texture redirects and DDS used by neighbors.
There are no new proprietary assets for this partial mode; none are bundled.
Missing resources are errors. The full-stage texture inventory is still pending.

The local workflow template adds physics-drive. The GitHub clone and active
workflow were not changed in this batch. Keep the secondary template untracked;
after an authorized source synchronization, add ONLY this option to the existing
active .github/workflows/ios-probe.yml, after physics-probe:

    - physics-drive

The active workflow already passes inputs.build_mode as PROBE_MODE. The script
uses --no-default-features --features physics-drive and sets Release panic=unwind.
Static-library paths, Xcode/lipo checks and unsigned IPA packaging stay unchanged.
CoreMotion is linked by the Xcode project; Info.plist supplies the usage description.
The native bridge supports winit's legacy app lifecycle without a scene manifest.

Future build steps, once reviewed/published by the owner:

1. Synchronize only source changes listed in VALIDATION.md into the standalone
   repository root. Do not copy github-actions/, .validation/, target/, build/,
   private Documents, RBR/ or Harwood/. Preserve the two legitimate CPU BIN tables.
2. Make the one-line active selector change above, review GitHub Desktop Changes,
   and publish only when authorized. No publication/Actions dispatch was performed.
3. Run ReRBRProbe unsigned iPadOS with build_mode=physics-drive on the updated commit.
4. Check build/environment.txt for the feature flags, host tests, Rust ARM64 archive,
   Xcode native compilation, link and IPA verification. An artifact is not proof of
   device execution. No ARM64 compilation is possible with the available Windows SDKs.
5. Download ReRBRProbe-unsigned-physics-drive-<run>, verify SHA256SUMS, and sign using
   the owner's existing Windows installation method. Preserve the exact final
   signed application identity/account; tools can rewrite the unsigned bundle ID.
6. Update over the existing app without uninstalling. Keep a private Documents
   backup beforehand. Source bundle identifier remains org.rerbr.probe. Confirm
   existing files remain accessible, allow motion if asked, then test while paused.
7. Verify steering sign/calibration in both landscapes, multitouch, braking,
   suspension/image alignment, restart, backgrounding, boundary pause, cadence,
   memory and thermals. Share Documents/rerbr-probe.log and a short device capture.

Pending acceptance: Apple compile/link, successful Actions run/artifact, installation,
real motion/touch interaction, visual inspection and measured moving performance.
Full-stage rendering, higher gears/original assists, finish/progress, nearest-road
recovery, full wheel orientation and comprehensive collision/oracle checks remain.
