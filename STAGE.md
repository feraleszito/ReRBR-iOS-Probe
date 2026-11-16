# physics-stage: full-terrain/static-scenery candidate (2026-11-16)

The owner reports the previous physics-drive mode working on iPad mini A17 Pro,
including tilt/touch and 60 FPS in their partial-stage tests. Those results do not
apply to this new workload. No new iPad performance or visual result is asserted.

## Scope

Select the independent Cargo feature/build mode `physics-stage`. It includes the
existing driving implementation and adds presentation only. `physics-drive` keeps
its 20-block workload. No original physics source, equation, control calibration,
pedal mapping, fixed-step clock or collision solver is changed.

The full resident LBS terrain, static object lanes and supported `(1,6)` placements
replace the selected neighbors only in this mode. The original COL is also read
independently for a sampled terrain/physics coverage check. All 282 terrain blocks
must convert without unsupported draws or bad indices, and all 568 LBS driveline
samples must have terrain and corresponding COL height within 0.5m. This is a
loading gate, not proof of every intervening contact or scenery collision. The
Corolla still moves solely from the original simulation, never along that line.

CPU results on the owner's installation:

| Data | Result |
| --- | ---: |
| Terrain blocks | 282 / 282 |
| Static object items | 442 |
| Supported interactive placements | 604 |
| Withheld interactive placements | 0 |
| Vertices | 710,699 |
| Triangles | 427,976 |
| Potential indexed draws | 2,386 |
| Unique resolved DDS allocations | 28 |
| VB + IB + compressed DDS estimate | 36,470,024 bytes |
| LBS samples with visual/COL ground | 568 / 568 |
| Maximum sampled visual/COL height difference | 0.000053406 m |

Original baked RGB, UVs, world positions, triangle indices and diffuse blending
are retained. Vegetation cards/scenery use the original two-sided alpha-test
threshold (texture alpha > 60/255), depth testing/writing, wrap/mirror/clamp modes,
and shared reverse-Z depth. Named interactive transforms are baked once from the
original matrices; they are static, not animated or connected to damage state.

Every draw has a precomputed world AABB. Submission tests its distance (450m)
and homogeneous wgpu frustum, without per-frame allocations in culling. This
keeps near-plane intersections conservatively, instead of testing only visible
corners. Resources are loaded/uploaded once; no traversal I/O or streaming spikes.
The initial diagnostic camera selected 24/2386 draws on CPU. Actual frame counts
depend on camera and appear in `physics-stage VISIBLE` log lines every five seconds.
This is frustum/distance culling, not original PVS/occlusion or GPU-driven culling.
Resident data is capped at 192MiB before upload. The estimate excludes load-time
duplicates, CPU terrain/grid indexes, physics, depth/drawables and driver memory.
The existing HUD still reports cadence, CPU preparation, worker and app footprint.

The artificial 20-block boundary is absent in this mode. A car genuinely outside
supported terrain still pauses with an explicit error; unsafe chase terrain also
pauses. Original collision loads the complete stage, including colliders whose
animated visual representation is not implemented. Full-stage driving and object
contact must be tested physically. First gear/reset-to-spawn remain unchanged.

This is **not complete visual parity**: 7 sky/backdrop items, 1 water item and 71
animation-object records are counted but withheld. No cloud/dome pass, wind/tree
animation, spectators' skeletal animation, water/reflections, dynamic scenery
damage, moving start digits or full original lighting is added. Static vegetation
and supported scenery placement are included. Near/always terrain is used at all
visible distances; far-LOD switching and transition fading remain future work.

## Cameras

The new CAM button in the second top row cycles CHASE, BONNET, and
COCKPIT VIEW ONLY - NO INTERIOR. Other controls keep their previous locations.
Camera changes affect presentation only. The bonnet reads `Cam_bonnet2` and the
cockpit viewpoint reads `Cam_internal` Pos/Target from the original Corolla INI.
Both are transformed by the interpolated physical body matrix. Existing upright
view construction stabilizes roll; the existing 65-degree chase FOV/near/reverse-Z
projection is retained rather than introducing a different projection pipeline.
Unsupported/underground mounts return to the checked chase camera. Chase retains
its existing smoothing and terrain tests; scenery/branch camera occlusion is not
solved. Switching views never pauses or changes physical input.

The original SGC/INI has interior parts, but our exterior-only converter has not
converted them. The explicitly labeled cockpit fallback hides the exterior model
and shows the original interior viewpoint. It contains no invented dashboard,
steering wheel or wheel animation. A later true cockpit needs the original
`corolla_internal.dds`, `corolla_internal_transparent.dds` and potentially material-
referenced extras verified during a dedicated interior conversion. Both files
exist in the owner's `Cars/COROLLA/textures` directory. Do not import them yet.

## Audio audit, not playback

`rerbr_sim::audio_audit` uses the already-vendored original `audio::CarSound` and
`eng::parse` readers. On this installation Car6 maps to `subaru`, as retail does:

- `C:\Richard Burns Rally\Audio\Cars\Cars.ini`
- `C:\Richard Burns Rally\Audio\Cars\subaru.eng` (785,924 bytes)

The decoded bank has 6 RPM banks, 12 load parts, 388,138 PCM samples, 1,141 firing
markers, rates 44,100 and 48,000 Hz. This is the original engine bank assigned to
the Corolla, not a distinct Corolla-only recording. ENGI/PART/WAVE contains PCM16
mono and firing markers; it is not an ordinary WAV and cannot simply loop as one.

Optional private import for this audit only:

    Documents/RBR/Audio/Cars/Cars.ini
    Documents/RBR/Audio/Cars/subaru.eng

Copy from the owner's installation only, preserving these paths. Missing banks
log AUDIO DISABLED without preventing geometry. No samples are bundled, exported
or logged. This build is explicitly silent (HUD AUDIO OFF). No generic synthesized
audio or Windows DLL execution was added.

The original `crates/rbr-sound/src/engine.rs` is a pure granular voice; `data.rs`
loads the above files and effects, `mixer/` mixes them and `device.rs` uses cpal.
A reliable next batch must adapt the voice/mixer, supply original SND1/EngineNoise
telemetry (including load/cylinder flags, not fabricated from RPM), handle both
sample rates and pause/reset/interruption, then implement and measure an iOS audio
output backend. Existing `rbr-sound` depends on cpal/hound/lewton and embedded
effects outside this probe's subset. It was not linked speculatively in this
batch. Gear, tyre/gravel and ambience remain unimplemented, not generic stand-ins.
Their installation inputs include `Audio/Cars/Effects.ini`, `Audio/Surfaces/`,
`Audio/Impact/Sound.ini`, `Audio/Ingame.ini` and root `audio.ini` (present here,
resolved by the original Data reader). The full bank loads entries through
`audio.dat`; a sandbox loose-file/archive policy and exact per-effect inventory
must be audited before importing more.

## Assets and validation

Keep the existing 16 physics inputs and eight-file Corolla exterior profile.
Full stage additionally needs all resolved diffuse DDS for static/interactive
scenery, not just the former 8 terrain textures. Reads stay inside the existing
private Documents roots (`Harwood/Maps`, `RBR`), never the app bundle. The shared
reader supports loose DDS and the owner's existing private RBZ/redirection setup.
Missing required stage textures fail explicitly before driving; no substitutes.

Run the read-only CPU example to list the exact 28 resolved paths (names only):

    cargo run --locked --features physics-stage --example stage-diagnostic -- "C:\Richard Burns Rally\Maps" "C:\Richard Burns Rally"

Most are in track-20_N_textures dry/new, dry or damp, including trees, fences,
rocks and signs. `track-20_O_textures/dry/start_clock.tga_0.dds` is an O resource.
Preserve the directories, track-20_O.ini, TextureFilenameMap.dat and existing
track-20_O.lbs/col in Documents/Harwood/Maps. There is no need to copy unrelated
stages or the original installation into Git. Stage counters match on Windows;
physical image/material validation is still required.

Tests cover world placement/index/color/UV conversion, invalid indices, conservative
frustum cases, camera mount parsing/body transformation/cycling, original driving
math/control regressions, WGSL and optional real-asset stage/audio checks. See
VALIDATION.md for measured results and unavailable Apple checks. No GPU capture
or audible render was possible in this Windows Metal-only build.

## Next build and physical test

This batch stays in the working source. It does not synchronize the GitHub clone,
commit, push, dispatch Actions or generate an IPA. The local secondary template
contains `physics-stage`; do not copy that template to Git. After a reviewed source
sync, add only `- physics-stage` after `- physics-drive` to the active workflow
build_mode choices. Existing PROBE_MODE forwarding selects
`--no-default-features --features physics-stage`, with Release panic=unwind and
unchanged ARM64/Xcode/lipo/signing/IPA packaging. Bundle ID is org.rerbr.probe.

Run that manual build only after publishing the reviewed changes. Inspect host
tests, source integrity, Apple compilation/link and IPA checks. Sign/update using
the same effective signed identity and account, back up Documents, and do not
uninstall the working app. No extra assets belong in the IPA.

On device: verify the coverage and DDS/upload logs; calibrate and resume; test
bonnet/chase/fallback camera while stationary first; drive beyond the old boundary;
inspect cutout foliage/fences/signs, car alignment, pauses/background/reset. Capture
moving frame cadence/CPU/draw counts/footprint and a short image/video on a long
traversal. The existing mode is still the fallback. Audio OFF is expected. Report
new FPS/thermals/memory measurements rather than carrying over the old 60 FPS.

## Provenance

GPL-3.0-or-later, preserving all existing upstream LICENSE files and hashes.
The new stage conversion adapts `crates/rbr-render/src/stage/mesh.rs` object_mesh,
interactive_mesh and buffer_mesh and `stage/mod.rs`'s evidence-based interactive
support gate; stage positions and two-sided alpha >60 follow those original
functions. Camera Pos/Target composition follows car camera mounts/body poses.
Source reference: https://github.com/JodusNodus/rerbr, local reference archive
`C:\Users\adria\Downloads\rerbr-main\rerbr-main`. No commit ID is asserted for the
archive. New culling, loading budget and camera UI are app-owned presentation code.
No new imported-source hash exceptions, proprietary fixtures or equations added.
