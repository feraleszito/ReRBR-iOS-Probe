# ReRBR Content Manager — architecture foundation

Physics compatibility clarification: [DRIVING-MILESTONE-STATUS.md](DRIVING-MILESTONE-STATUS.md)
documents the subsequent local installation audit. “Verified preset” below means
descriptor paths/profile checks; the available stock-format Corolla physics are
not certified compatible with NGP 7.4.778.

2026-11-16. This is preparation for a future Content Manager milestone, not an
importer or a claim that additional cars/stages already work. The current
milestone still prioritizes validating the Corolla's original physics,
transmission, cockpit and authentic engine audio. Audio playback remains
unfinished; this change routes its existing bank audit through the vehicle
descriptor rather than pretending that an audio output backend exists.

## Completed in this pass

Added `rerbr-content`, a small GPL-3.0-or-later local crate with **no external
dependencies, assets, filesystem operations or physics/rendering dependencies**.
Both the app and simulation adapter use the same owned Rust descriptor types.
They are runtime values, not a new collection of compile-time-only enum variants
requiring one binary variant per future car or stage.

| Type | Current information |
| --- | --- |
| `VehicleDescriptor` | Stable content ID/name, original physics slot, tyre type, forward gear count, model/config/shader paths, required/optional texture dependencies and original audio mapping path |
| `StageDescriptor` | Stable ID/name, original map ID, file stem, Documents-relative Maps root, weather selection and expected terrain block count |
| `Selection` | The vehicle and stage request shared across loading and session restart |

The verified Corolla/Harwood preset is defined centrally. It preserves Car6,
map 20, gravel tyre type 4, six forward gears, dry O weather and the existing
private layout. There is no new ratio table or replacement physics configuration.
The original gearbox definition is read after boot and its gear count must agree
with the descriptor. Original ratios, physics equations and source integrity
manifests are unchanged.

Selection-aware entry points now exist and are used by the current production
path, including the default wrappers:

- `car::prepare_selected` and `car::load_vehicle`: profile-bound model, INI,
  shader and DDS reads; `Cars.ini` references must match the selected profile.
- `harwood::stage::load_selected`: explicit stage request and compatibility
  check; terrain block validation and COL selection use its descriptor.
- `inputs::Paths::for_selection` and `inputs::check_selection`: original private
  roots and required physics files, with selected model/stage file dependencies.
- `Session::boot_selected`: original map/car/tyre/weather configuration goes
  through the existing original boot interface; no physical state is synthesized.
- `drive::collision_triangles_for`: independent collision diagnostic uses the
  selected stage's file stem.
- `audio_audit::inspect_vehicle`: resolves the selected original slot through
  its audio `Cars.ini`, then parses the assigned ENG bank. No hardcoded Subaru
  substitution or fabricated audio mapping is added.

`Prepared` carries its `Selection` through GPU installation. Installation checks
that prepared vehicle metadata matches the request and clears a cached old
frame. The driving runtime retains the selection, uses it on reset/restart, and
uses descriptor names/gear count in the main menu and HUD. Touch gear limits
come from the validated vehicle descriptor, with the actual original gearbox
count checked separately at boot. Cockpit texture reads use that same vehicle
profile. Default stage stems/counts are shared with the existing asset probe.

Existing no-argument/default APIs remain as wrappers so diagnostics and previous
build modes do not need a simultaneous rewrite. The original fixed baseline
physics probe intentionally remains a reproducible Corolla/Harwood diagnostic.

## Current support boundary

Schema/path checks and **backend compatibility** are separate. Descriptors use
owned strings/lists so a future validated manifest/catalog can construct them at
runtime. Today, `validate_current_backend` explicitly accepts only the entire
known Corolla/Harwood profile. Unknown or mismatched profiles return a clear
unsupported-profile error before private-file reads. Merely editing a label,
slot or model path does not enable untested content or mix its presentation with
another car's physics.

This compatibility gate is temporary and centralized. The future Content Manager
must replace it with evidence-based format/backend capability validation; it
must not add a new hardcoded ID branch for every imported package.

Relative-path schema checks reject absolute/drive/UNC paths, traversal, empty
components and control characters. Car reads still apply their canonical-root
containment and file-size checks. These are useful primitives, **not a complete
package installer security policy**. No ZIP extraction, signature/content
certification, symlink installation policy or dependency catalog is claimed.

## Remaining for the Content Manager milestone

1. Define/version a compatible package manifest and dependency model; start with
   a small, audited set of existing formats rather than claiming every mod works.
2. Add the iPadOS Files picker/import lifecycle, bounded staging/extraction,
   canonical path/collision/link handling, dependency verification and clear
   diagnostics. Validate before making a package available for selection.
3. Implement private transactional installation/rollback, an installed-content
   catalog, automatic discovery and persisted stable selected IDs. Define isolated
   package roots/overlays while preserving the legacy Documents layout.
4. Generalize the remaining Harwood-specific terrain/texture alias/coverage
   assumptions and car-specific gravel wheel/material/camera conventions into
   actual format adapters. `load_selected` is an interface and guarded current
   implementation, not proof that arbitrary stages already decode correctly.
5. Resolve original registry/physics-slot mappings and package-local/shared
   dependencies. Upstream boot still reads original-format registries and setup
   data; a descriptor alone does not assemble a compatible mod's physics pack.
6. Add installed car/stage selection to the main menu and connect it to validated
   load requests, transactional resource swaps, complete audio and lifecycle
   management. The current menu intentionally offers only the verified preset.
7. Validate additional compatible packages against the original simulation and
   on the physical iPad. After format adapters/catalog imports exist, additional
   compatible packages should be data installations, not IPA rebuilds.

No importer, installed-content enumeration, new directory layout, selectable
unvalidated content or private-asset migration was added now. No private assets
were copied into either source tree or bundled. `org.rerbr.probe`, native Xcode
files, build modes and the Actions/IPA workflow remain unchanged. Cargo manifests
and lockfile include only the new local descriptor crate; `scripts/verify.py`
was updated to check that dependency explicitly and keep it free of dependencies.

## Validation

| Check | Result |
| --- | --- |
| All-feature public app tests | PASS, 70 |
| Descriptor crate tests | PASS, 3 |
| Default triangle/content tests | PASS, 2 |
| Release all-feature private suite | PASS, 16 |
| Original partial-stage car benchmark | PASS, 1 |
| Clippy with `-D warnings` | PASS: all features, descriptor crate, partial physics-drive and car benchmark configurations |
| Rust formatting | PASS |
| Physics integrity | PASS: 623 original source/reference entries and existing patch exceptions; original flat-memory reference retained |
| Original stationary and acceleration/braking fingerprints | UNCHANGED |
| Diff/secret-pattern/private-asset scans | PASS |

The tests cover descriptor schema and profile consistency, rejection before
file access, unchanged private roots and the existing private vehicle/stage/
physics regressions. The Car6 audio mapping and original bank parse still pass;
that is not an audible playback test. These checks do not establish
physical iPad performance, audible output, arbitrary package compatibility or
complete original-game runtime parity.

Logs are local ignored `.validation/content-*` files. The Python source/IPA
verifier was updated for the new local crate but cannot run on this Windows host
without a working Python interpreter. Its source integrity, asset, dependency
and native/build-contract checks were performed separately. Apple compilation,
IPA verification and physical-device tests remain pending. No new private
fixture or game asset was created, copied or exported.

## Files in this architectural pass

```text
AGENTS.md
CONTENT-MANAGER-ARCHITECTURE.md
EXPANDED-EXPERIENCE.md
Cargo.toml
Cargo.lock
scripts/verify.py
src/lib.rs
src/asset_probe.rs
src/car/mod.rs
src/car/cockpit.rs
src/car/runtime.rs
src/car/tests.rs
src/driving/controls.rs
src/driving/mod.rs
src/driving/worker.rs
src/harwood/gpu.rs
src/harwood/stage.rs
tests/content.rs
vendor/rerbr-content/Cargo.toml
vendor/rerbr-content/src/lib.rs
vendor/rerbr-sim/Cargo.toml
vendor/rerbr-sim/src/audio_audit.rs
vendor/rerbr-sim/src/drive.rs
vendor/rerbr-sim/src/inputs.rs
```

The earlier cockpit/menu/transmission changes remain in the working source; their
reports describe those changes. Nothing was synchronized to the GitHub clone,
committed, pushed, sent to Actions or installed on the iPad in this pass.
