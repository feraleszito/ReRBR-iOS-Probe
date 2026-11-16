# NGP 7.5.779 investigation and implementation

2026-11-16. **Feasible candidate for further port validation; not enabled as a
supported physics engine.** There is no genuine NGP vehicle package or original
vehicle-CRC implementation in the inspected local inputs. No 7.5 physics-parity
claim is made. Private physics, DLLs and the installed IPA were not changed.

## Imported implementation and exact assumptions

The imported GPL reRBR code targets NGP **7.4.778**, physics revision **778**,
using immutable inputs from DLL **7.4.778.507 (2024-09-15)**, SHA-256
`95b6dbf5e8957357d857a8e919cff177a6bfb832f5324da90467d43732edaaa4`.
`upstream/rbr-formats/src/ngp/mod.rs::SUPPORTED` contains only this hash. Parsing
validates it before decoding version-specific PE32 data and initializer ranges.
`Tables` is cached process-wide in a `OnceLock`; it is not a per-engine-version
cache. The port uses original 1.02 and NGP Rust functions, typed memory layouts,
named bindings and original numeric precision rules, not DLL execution.

The stage adapter already treats global revisions **778 and 779** as selecting
the first original stage/material table (`ngp/layout/stage.rs::REVISIONS`). This
was present before this change. Accepting that revision number does not select a
7.5 engine or validate a vehicle. Fixed stepping remains the original f32
`0x3ab60b61` (1/720 s), with the original load/settle sequence.

Per-car initialization, clamps, torque/drivetrain/tyre logic, callbacks and data
addresses are ports of 7.4.778. Its car-file CRC and later tamper rechecks are
explicitly unported (`ngp/layout/car_init.rs` header; parent `notes/ngp.md`,
“Boot”). The standalone integration consequently cannot certify original NGP
car authenticity merely by loading a script, finding CarId or driving in six
gears. The existing experimental Corolla baseline remains explicitly unverified.

## Direct local 7.4 versus 7.5 comparison

Candidate: `C:\Richard Burns Rally\Plugins\NGP.dll`, **7.5.779.508
(2025-03-05)**, SHA-256
`cf351cdf99f880631ed2cb1d9b6b5c9db7d7c233ad905740393c37df967801ec`.
Both proprietary binaries needed for this comparison are available locally.

Ran the original content-relocating comparator against those exact binaries:

- **25 read ranges located**. Address shifts: `+0x70` (5), `+0x80` (3), `+0x90`
  (1), `+0x11fe0` (5), `+0x13fe8` (1), `+0x13ff8` (1), `+0x14000` (3),
  `+0x14008` (5), `+0x14140` (1).
- Every compared decoded table is equal: stage constants, tyre presets, key
  names/classes/offsets, removed keys, engine/probe defaults, temperature data,
  atmosphere and road-surface mapping. Decoder-read code differs in no compared
  non-address bytes. Verdict: **ReadsUnchanged**, not physics parity.
- Installed `physics.rbz`: SHA-256
  `00a0f12d8d6844686eedcaee3fc804cd0040ae70bdd8af5b48fe6e3edd32a49f`,
  global revision **779**. Both archives have the same 96 file names. Compared
  raw decompressed entries against the 7.4 package: **95 equal entries; only
  `Physics/physics.lsp` differs**. Normalizing its revision number makes the
  complete global script identical: only 778 to 779 changed.
- Installed and 7.4 SDK `IPhysicsNGProxy`, `IService`, `IProxy`, and
  `TelemetryData` headers are byte-identical. Proxy API is documented as version
  **3**; its methods query revision/material patch/service state. It is not a
  portable physics-step or vehicle-CRC API.
- Installed NGP ReadMe's 7.5 changelog lists FFB fixes, pausing physics when the
  main loop fails to execute, FMOD 2.03.06 and ARM64 FMOD/telemetry integration.
  This is not an exhaustive proof of unchanged vehicle initialization or forces.
  The ARM64 helpers are Windows executables/DLLs, not iPadOS libraries.

Uncompared code and runtime behavior remain unverified. In particular, the new
main-loop pause behavior must be tested with authentic captures; it is not the
same thing as imposing a rendering-FPS threshold. Simply adding the candidate
hash would also be wrong because the current runtime parser uses fixed 7.4
decoder addresses, whereas the comparator locates candidate inputs by content.

Parent upstream notes independently describe this exact 779 candidate and an
84,084-step matching **drive2/attempt 5**, Ford Focus RS WRC 06 (RSF car 55), on
snow using 778 tables. That is upstream historical evidence, **not rerun here**,
and not a certification across vehicles, surfaces, damage and control states.

## Vehicle packages and CRC

The installed archive contains eight original-slot car folders. Inspection of
all three surface option files per car finds **no readable NGP headline specs**
and missing NGP torque/drivetrain keys in every slot. Stock cars can contain a
numeric CarId; that field alone proves nothing. No loose `Physics` directory,
RSF car package directory, loose LSP files or separate vehicle physics archive
was found. The RSF cache holds `audiobank.ini` and preview images. Additional
models, including Lada VFTS NGP, do not include a corresponding verified physics
package in the inspected installation. `Cars.ini` still selects stock models.

Therefore **no locally available Corolla or alternative vehicle can currently
be identified as genuinely 779-compatible**. The Focus RS WRC 06 / RSF 55 is a
useful first candidate because of upstream reference evidence, but its genuine
physics package and capture are not present here; it has not been substituted
for the Corolla. No car was labeled authentic based on its filename or preview.

Genuine script data can in principle be read without changing parameters: the
compared NGP key tables are equal and the existing original format parser knows
NGP torque/options fields. This establishes a plausible format path, not verified
loading of a genuine local 779 vehicle. RSF pack resolution is present in the
parent project; the current iOS descriptor backend deliberately supports only
the existing Corolla/Harwood profile, not arbitrary packs.

**Original CRC restoration is blocked, not implemented or bypassed.** Upstream
notes describe hashing loaded values against CarId, limiting an incompatible
car to reverse/neutral/first, and later randomized runtime checks that can abort
after tampering. No complete checksum algorithm, serialized field order,
normalization/initialization boundary, known-answer vectors or 779-specific
implementation was found in the source or SDK. Notes provide 778 function
addresses; they cannot be assumed to be 779 addresses. A file SHA-256 or the ZIP
CRC is not this loaded-value algorithm. No replacement checksum, fabricated
CarId, gear-limit bypass or parameter edits were introduced.

The new candidate remains rejected before boot, so it cannot reach an unchecked
7.5 driving path. The separately preserved legacy experiment is not relabeled as
CRC-valid. Restoring a safe rejection policy for a candidate is distinct from
reimplementing the original validator and its runtime behavior.

## Implementation results and baseline preservation

- Added app-owned `rerbr_sim::ngp_candidate`: read-only archive inventory, original
  parser-based per-surface evidence, exact source-file hashes, candidate binary
  comparison and explicit unsupported-runtime diagnostics. Every archive script
  is read through the original ZIP reader; ZIP CRC integrity is clearly separated
  from vehicle authentication. There is no DLL code execution.
- Added `ngp-candidate-audit` CLI and tests for repeatable comparisons/inventory,
  false labels/stock CarId, and rejection of the exact 779 binary while retaining
  the 778 baseline allowlist. No new third-party dependency was added.
- The existing input check delegates to that runtime policy. The only behavior
  change for 779 is an actionable refusal explaining the missing CRC/parity work.
  Existing 778 loading, descriptor selection, physics, transmission, rendering,
  cockpit, worker timing and audio mapping remain the regression path.
- Before changes, recorded hashes of **782 source files** and saved copies of
  touched baseline source/manifests under ignored
  `.validation/baseline-before-ngp779`. No imported-source exception or supported
  DLL hash was added. This is a local source checkpoint, not a commit or an IPA.

Reproduce the read-only audit (PowerShell, from this source directory):

```powershell
$env:CARGO_TARGET_DIR = Join-Path $PWD '.validation\target'
cargo run --offline --locked --release --features physics-probe --example ngp-candidate-audit -- 'C:\Users\adria\Downloads\RBR_NGP-7.4.778.507\Plugins\NGP.dll' 'C:\Richard Burns Rally\Plugins\NGP.dll' 'C:\Richard Burns Rally\physics.rbz'
```

Exit success means the research diagnostic completed, never that 7.5 is supported.
No vehicle is certified genuine by this command.

## Remaining blockers and exact next steps

1. Obtain an unmodified, provenance-identified genuine 779 vehicle package plus
   its matching model/config/setup dependencies. Prefer Corolla if available;
   otherwise investigate Focus RS WRC 06 / RSF 55 because upstream has a related
   reference. Validate it in a **separate Windows reference installation**, with
   original 7.5 checks active. Do not overwrite the user's live physics archive.
2. Recover/port the original loaded-value CRC and runtime revalidation from
   available legitimate source or documented binary behavior. Establish 779
   function locations, algorithm, field order and modes with known-answer
   captures. Include genuine, corrupted, wrong-CarId, post-init mutation and
   surface/setup cases. Until then, genuine 7.5 runtime loading remains blocked.
3. Capture identical initial state, complete setup, vehicle/archive/DLL hashes,
   inputs and original Windows 7.5 results. The parent harness expects FRM1
   physics plus RSS1 session/control/settings records and setup snapshots;
   generic telemetry or an RPL alone is insufficient for exact step parity.
   No matching capture, installed RBRRecomp harness plugin or configured reference
   environment was found here. SDK headers do not supply the checksum or recorder.
   The checked-in recorder build recipe depends on its configured MSVC 7.1/Wine
   toolchain; it cannot simply be invoked as a native iPad backend.
4. With those inputs, use parent `harness/rsf_compare.py SESSION --attempt N
   --sections physics --out REPORT` and the upstream gate matrix. Cover dry/wet
   gravel, tarmac, snow, manual/automatic/clutch/start logic, braking, uneven
   contact, damage and main-loop pause/resume. Reconcile every divergent field.
   The Python harness and its reference data are not currently configured here.
5. Only after CRC and numerical evidence pass, add a reviewed relocated immutable
   reader/version-specific runtime adapter, version-keyed table storage if needed,
   and explicit engine requirements on vehicle/session inputs. Then extend the
   descriptor backend for the verified vehicle. Do not silently route 779 data
   through an engine labeled 7.5 while retaining unverified 7.4 behavior.
6. Run ARM64/iPad fixed-step determinism and device driving tests. Windows FFB,
   service APIs, FMOD helpers and telemetry daemons require portable presentation
   implementations or separate host tooling; their binaries cannot run on iPadOS.
   No installed IPA replacement or Actions build is part of this investigation.

## Validation

- Formatting and all-feature/all-target Clippy with warnings denied pass.
- Public app suite: **72 passed**. Separate candidate policy/parser unit tests:
  **2 passed** (labels cannot enable a binary; stock CarId is not authentication).
- Private release suite: **11 passed**, including repeatable 25-range candidate
  comparisons, repeated 8-car/24-surface inventories, exact candidate refusal,
  original 778 acceptance, and existing driving, 60-second stability, contact,
  stationary/acceleration repeatability and full manual-shift regressions.
- All **623** imported provenance entries retain their original hashes or four
  pre-existing documented adaptations. Against the saved baseline, only three
  existing source files changed: Cargo example registration, adapter module
  registration, and the input-check diagnostic hook. New code and tests are
  separate files. No private game asset files were added to the source tree.
- The optimized original comparator completed successfully. A redundant slow
  debug comparator was stopped after the optimized result was available.
- No Windows NGP 7.5 driving comparison or iOS physics-parity validation has been
  performed here. No commits, pushes, Actions runs or IPA replacements occurred;
  the separate GitHub checkout remains clean.
