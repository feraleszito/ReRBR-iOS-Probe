# Integration review — 2026-11-16

Status: source review and local validation; no new IPA has been generated. GitHub synchronization,
commit, push, Actions dispatch and installation each remain subject to the owner's approval.

## Evidence and scope

Reviewed the integrated main tree, Qoder's `rerbr-ui` source and tests, Antigravity's
`scratch/ios-probe-work` and `scratch/dev-backup-audio-merge`, and Qoder's isolated
`ngp779-work/.validation/ngp-779-report.md` and `ngp-779-crc.md` in full. The owner confirmed that the
UI/audio reports were conversation handoffs, with no separate report confirmed on disk. Those
conversations are not available here; conclusions below come from source, comparisons and tests.

The clean GitHub clone remains at `3d8777d37514ae709139e777a5d2b77bbe1126de`. Its active
`.github/workflows/ios-probe.yml` is unchanged (SHA-256
`5fbb8318963d2fb6d62e91b4a06d70ce7384dcc206ad4440776b763db95996a4`). The established workflow uses
`macos-15`, Xcode 16.4, Rust 1.97.1, `PROBE_MODE`, the existing unsigned packaging script, and
separate IPA/checksum and diagnostic artifacts. No local Mac is required for that pipeline.

## Review findings and repairs

- Native audio referenced Rust exports absent from non-driving builds. Added a silent ABI for those
  modes and an initialization gate so RemoteIO starts only for driving modes.
- The audio callback could initialize shared state, grow stereo/mono/granular buffers and accept an
  unchecked native buffer layout. Preparation and bounded scratch storage now happen before playback;
  unsupported sizes/rates/layouts return silence. Panic recovery clears the output. Native setup stops
  on errors and disposes a failed AudioUnit. The 44.1 kHz interleaved client format is explicit;
  hardware conversion/route behavior still needs iPad testing.
- Physics telemetry used a blocking audio mutex. It now uses `try_lock`: contention can discard an
  audio update, but cannot make physics wait for the render lock. Audio still reads the original sound
  payload after stepping; it does not write simulation memory.
- Restart, return-to-menu, worker stop/error and terrain/camera failure could leave old audio playing.
  These paths now mute audio; each frame also follows the worker's paused state. Existing app lifecycle
  pause hooks remain in place.
- The private bank test used to return success on parse failure. It now requires an explicit private
  fixture and fails on a load error. Added callback contention/bounds, preallocated buffer RPM-sweep,
  and private physics-with/without-audio regression checks.
- Cabin configuration now uses the existing original format reader's values and defaults. Malformed
  banks and nonfinite cabin settings are rejected into the existing silent fallback.
- Qoder's menus share row geometry with hit testing; unavailable imports are disabled, descriptor
  validation gates runtime selection, and only the current Corolla/Harwood preset is supported.
  Settings use the existing Documents preference file. The new Canvas was unconditional in older
  benchmark modes and failed `clippy -D warnings`; it is now restricted to driving builds.
- Source preflight now checks the fixed bundle identifier and required native audio frameworks.
  The existing CI test selection now includes the new content descriptor crate.

The audio port is an engine voice with cabin filtering, not the complete original sound system.
Gear state is recorded but no transmission whine is synthesized; tyres, impacts, turbo, exhaust,
reflections and codriver output are not integrated here. Granular synthesis derives from the parent's
GPL `crates/rbr-sound/src/engine.rs` (SHA-256
`2f2dfa277348eb743457accf5a19dbd401e5202b56101f1bb0a51b21a89e4b4f`). Its soft knee, gain smoothing
and independent RNG are presentation choices already described upstream. No audible or bit-exact
audio parity claim is made. Callback scheduling and contention dropouts require real hardware.

## NGP research accepted into the development tree

Accepted only the CRC-32 accumulator and five tests, read-only per-surface script/CarId evidence,
the car-pack inspector, diagnostic output and example registration. Research remains disconnected
from driving initialization and the supported DLL allowlist.

Independently compared the 40 stored `.b78`/`.b79` normalized function pairs: 40 match. This checks
Qoder's comparison artifacts, not an independent reconstruction of every binary instruction or the
contents reached through absolute/indirect references. Address normalization and matching walk code
do not prove equal loaded values or physics behavior. The three stored Focus CarIds remain references,
not reproduced outputs. No private package, disassembly or extracted asset was copied into public
sources. The original technical reports remain in the isolated research workspace.

NGP 7.5 stays blocked until an authentic loaded-object CRC walk and independent original-engine
captures establish parity. No CRC substitute, version bypass or drivetrain workaround was added.
The current stock Corolla baseline still reports 34 missing NGP keys and an unchecked vehicle CRC;
preserving its behavior does not certify that package as authentic NGP physics.

## Validation

Local results (all commands used the pinned toolchain and lockfile):

| Check | Result | Local evidence under `.validation/` |
| --- | --- | --- |
| `cargo test --offline --locked --workspace --all-features` | 1,106 passed, 0 failed, 22 ignored | `lead-workspace.log` |
| Final probe/sim/content tests, all features | 120 passed, 0 failed, 22 ignored | `lead-tests-final.log` |
| Default triangle tests | 2 passed | `lead-default.log` |
| All nine build modes, host Clippy library with `-D warnings` | PASS | `lead-mode-*.log` |
| All-feature/all-target host Clippy with `-D warnings` | PASS | `lead-clippy-final.log` |
| `cargo check --locked --target aarch64-apple-ios --all-features --lib` | PASS; Rust type checking only | `lead-ios-rust-check.log` |
| Explicit private bank + audio/physics regression | 2 passed; identical sampled trajectories over two 1,440-step runs | `lead-private-audio.log` |
| Exact installed 779 rejected / reference 778 accepted | PASS | `lead-ngp-gate-corrected.log` |
| Source/package preflight | PASS; 623 upstream hashes, retained four adaptations, no private assets in publishable source | `lead-preflight.log` |
| Formatting | PASS | `lead-fmt-final.log` |
| Normalized research walk bodies | 40/40 stored `.b78`/`.b79` pairs byte-identical | Qoder `crc-analysis/closure2/` |
| Upstream bytes, hash/patch manifests, Cargo.lock versus entry snapshot | No changes | `lead-results.json` |

Tests requiring original recordings/vectors may skip internally when their data is unset; the counts
above do not mean an original-engine oracle session was rerun. The stored disassembly pairs were
independently compared, but DLL disassembly/normalization was not independently regenerated; absolute
data contents and indirect calls remain outside that comparison. An initial DLL gate attempt used a
reference root without `Plugins/`; it failed with file-not-found, then passed using the existing
private baseline fixture. The iOS Rust check first needed missing Apple-target dependencies fetched
from the unchanged lockfile; no Apple SDK or linker was used.

Windows checks cannot establish
Objective-C compilation, Apple linking, Metal appearance, actual audible output or iPad performance.
The full Xcode build, physical-iOS ARM64/Mach-O/unsigned IPA inspection and physical testing remain
pending the approved CI run. No new IPA exists to scan or install yet.

For the first approved iPad run, back up Documents, then check all menu pages, landscape rotations,
steering calibration and multitouch pedals/shifts; test volume 0/100%, chase/cockpit audio, pause,
restart, return to menu, lock/background/resume and audio interruption/headphone routes. Verify that
paused/error states are silent, the old Documents files remain present, and the app still reports
uncertified physics. Retain `rerbr-probe.log` and note audible dropouts, latency, frame time and memory.

Audio additionally requires private `Documents/RBR/Audio/Cars/Cars.ini` and the `.eng` bank named by
that file's current vehicle mapping (the audited mapping loads `subaru.eng`). Existing minimal
physics/terrain imports may omit these audio files. Transfer them privately through the established
Documents workflow after backing up; retain the existing physics archive and 7.4 DLL. Missing audio
files intentionally produce silence and an audio diagnostic. None of these files belongs in Git or
the IPA.

## Safe synchronization, build and install sequence

1. After explicit synchronization approval, recheck the clone is clean and preserve its HEAD and
   active workflow hash. Use `.validation/lead-sync-plan.json` as a reviewed file/hash inventory.
   Refresh it after any source edit. Copy only listed changed/new source files; never mirror/delete
   the clone, copy `.validation`, `target`, `build`, private Documents or the surrounding parent tree.
   Preserve `.github/workflows/ios-probe.yml`; the source template is not its replacement. Review the
   diff and rerun preflight, formatting, relevant tests and Clippy in the clone before approval to commit.
2. After explicit commit/push approval, publish the reviewed source revision to
   `feraleszito/ReRBR-iOS-Probe`. Record its full SHA. Do not rerun a job for an older revision.
3. After explicit build approval, open the existing **ReRBRProbe unsigned iPadOS** Actions workflow,
   select that branch/revision and `build_mode=physics-stage`. Its existing invocation is
   `PROBE_MODE=physics-stage bash scripts/build-ios.sh` on the pinned macOS runner. It runs source
   checks, host tests, iOS Clippy, release static-library compilation, Xcode and package inspection.
   If Xcode 16.4 is unavailable or a check fails, stop and review that failure; do not silently change
   the runner or reuse an old artifact.
4. Download the artifact for that exact SHA/run: `ReRBRProbe-unsigned-physics-stage-<run number>`.
   Extract it and compare `Get-FileHash -Algorithm SHA256 ReRBRProbe-unsigned.ipa` with `SHA256SUMS`.
   Review all build logs and run `python scripts/verify.py --ipa <path>` against that artifact. The
   package must retain `org.rerbr.probe`, iPad family 2, iPadOS 18 minimum, physical-iOS ARM64, and
   contain no private game resources. Retain the previous known-good IPA and its checksum separately.
5. Before any separately approved installation, back up the complete current app Documents to a
   private location and verify the backup, including `RBR`, `Harwood`, preferences and logs. Record
   the current signed bundle/application identifier and signing team. In the established iLoader
   setup, preserve those identities and update the existing installation. Do not uninstall, delete
   Documents, change the bundle ID or accept an erase/remove prompt. If the signing tool changes the
   identifier/team or cannot update in place, stop; data retention has not been established.
6. On iPad mini A17 Pro verify existing Documents before driving. Check menu navigation, both landscape
   orientations, safe areas, concurrent pedals/shifts, calibration, volume/mute, pause/resume, restart,
   background/foreground, audio interruptions, cockpit/chase filtering and speaker/headphone routes.
   Check missing/malformed audio produces silence without losing driving. Capture logs and assess
   temperature, memory, frame times and audible dropouts. This is a regression hardware test, not
   NGP 7.5 certification.

No commit, push, workflow run or installed-app replacement occurred during this review.
