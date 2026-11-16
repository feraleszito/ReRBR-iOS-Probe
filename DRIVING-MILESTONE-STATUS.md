# Driving milestone: implementation and local physics audit

2026-11-16. Source work remains in the extracted development tree, not the clean
Git checkout. No commit, push, Actions run, IPA build or device installation.
Private inputs were read only. No further Content Manager features were added.

## Status before this pass

| System | Implemented | Partial / awaiting verification | Not implemented |
| --- | --- | --- | --- |
| Physics fidelity | Original ported equations, original fixed 720 Hz timestep, dedicated simulation worker, original collision boot and read-only contact diagnostics; deterministic stationary and acceleration/braking checks | Corolla data compatibility, force/material parity, suspension behaviour and the reported floaty feeling remain unvalidated against original-game recordings and the iPad | A demonstrated fix for the floaty physical behaviour; complete compatibility CRC parity |
| Transmission | Manual R/N/1–6, original gear ratios and clutch, touch shift controls, actual gear/RPM display; deterministic gear acceptance, reverse motion and a loaded upshift | Complete loaded gearbox/engine-braking validation and device usability | Working optional automatic transmission with verified vehicle thresholds |
| Cockpit | Original Corolla interior geometry/material loading; wheel animation around original pivot driven by simulated steering-rack angle | Metal rendering, clipping and rotation direction require physical-device inspection; missing private textures prevent full cockpit on an incomplete installation | Animated driver hands, instrument needles and advanced reflections |
| Engine audio | Read-only original Car6 mapping and ENG sample/marker audit | Car6's existing mapping selects a Subaru-named ENG bank; this is the installed mapping, not proof of Corolla-specific recordings | iOS engine playback/output, RPM/load mixing, seamless loop playback and device listening validation |
| HUD | Compact edge controls, actual speed/gear/RPM, optional FPS and diagnostic overlay | Device safe-area, legibility and touch-comfort review | Final device visual approval |
| Main menu/session | Drive, Settings, Controls, About; pause/resume, restart/reset to start, main-menu return, saved steering/HUD settings | Device navigation and lifecycle review | Roadside recovery and functioning audio/automatic-transmission settings |

Source implementation does not constitute device validation. The cockpit, HUD
and menus cannot be called visually complete from Windows CPU tests alone.

## Local installation findings

- `C:\Richard Burns Rally\Plugins\NGP.dll` reports **7.5.779.508
  (2025-03-05)**. Its SHA-256 is
  `cf351cdf99f880631ed2cb1d9b6b5c9db7d7c233ad905740393c37df967801ec`.
- That installation's `physics.rbz` declares `PhysicsRevision_NGP 779`.
- The separate `RBR_NGP-7.4.778.507` package DLL has the supported SHA-256
  `95b6dbf5e8957357d857a8e919cff177a6bfb832f5324da90467d43732edaaa4`;
  its outer archive declares revision **778**. Existing isolated simulation
  fixtures use this package, not the installed 7.5 DLL.
- There is no loose `C:\Richard Burns Rally\Physics` directory. The inspected
  installation did not contain a separate Corolla NGP physics archive or a
  corresponding RSF car package. `Cars/Cars.ini` Car06 selects the original
  `Cars\Corolla\Corolla.sgc` model/config; this identifies the model, not physics
  compatibility.
- The Corolla `common.lsp`, `gravel.lsp`, `r_gravel.lsp` and
  `setups/d_gravel.lsp` entries are identical between the installed archive and
  the verified 7.4 outer archive (entry text compared independently of ZIP
  compression, case and path separators). They contain no NGP torque-table,
  `MaxTorque_NGP`, `Features_NGP`, or automatic gear-threshold keys. This is
  stock-format Corolla data retained in the outer package, not an NGP Corolla
  conversion. Replacing the global physics archive/DLL does not convert it.

Entry UTF-8 text SHA-256 values (not whole-archive hashes):

| Entry under `Physics/t_coroll/` | SHA-256 |
| --- | --- |
| `common.lsp` | `27dee2411d5b6a0308e55a79c0e1a90108fc4ac48a6429268083315af9794af8` |
| `gravel.lsp` | `4191360b4be23af7baa92888d3c1563e61ec355dd9e43446706b0d47748de840` |
| `r_gravel.lsp` | `28647d59c559f9a72a080b5cd3c23f6976fd0df9cc3f7cb5496d0a1c0f067693` |
| `setups/d_gravel.lsp` | `4cff00d59ee7caa30767462271cde8219577e3419700bb80d88528d829ed2934` |

Important correction to earlier reports: a verified **DLL**, successful boot,
six-gear acceptance or repeatable simulation does **not** establish a compatible
NGP vehicle. The frozen port explicitly omits NGP's car-file CRC check
(`upstream/rbr-physics/src/ngp/layout/car_init.rs`, header). In the original,
failure limits the gearbox definition to three indices (reverse, neutral,
first). The original NGP ReadMe also documents reverse/first-only operation for
non-genuine NGP cars. Consequently previous gear-acceptance results are control
integration evidence only. The descriptor preset's earlier use of “verified”
must be read as path/profile validation, never certification of its physics.

Missing requirements for authentic NGP Corolla validation:

1. A genuine Toyota Corolla WRC physics conversion explicitly compatible with
   NGP 7.4.778, including common physics, relevant surface definitions, options,
   setups, correct torque/engine/drivetrain and suspension data, and required
   dependencies. File provenance/version and original-game compatibility must
   be demonstrated; an arbitrary replacement file is insufficient.
2. Verified original automatic upshift/downshift configuration if automatic
   operation is expected (`Gear<N>Upshift` / `Gear<N>Downshift`, with the
   appropriate original gear indexing). Current effective VCU upshift thresholds are zero.
   Manual shifting does not require invented automatic thresholds.
3. Matching original-game recordings/setup/input data for numerical comparison,
   followed by physical-iPad driving/cockpit/audio validation. Determinism alone
   cannot diagnose the floaty feeling or establish force parity.

## Changes in this pass

- Fixed pause/resume scheduling so paused wall time is excluded. Controls and
  pause state are reread at every fixed-step boundary, including catch-up.
- Removed the 150 ms backlog pause policy. Backlog retains fixed-duration
  original physics steps regardless of rendering performance; it is not skipped
  or converted into a larger physics timestep. On an overloaded device the
  simulation may lag wall time until it catches up. Pause/shutdown remain
  responsive between steps. No simulation-rate or rendering-FPS target was
  changed to suppress features.
- Added a read-only audit of the exact physics/option script trees selected by
  the original boot resolver. It reports absent NGP torque-table/drivetrain
  prerequisites and explicitly states that key presence cannot certify CRC or
  original-game parity. Session, menu and optional diagnostics expose the
  result; no private parameters or physics equations are rewritten.
- Clarified the input-check log: its verified NGP DLL is not a certified vehicle.
- Expanded manual-transmission integration coverage to all six forward gears,
  shifting up and down under continuous throttle through the original payload.
  Both independent runs produce identical samples; actual gear, RPM, speed,
  throttle and clutch are read from the simulation. This is an input-path test,
  not a claim that aggressive downshifts are mechanically safe or that the
  vehicle is NGP-compatible. Existing reverse/neutral/clutch and automatic-aid
  gating tests also pass. No automatic RPM thresholds were supplied.

The existing vehicle/stage descriptors continue to select the boot, gearbox,
model, cockpit and audio mapping. No importer/catalog/format expansion was added.
Future Content Manager work still needs Files import, package manifests and
dependencies, transactional private installation, discovery/selection, supported
format adapters and actionable asset errors. Those remain outside this milestone.

## Validation

Formatting and all-feature/all-target Clippy with warnings denied pass. The
all-feature public app suite passes (72 tests; private tests are explicitly
ignored there), as does the separate simulation-audit parser test. Fixed-step
scheduling tests cover pause/resume and 500 ms of backlog without dropping steps.

All 623 imported provenance entries match the original manifest or the four
pre-existing documented adaptations; no new exceptions were introduced. The
original flat memory-host reference also matches. No private game asset files
were found in the source tree, and the separate Git checkout remains clean.

Private release-mode regression: **10 passed, zero failed**, using the existing
isolated 7.4.778 fixture. This includes original interior/pivot loading, actual
physics-driven steering angle, touchscreen acceleration/braking, deterministic
cornering/contact checks, two 60-second stability runs, original stationary and
acceleration repeatability, all-gear/clutch/reverse checks, the new loaded forward
shift sequence and automatic-aid gating. The installed archive and DLL retain
their original hashes after the audit. These are integration regressions with
uncertified vehicle data, not original-game numerical parity proof. iOS
compilation, Metal inspection, listening tests, original-game oracle parity and
physical-device performance remain pending; no IPA was built. The Python
`scripts/verify.py` runner is unavailable on this Windows host; the source-hash
and asset checks above were performed directly with PowerShell.
