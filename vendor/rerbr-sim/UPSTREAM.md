# Upstream source and probe adaptations

GPL-3.0-or-later, retaining reRBR's source and LICENSE. Source archive:
https://github.com/JodusNodus/rerbr, local reference
`C:\Users\adria\Downloads\rerbr-main\rerbr-main`, imported 2026-11-15.
No commit identifier is asserted because the reference archive has no Git metadata.

UPSTREAM-HASHES.json records the original SHA-256 of all 623 imported Rust/source
and numerical-test/table files. It covers the src trees of rbr-physics, rbr-hybrid,
rbr-formats, rbr-gameplay, rbr-recording, rbr-engine, rbr-tyre and rbr-school, plus
standalone boot/{mod,hosts}.rs and handles.rs. GPL licenses are retained; the
dependency closure is local. The rbr-hybrid manifest builds rlib only, omitting its
desktop harness cdylib; all other upstream manifests are retained. No rbr-render/launcher/sound/SDL crate,
binary, game asset or captured commercial recording is copied. School/experimental
model sources are needed by original type dependencies; those modes are not selected.

The unchanged standalone boot is used directly, including original load order,
registry/factories, stage COL/TRK/MAT/FNC and SGC collision meshes. The probe's new
src/lib.rs adapts the orchestration in standalone physics.rs::boot_playback and
live/start.rs::settle. It uses the original Stage transition rules, fixed NGP
timestep and direct pure dispatcher. It deliberately omits Game/frame-finish/UI,
and only benchmarks a stationary neutral/braked car, never a simplified vehicle.

Exactly four imported files are adapted; PATCHES.json records their current hashes:

- upstream/rbr-physics/src/mem.rs: registers test-only original flat-host/reference
  and probe parity tests. The physics API remains the original one.
- upstream/rbr-physics/src/mem/pages.rs: substitutes an app-owned sparse allocation
  map for the 4GiB flat mapping and exposes allocation counters. Numerical ports and
  remaining word/byte/copy/epoch/snapshot algorithms are unchanged. sparse_map.rs
  and probe_tests.rs are new host code. pages_flat.rs retains the byte-identical
  original host, compiled only for tests.
- upstream/rbr-formats/src/ngp/mod.rs: path() uses the explicit Data roots instead
  of process-global environment overrides. SHA-256 validation, supported version,
  immutable-data decoding and no-execution behavior are unchanged. A read-only Data
  overlay lets Windows use the compatible package without altering RSF's DLL.

No physical equations or alternative tyre/engine algorithms are changed or added.
The existing generated exe_data.rs tables are upstream licensed Rust source, the
same table exception documented by the reference AGENTS.md. rcp.bin/rsqrt.bin are
CPU instruction-approximation reference tables, not game files; the fuzz text is
synthetic numerical validation data. Their exact hashes are retained and checked.

App-owned inputs/measurement/UI/worker code is GPL-3.0-or-later.
The app-owned adapter also exposes a separate acceleration diagnostic and records
physical displacement; see PHYSICS-DRIVE-AUDIT.md at the app root. It does not alter
the stationary probe controls or any imported equations. The movement test now
passes with the original first-gear index 2 (1 was neutral), followed by braking.
Read-only diagnostics use original typed layouts. This is not a playable app mode.
drive.rs adds an app-owned reusable boot/step Session for the physics-drive app
mode, using the same authentic calls. Its COL triangle reader is only an independent
contact diagnostic; simulation collision stays the original dispatcher. Added wheel
centre/strut/roll readings use original recording/layout definitions. No imported
source hashes or equations changed. See PHYSICS-DRIVE.md for the partial-stage scope.

Native public Mach
footprint reporting lives in the existing main.m; the app's previous entry/lifecycle
is retained. Panic unwinding is enabled only for the physics device build so a pure
host stop can be reported, without escaping the CPU worker or running original code.

The fourth adaptation, upstream/rbr-hybrid/src/lib.rs, changes only two test
inputs from 3.0 * 2f64.powi(-1060) to f64::from_bits(3u64 << 14). This
constructs the same subnormal directly for ARM64 test portability. Runtime
conversion code and physics equations are unchanged. PATCHES.json records its
existing hash; verify.py explicitly permits all four adaptations and still
checks every one of the 623 original/adapted entries.

audio_audit.rs is an app-owned read-only inventory adapter around the existing
original rbr-formats audio::CarSound and eng::parse modules already covered by the
623-file manifest. It adds no original source changes, audio samples, DLL execution
or playback backend. See STAGE.md at the probe root for measured bank inventory.

The app-owned `engine_audio.rs` subsequently adds an engine voice derived from
the parent GPL `crates/rbr-sound/src/engine.rs` (SHA-256
`2f2dfa277348eb743457accf5a19dbd401e5202b56101f1bb0a51b21a89e4b4f`),
with local cabin filtering and callback buffer preparation. `drive.rs` exposes
read-only original sound telemetry; no physics memory writes or equations were
added. `ngp_crc.rs` and the expanded `ngp_candidate.rs` are research adapters,
not runtime version enablement. The 623 upstream entries and four recorded
adaptations are unchanged. See `INTEGRATION-REVIEW.md` for limits and validation.
