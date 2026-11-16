# Validation record

Validation performed on Windows x86_64 with Rust 1.97.1. These results apply to source
checks and the Windows host, not to the iOS application or Metal backend.

| Check | Result |
| --- | --- |
| `cargo check --offline --lib` | PASS on Windows; generated the independent lockfile using the parent's existing dependency versions as a seed |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --offline --locked --all-targets -- -D warnings` | PASS on Windows |
| `cargo test --offline --locked --test shader` | PASS: one Naga WGSL validation test; no GPU execution |
| `cargo metadata --offline --locked --no-deps --format-version 1` | PASS: workspace root is `ios-probe`, with only `rerbr-probe` as a member |
| Runtime dependency versions | winit 0.30.13, wgpu 30.0.1, futures-executor 0.3.34; naga 30.0.1 for testing |
| Plist, launch storyboard and shared scheme XML | Parsed successfully with PowerShell's XML parser |
| Xcode project object IDs | Unique definitions and no unresolved object references in a PowerShell structural check |
| Workflow activation | Owner authorized creation of `.github/workflows/ios-probe.yml`; its SHA-256 matches the template |
| Shell line endings | `build-ios.sh` contains LF line endings |
| Workflow trigger | Manual `workflow_dispatch`; no push or pull-request triggers |

The commands used a private copy of the existing Cargo index/package cache and build
outputs under the ignored `ios-probe/.validation/` directory. No dependencies were
downloaded and no parent source/configuration files were edited. The only new file
outside this directory is the explicitly authorized workflow. No Git commit was made:
Git is not available in this environment and the provided checkout has no `.git` folder.

Not run here:

- `scripts/verify.py` itself: Python is not available as a usable interpreter in this
  environment. The plist/XML/object-reference subset was checked with PowerShell.
- Bash syntax/execution and Xcode `plutil`/project validation: no local macOS/Bash tools.
- iOS target compilation and native linking: only the Windows Rust target is installed;
  no Xcode or iPhoneOS SDK is available on this Windows host.
- IPA generation and executable/package inspection: no IPA has been generated yet.
- Device installation, visual capture, touch, rotation, suspend/resume and performance.
- The parent repository's `scripts/rust-check.sh`: this experiment is deliberately
  independent and does not change game code.

The workflow is prepared to run the remaining build/package checks on macOS. A first
successful Actions run is required before calling the Apple build successful. Hardware
checks in `README.md` must then be performed on the iPad mini A17 Pro.

## Standalone repository path correction

The owner reported that the first Actions run failed before invoking the build script:
`bash: ios-probe/scripts/build-ios.sh: No such file or directory`. Their public
`ReRBR-iOS-Probe` repository contains the probe files at its root.

Both workflow copies now use root-relative script, cache and artifact paths. All four
Cargo commands in the build script explicitly select the manifest beside `scripts/`.
The workflow checks for the standalone input files before installing Rust or caching.
The Xcode and Python verifier paths already resolve relative to the probe and needed
no changes. `Cargo.toml`, `Cargo.lock`, Rust sources and Xcode files were not modified.

Windows checks for this correction:

| Check | Result |
| --- | --- |
| Standalone relocation | Copied Cargo files, scripts, sources, tests and Xcode files into an ignored `.validation/root-layout-check/` directory, without an `ios-probe` wrapper |
| Relocated `cargo metadata --offline --locked --no-deps` | PASS: workspace and target resolve under the relocated root; only `rerbr-probe` is a workspace package |
| Workflow copies and root paths | PASS: SHA-256 matches; build uses `scripts/build-ios.sh`, cache uses `. -> target`, and IPA/checksum/logs use `build/` |
| Script and Xcode paths | PASS: four explicit Cargo manifest arguments; Xcode's `../target/.../librerbr_probe.a` resolves to the relocated root's `target/`; shader and verifier source paths exist |
| Relocated plist/storyboard/scheme | PASS: XML parsing with PowerShell |
| Shell line endings | PASS: LF |

No Rust or shader implementation changed, so their earlier checks were not repeated.
These are path/metadata checks, not a Bash, Xcode, IPA or hardware test. The corrected
workflow must be copied to the standalone repository, committed, pushed and run again.

## Static library linker search correction

The owner reported that the next Actions run reached Xcode and failed with
`ld: library 'rerbr_probe' not found` (exit 65). The available project referenced
`../target/aarch64-apple-ios/release/librerbr_probe.a` in its Frameworks link phase,
but did not declare `LIBRARY_SEARCH_PATHS`. The reported error is consistent with
Xcode linking by name (`-lrerbr_probe`) without the Cargo output directory in the
linker's search paths. The supplied error excerpt does not independently establish
whether the archive existed in that run; the new build guard checks this explicitly.

Changes:

- Xcode now defines `RUST_LIBRARY_DIR` and adds it to `LIBRARY_SEARCH_PATHS`.
- The script derives the expected archive path from its own `CARGO_TARGET_DIR`, the
  `aarch64-apple-ios` target and the release profile. It passes that same absolute
  directory to Xcode as a build setting override.
- Before invoking Xcode, the script rejects a missing/empty archive and uses `lipo`
  to inspect it and verify ARM64. The archive path/architecture and effective Xcode
  build settings are saved as `build/rust-library.txt` and `build/xcode-settings.txt`.
  Existing workflow globs already upload these files, so neither workflow was changed.
- `cargo rustc --print native-static-libs` is retained. This print mode performs
  compilation when used to produce a static library; it is not itself evidence that
  compilation was skipped ([Rust documentation](https://doc.rust-lang.org/rustc/command-line-arguments/print-options.html#native-static-libs)).

Windows checks for this correction:

| Check | Result |
| --- | --- |
| Relocated `cargo metadata --offline --locked --no-deps` | PASS: the copied standalone root resolves `rerbr-probe` and its own target directory |
| Cargo/Xcode directory resolution | PASS: the script's expected release directory and Xcode's `SRCROOT/../target/aarch64-apple-ios/release` normalize to the same location |
| Search path wiring | PASS: project adds `RUST_LIBRARY_DIR` to `LIBRARY_SEARCH_PATHS`; script supplies the absolute override |
| Early archive diagnostics | Source inspection confirms missing/empty-file rejection and ARM64 inspection precede Xcode; effective settings use the same argument array as the build |
| Relocated plist and shared scheme | PASS: XML parsing with PowerShell |
| Line endings and diagnostic upload paths | PASS: shell script has LF; existing `build/*.txt` and `build/*.log` globs cover the added logs |

Only `scripts/build-ios.sh`, `xcode/ReRBRProbe.xcodeproj/project.pbxproj`, `README.md`
and this validation record changed. Cargo files, dependencies, Rust sources, workflows
and the original reRBR engine were not modified. No iOS compilation, `lipo`, Xcode
link, IPA creation or hardware execution was performed locally. The fix still requires
a new Actions run using the updated files; iOS build success has not been confirmed.

## lipo argument order correction

The owner reported that the third Actions run found an 85 MB `librerbr_probe.a` and
`lipo -info` identified it as ARM64, then `-verify_arch` interpreted the archive's
path as an architecture name. The command had the input file after the architecture
list. Apple's [lipo manual](https://github.com/apple-oss-distributions/cctools/blob/main/man/lipo.1)
requires input files before `-verify_arch`, which consumes all following architecture
arguments. The corrected call is:

```sh
xcrun lipo "$RUST_STATIC_LIBRARY" -verify_arch arm64
```

ARM64 validation remains mandatory: a failing `lipo` still stops the script through
`set -euo pipefail`. The other `lipo -info` and `lipo -archs` calls are unchanged.

Windows source/path checks passed: the corrected argument order is present, the old
call is absent, fail-on-error remains enabled, and the script has LF line endings.
For a standalone `ReRBR-iOS-Probe` root, Cargo's target/release directory and Xcode's
`SRCROOT/../target/aarch64-apple-ios/release` resolve to the same directory; the script
still supplies that absolute `RUST_LIBRARY_DIR` to Xcode. No Cargo, Xcode, workflow,
dependency or original engine changes were needed.

Only `scripts/build-ios.sh` and this validation record changed. No local execution
of `lipo`, Xcode or the IPA was possible on Windows. The reported ARM64 archive is
evidence from the owner's run, not a successful IPA build. A fresh Actions run and
subsequent device installation/rendering checks remain required.

## First camera/coordinate integration batch

The owner reports that the preceding triangle build now runs on their iPad mini A17 Pro,
including animation and touch-driven background changes. This report establishes the
baseline; the checks below were performed locally on Windows, not on the iPad.

Added the optional local `vendor/rerbr-render-core` workspace package and the `rerbr-3d`
feature. The app's default features remain empty and its default workspace member is
the app itself. This feature only compiles the imported mathematics; it does not change
the triangle or add a scene, mesh, shader or drawing pass.

The original `coords.rs`, `camera.rs`, `triple.rs`, GPLv3 license text and formatting
configuration were copied byte-for-byte and documented with hashes in `UPSTREAM.md`.
The additional crate scaffolding uses the upstream GPL-3.0-or-later package license.
The mathematical triple-screen types are a dependency of the original camera API,
not an implementation of additional iPad windows. No game-system crate is imported.

Before editing the preflight verifier, the owner was informed that its existing closed
dependency list would reject the newly authorized library. Only `scripts/verify.py`
was adjusted to allow the exact local optional dependency, check the disabled-by-default
feature and validate the provenance hashes. The workflow and `scripts/build-ios.sh`
were not changed.

| Windows check (Rust 1.97.1, existing private offline Cargo cache) | Result |
| --- | --- |
| `cargo check --offline --lib --features rerbr-3d` | PASS; lockfile added the local package and pinned glam 0.33.12 |
| `cargo test --offline --locked --workspace --all-features` | PASS: 21 upstream math tests, 2 feature-gated integration tests, 1 unchanged triangle shader test; 24 total |
| `cargo test --offline --locked -p rerbr-probe --no-default-features` | PASS: triangle shader test; optional math integration tests disabled |
| `cargo clippy --offline --locked --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo clippy --offline --locked -p rerbr-probe --all-targets --no-default-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS, including unchanged imported sources under their original formatting configuration |
| Import identity and provenance | PASS: five copied files match their originals and all five documented SHA-256 values |
| Dependency selection | PASS: default normal dependency tree contains only futures-executor, wgpu and winit; the optional tree adds rerbr-render-core and glam |
| Standalone-root relocation | PASS: copied sources/manifests/vendor package into an ignored directory with no ios-probe wrapper; `cargo metadata --offline --locked --no-deps --features rerbr-3d` resolves that root, both workspace packages and the app-only default member |

The two new mathematical tests exercise near/middle/far reverse-depth ordering and an
orbit camera's target/eye projection across portrait and landscape aspect ratios. They
open no GPU device, window or commercial data file. The original triangle implementation,
shader, touch code, native entry, Xcode project, workflows and iOS build script were not
edited. The original reRBR engine and parent workspace files were not edited.

The updated Python preflight itself was not executed locally: no usable Python
interpreter is available in this Windows environment. The provenance regex and all five
hashes were checked with PowerShell instead. All Cargo commands used `.validation/`
for cache, build and temporary files and required no dependency downloads.

The existing workflow still builds only the default triangle and does not enable the
new feature or run the imported core's tests. Neither the updated default package nor
the optional feature has been built for iOS in this editing environment. A new Actions
default build and a later explicitly enabled Apple-target feature build are required
before making new iPadOS compatibility claims.

## Optional-feature Actions selector

The owner reports that Actions succeeded with the vendored library present in the
default triangle configuration. This does not establish that `rerbr-3d` was enabled
in that build.

Both workflow copies now declare a manual `build_mode` choice: `triangle` (default)
or `rerbr-3d`. The value is passed as the build step's `PROBE_MODE` environment variable,
not interpolated into shell commands. The script defaults to `triangle`, whitelists
the two values and rejects unknown values before invoking the SDK tools.

The selected Cargo argument array always includes `--no-default-features`; only
`rerbr-3d` appends `--features rerbr-3d`. Nonempty arrays avoid macOS system Bash 3.2's
empty-array/nounset issue. The same arguments reach the host tests, target Clippy,
release static-library compilation and dependency-tree diagnostic. In feature mode,
host tests select the complete workspace; default mode retains the triangle shader test.

The logs record the selected flags in `environment.txt`, dependencies in
`rust-dependencies.txt` and test results in `host-tests.log`. Artifact labels include
the selected mode. The IPA filename, Payload layout, checksum, unsigned packaging,
Xcode settings and corrected file-before-`-verify_arch arm64` order are preserved.

Windows checks using Rust 1.97.1 and the private offline Cargo cache:

| Check | Result |
| --- | --- |
| `cargo test --offline --manifest-path Cargo.toml --locked --no-default-features --test shader` | PASS: one existing triangle shader test |
| `cargo test --offline --manifest-path Cargo.toml --locked --no-default-features --features rerbr-3d --workspace` | PASS: 24 tests, including 21 imported math tests and two feature-gated integration tests |
| Host `cargo clippy --offline --manifest-path Cargo.toml --locked --lib` with each mode's feature flags and `-- -D warnings` | PASS in both configurations; iOS target compilation was not run |
| `cargo fmt --all -- --check` | PASS |
| Host dependency trees using each mode's flags | PASS: rerbr-render-core/glam appear only in feature mode |
| Script inspection | All four feature-sensitive Cargo commands use the selected array; original Xcode/build/IPA section from `plutil -lint` onward is unchanged; corrected lipo call retained |
| Workflow copies and line endings | PASS: workflow/template SHA-256 values match; script and both workflows have LF line endings |

Changed only `scripts/build-ios.sh`, the two experimental workflow copies, `README.md`
and this record. Cargo manifests/lockfile, app source, shader, vendored sources, Xcode
project and original reRBR engine were not changed. No geometry or on-screen behavior
was added. No dependencies were downloaded for these Windows checks.

Bash/macOS/Xcode are unavailable locally, so the script itself and the iOS feature
build were not executed here. Commit and push the updated files, start a new manual
Actions run and select `rerbr-3d`. Only a successful run with that selection can confirm
the Apple-target feature compilation and IPA packaging; device behavior remains a
separate check.

## First procedural 3D scene

Implemented a separate app-side `scene3d` pass and pure `scene_geometry` module behind
`rerbr-3d`. The scene is a one-unit cube resting at Y=0 on a flat ground quad. It uses
28 position/colour vertices, 42 u16 indices and no assets or textures. Vertex/index
bytes are serialized explicitly, without unsafe casts or new dependencies. A new WGSL
shader transforms positions and outputs face colours; this is not the full stage shader.

The original imported `FreeFly` and coordinate module provide the fixed eye/target,
horizontal FOV and left-handed Y-up view/projection. GPU state uses `coords::FRONT_FACE`
(clockwise, back-face culling), `Depth32Float`, clear 0 and `Greater` with depth writes.
Depth is allocated lazily for an active nonzero drawable and recreated when its size
changes. The original host's suspension, zero-size handling, surface recovery, touch
background toggle and submission/presentation code are retained.

The default triangle's pipeline, uniform animation and draw pass are wrapped in
`cfg(not(feature = "rerbr-3d"))`; the shader itself is untouched. Feature builds construct
only the 3D pass. They reuse the same Metal device and surface and do not change the
native/Xcode integration, IPA packaging or corrected lipo invocation.

Windows checks with Rust 1.97.1 and the existing private offline Cargo cache:

| Check | Result |
| --- | --- |
| First geometry-only test step | PASS: index/finite/area checks, outward normals, complete cube framing, reverse-depth ordering |
| `cargo check --offline --locked --lib --no-default-features --features rerbr-3d` | PASS |
| `cargo test --offline --locked --workspace --no-default-features --features rerbr-3d` | PASS: 30 tests (21 imported, 2 existing math integration, 5 geometry/camera tests, 2 WGSL validations) |
| `cargo clippy --offline --locked --workspace --all-targets --no-default-features --features rerbr-3d -- -D warnings` | PASS |
| `cargo test --offline --locked -p rerbr-probe --no-default-features` | PASS: unchanged triangle shader test; feature-specific tests disabled |
| `cargo clippy --offline --locked -p rerbr-probe --all-targets --no-default-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| Triangle and host source comparison | PASS: after ignoring formatting whitespace, triangle initialization, animation and draw calculations match the preceding source; lifecycle/touch handlers and submission/presentation tail also match |
| Import/fallback integrity | PASS: all eight vendor files match the prior snapshot byte-for-byte; triangle shader SHA-256 unchanged |
| Workflow copies | PASS: hashes match; only selector description changed to describe floor/cube mode |

The geometry tests check finite vertices, valid indices/nondegenerate triangles, outward
cube/upward floor normals, clockwise projected front faces, whole-cube visibility across
portrait/landscape aspects, and a shared camera ray where the cube must win the reverse
depth comparison against the floor. These are CPU/shader checks, not rendered-image tests.

Created `src/scene3d.rs`, `src/scene3d.wgsl`, `src/scene_geometry.rs` and
`tests/scene_geometry.rs`. Modified `src/lib.rs`, `tests/shader.rs`, the Cargo feature
comment (no dependency/lockfile changes), preflight source checks, workflow descriptions,
README and this record. No original engine or vendor files were edited. The build script,
Xcode project, native entry, signing/package checks and triangle shader were not changed.

The updated Python preflight was not run locally (no usable Python interpreter).
Neither Bash/Xcode/iOS compilation nor Metal pipeline creation and drawing were run
on this Windows host. There is no before/after render or iPad screenshot for this step.
A new Actions run with `rerbr-3d`, installation on the iPad, portrait/landscape captures,
depth/touch/lifecycle checks and a triangle-fallback comparison are still required.
Do not treat these local results as confirmation that the scene works on iPadOS.

## Harwood private-file diagnostic: stage 1

Added `vendor/rerbr-asset-formats` with nine original Rust source/test files, LICENSE
and rustfmt.toml copied byte-for-byte from the local reRBR snapshot. Its UPSTREAM.md
records original paths, GPL-3.0-or-later licensing and all eleven import hashes.
The new library exports only LBS, INI, texture-map, RBZ and DDS readers. Its one direct
dependency is zip 9.0.3, pinned with the deflate-flate2-zlib-rs backend. The offline
Cargo cache supplied all dependencies; no download or dependency installation was run.

The app's optional `rerbr-3d` feature includes the readers and CPU-only diagnostic.
The native entry passes Foundation's Documents path before Rust starts. After GPU
creation, a one-shot background worker reads `Documents/Harwood/Maps` if present,
otherwise logs SKIP. It uses bounded reads, case-insensitive lookups, original texture
request/remapping rules, ZIP read-to-EOF/CRC checks, DDS parsing and basic advertised
BC/texture-size checks. It never extracts or writes resources. Missing/corrupt assets
report errors without selecting a different scene. Optional device features are not
changed; no asset upload, Metal sampling or stage drawing is part of this batch.

First ran the command-line example directly on the owner's installed Maps folder,
then explicitly ran the ignored private-install integration test on the same folder.
Both completed successfully with the original Rust parsers, not header-only probes:

```text
blocks=282 (expected 282), visibility=282, object items=442
INI diffuse=39, specular=5; required DDS=38/38; declared DDS=44/44; redirected to N=41
required asset diagnostic PASS
```

Texture counts are references, not unique files. The required set has 37 diffuse
references and one specular reference; the audit also read all six remaining INI
entries. Some dry/new requests intentionally redirect to N's damp folder, so the
documented transfer keeps both complete RBZs. The diagnostic excludes global clouds
and water's unresolved runtime-manager keys; it does not prove full render coverage.

Windows checks with Rust 1.97.1 and the private offline Cargo cache:

| Check | Result |
| --- | --- |
| `cargo run --offline --example harwood-diagnostic --no-default-features --features rerbr-3d -- <private Maps>` | PASS, exit 0; initial run also resolved the updated lockfile |
| `HARWOOD_MAPS=<private Maps>` with `cargo test --offline --locked -p rerbr-probe --no-default-features --features rerbr-3d --test asset_loading real_harwood_installation -- --ignored --nocapture` | PASS: one explicitly executed test on the real installation |
| `cargo test --offline --locked --workspace --no-default-features --features rerbr-3d` | PASS: 91 tests; one real-data test intentionally ignored in the ordinary run |
| `cargo clippy --offline --locked --workspace --all-targets --no-default-features --features rerbr-3d -- -D warnings` | PASS |
| `cargo test --offline --locked -p rerbr-probe --no-default-features` | PASS: unchanged triangle shader test; feature tests disabled |
| `cargo clippy --offline --locked -p rerbr-probe --all-targets --no-default-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| Default normal dependency tree | PASS: only futures-executor, wgpu and winit are direct dependencies; no asset or camera library enabled |
| Asset dependency tree for aarch64-apple-ios | Inspected: ZIP/CRC/deflate/zlib-rs and Rust helpers; this is not a target compilation |
| Source integrity | All eleven new import hashes match; the existing camera vendor files and all scene/shader files match the preceding snapshot |
| Host integration integrity | After removing only the new module declaration and diagnostic call, src/lib.rs matches the preceding source exactly |

The six generated-fixture tests cover complete O-to-N resolution, case/slash
differences, GPU capability warnings, missing archives, bad DDS, CRC corruption,
path traversal, wrong block counts, truncated LBS and unused-vs-required errors.
No fixture was extracted from game files. Original upstream parser tests are retained.

Changed only the experiment's `.gitignore`, Cargo manifest/lockfile, `src/lib.rs`,
`xcode/ReRBRProbe/main.m`, `scripts/verify.py`, README and this record. New files are
`HARWOOD.md`, `src/asset_probe.rs`, `examples/harwood-diagnostic.rs`,
`tests/asset_loading.rs` and the complete new vendor directory. HARWOOD.md lists the
copy destinations and the five private files needed on the device. Source/package
preflight now also rejects private resource filenames and verifies the new imports.

The workflows, build-ios.sh, Xcode project/settings, Info.plist, signing/IPA layout,
corrected lipo invocation, original engine and RSF installation were not modified.
No proprietary asset was copied into the workspace, bundled or uploaded. No commit
or push was attempted; Git is unavailable and this checkout has no Git metadata.

Python preflight could not be executed locally: only WindowsApps aliases are present,
with no usable interpreter found. Its updated checks were reviewed, with hashes,
dependency selection and absence of source assets independently inspected on Windows.
The normal macOS Actions script will run it before the Apple build and against the IPA.
Full original-engine checks were not run because the original engine is outside this
isolated batch. No native Objective-C/Xcode/iOS build or physical-device check was run.

Pending: Actions with `rerbr-3d`, normal signing/install, private-file transfer to
Documents, relaunch, BC/log verification, and visual/touch/lifecycle comparison of
both existing modes on the iPad. The app's actual BC flag, sandbox path, thread/log
behavior and iOS compilation remain unverified on Apple hardware.

## First real Harwood geometry block: stage 2

Owner report, 2026-11-15: stage 1 passed on the physical iPad mini A17 Pro with
282 blocks/cells, 442 objects, 38/38 required and 44/44 declared DDS references,
adapter BC supported=true and device BC enabled=false. This is evidence for the
previous diagnostic only. No stage-2 iOS run has been performed here.

Added independent `harwood-block`, using the same optional libraries. Default
triangle and the separate `rerbr-3d` cube/audit retain their existing rendering.
If both optional features are enabled, Harwood takes priority. Only Harwood asks
for advertised TEXTURE_COMPRESSION_BC when creating the device. No dependencies,
lockfile, existing vendor sources or Xcode/native files were changed.

The read-only CPU worker reads the LBS through the imported original parser,
chooses a near/always block by distance from the start to its AABB, and converts
it using adapted reRBR block_mesh/slot_layout/repack/index/UV logic. The adapter's
GPL provenance, original source hashes and intentional reductions are recorded in
`src/harwood/UPSTREAM.md`. Only selected diffuse references are read through the
same INI/remap/archive/CRC/DDS code as stage 1. File reads remain bounded; LBS
storage is dropped before texture reads. GPU work remains on the rendering thread.

Measured on the owner's real Windows installation, without asset extraction/copy:

```text
READ: 28,014,518 bytes, 282 blocks, selected block 269 (zero-based)
start: (-406.9473, 74.991806, 1447.8748)
CONVERT: 2236 vertices, 1138 triangles, 9 draws
bad triangles / unsupported draws: 0 / 0
far LOD draws omitted: 5
specular draw flags: 2 (diffuse-only rendering in stage 2)
selected diffuse references: 0,1,2,3,4,6,7,8
projected winding at aspect 1.55: CW=1138, CCW=0
```

Selected materials include BR_07_ROADER, BR_07_GROUND_B, BR_01_FELLING and the
opaque track atlas. All eight references read and pass the CPU upload layout,
including all 32 stored mip levels. They are BC1/BC2, four mips each, up to
1024 x 2048. This is CPU evidence, not an executed GPU upload.

The fixed camera fits the converted box corners in portrait/landscape with original
FreeFly/coordinate maths. Matrix upload is column-major; positions and winding
are not flipped. GPU state is clockwise/back cull and reverse depth
Depth32Float/Greater/clear 0. The new shader uses baked colour, two UVs and the
original two-diffuse blend, with sRGB-surface compensation. Touches in this mode
cycle textures, vertex colours and UV checker while retaining the background
toggle. Specular lighting, original negative mip bias, sky, scenery lanes,
shadows, car, complete stage and automatic cameras are outside this batch.

The GPU path creates BC1/BC2 unorm textures and writes every stored mip with
block-correct row pitches and rounded copy extents. Error scopes separate
pipeline creation, upload validation and the first encoded/submitted/presented
frame. Log wording distinguishes accepted commands from GPU completion and
visible correctness. Missing BC/data or validation failure leaves a red
background and readable error instead of silently showing a different scene.

Windows verification, pinned Rust 1.97.1, existing offline cache:

| Check | Result |
| --- | --- |
| `cargo run --offline --locked --example harwood-block-diagnostic --no-default-features --features harwood-block -- <private Maps>` | PASS; real file selection/conversion/texture reads |
| Explicit `real_start_block_and_materials` ignored test with HARWOOD_MAPS | PASS; real materials, indices, framing, CW visibility, both BC formats and real CPU mip plans |
| Workspace tests with `--no-default-features --features harwood-block` | PASS: 83 tests; one private-data test ignored in ordinary runs |
| Workspace tests with `--all-features` | PASS: 97 tests; two private-data tests ignored in ordinary runs |
| Workspace tests with `--no-default-features --features rerbr-3d` | PASS: 91 tests; original mode remains independently compilable |
| Explicit stage-1 `real_harwood_installation` test using rerbr-3d alone | PASS: 282/282/442, 38/38, 44/44 and 41 references redirected to N |
| Default app tests with `--no-default-features` | PASS: unchanged triangle shader test; optional tests disabled |
| Clippy `--workspace --all-targets -- -D warnings` for harwood-block, rerbr-3d and all-features | PASS |
| Default app Clippy `--all-targets --no-default-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |

Synthetic tests cover exact slot-5/6/7/8 offsets, unchanged positions, BGRA-to-RGBA,
UVs, absolute indices despite a nonzero min_index, view bases, invalid indices,
far-LOD exclusion, AABB selection, camera framing/reverse depth and complete BC1/BC2
copy layouts down to 1x1. Naga parses/validates the new shader with no optional
shader capabilities. The MSVC host compiles the new wgpu API path; no Windows GPU
backend was added solely for testing. A test type-inference issue was corrected
before the successful checks; no failing check remains in the completed Rust suite.

Changed only local experiment files: Cargo.toml, src/lib.rs, src/asset_probe.rs,
tests/shader.rs, scripts/build-ios.sh, scripts/verify.py,
github-actions/ios-probe.yml, README.md, HARWOOD.md and this record. New files are
HARWOOD-BLOCK.md, examples/harwood-block-diagnostic.rs, tests/harwood_block.rs and
src/harwood/{mod.rs,mesh.rs,texture.rs,gpu.rs,shader.wgsl,UPSTREAM.md}.

The local workflow template/script add harwood-block selection only; previous
mode flags, Rust/Xcode library paths, lipo invocation, signing and packaging are
retained. The active `.github/workflows/ios-probe.yml` outside the experiment and
the GitHub Desktop repository were not modified or synchronized. No commit,
push, Actions run or dependency installation/download was performed. No game
resources were copied into the workspace or bundled. The original RSF install
was read only, and the original engine was not changed.

Pending: new aarch64-apple-ios compile/package verification, the Python/Bash
preflight on macOS (no usable local Python/Bash), Metal pipeline creation,
actual device BC feature enablement, all texture uploads, screenshots in each
view/orientation, depth/material appearance, touch/lifecycle and fallback-mode
checks. The stage-2 docs explicitly do not claim physical iPad success.

## Connected Harwood geometry: stage 3

Subsequent owner report: stage 2 passed on the iPad mini A17 Pro with block 269,
original geometry and BC1/BC2 textures. That supersedes the stage-2 pending status
above. This environment has not compiled or rendered the new stage-3 mode for iOS.

Added independent `harwood-neighbors`, a connected frontier selected from actual
shared triangle-boundary segments and guided by original LBS driveline/road
triangles. It reuses the unchanged mesh converter, camera maths, shader, BC upload
and reverse-depth state. Original positions are retained. Only index/range offsets
and texture aliases change when merging meshes. All imported vendor files remain
byte-identical. The old block loader retains its non-deduplicated return contract.

Read-only measurement of the owner's Windows Maps installation:

```text
IDs: 269,260,252,261,262,253,254,263,264,255,270,274,268,275,259,273,251,258,267,250
blocks=20; vertices=18983; triangles=9539; draws=124
material bindings=248; logical IDs=8; unique resolved DDS=8
VB+IB=721924 bytes; stored BC mips=1827840; combined GPU payload=2549764
source LBS=28014518 bytes; all converted mesh payload=7164920
road coverage=71/71; one sampled run from 0 to 588.1199m; unmatched=0
internal sampled gaps=0; all 19 tree connections have endpoint residual=0m
```

All 20 blocks fit the 400-draw/16-MiB-mesh/32-MiB-compressed-texture budgets; no
reduction occurred. Memory values exclude driver/state/alignment, surface/depth
images and CPU graph/parse overhead. Depth payload is separately logged on resize.
These estimates are not a measured peak or resident-memory benchmark. Sampled
coverage/edge matching cannot prove a watertight surface; see
HARWOOD-NEIGHBORS.md for tolerances and other limits.

Windows validation with the pinned Rust 1.97.1 MSVC toolchain and existing offline
Cargo cache (all Cargo checks below used `--offline --locked`):

| Check | Result |
| --- | --- |
| `cargo run --example harwood-neighbors-diagnostic --no-default-features --features harwood-neighbors -- <private Maps>` | PASS; actual selection, conversion, DDS resolution and CPU mip plans |
| Workspace tests, `--no-default-features --features harwood-neighbors` | PASS: 84 tests; two private-data tests ignored by default |
| Workspace tests, `--all-features` | PASS: 103 tests; four private-data tests ignored by default |
| Explicit workspace `--all-features -- --ignored --nocapture` with HARWOOD_MAPS | PASS: all four real-data tests executed (stage-1 audit, single block, connected region and DDS aliases) |
| Workspace tests, `harwood-block` alone | PASS: 83 tests; one private-data test ignored |
| Workspace tests, `rerbr-3d` alone | PASS: 91 tests; one private-data test ignored |
| Default triangle app tests with `--no-default-features` | PASS: one unchanged triangle shader test |
| Workspace/all-targets Clippy for neighbors, block, rerbr-3d and all-features; default app Clippy | PASS with `-D warnings` |
| `cargo fmt --all -- --check` | PASS |

Six new synthetic tests cover genuine edge connectivity versus box/corner/ID
coincidence, elevated disjoint surfaces, road-first selection, mesh/draw budget
limits, connected-prefix texture pruning, index/range/world-position preservation
and road-triangle height checks. The real-region test validates connectivity,
budgets, unique resolved DDS, material/index ranges and all indexed vertices in
four aspect-ratio frusta. The separate real-alias test loads IDs 0/12/22/23 and
retains two DDS. These private tests require explicit opt-in; no private fixtures
were added to normal CI.

Changed local experiment files only: Cargo.toml, src/lib.rs, src/asset_probe.rs,
src/harwood/{mod.rs,gpu.rs,UPSTREAM.md}, tests/shader.rs,
scripts/{build-ios.sh,verify.py}, github-actions/ios-probe.yml, README.md,
HARWOOD-BLOCK.md and this record. Added HARWOOD-NEIGHBORS.md,
examples/harwood-neighbors-diagnostic.rs, src/harwood/{neighbors.rs,neighbors_tests.rs}
and tests/harwood_neighbors.rs. Local `.validation/` contains ignored baselines,
build products and check logs, not application resources or files for synchronization.

The script/template change is limited to selecting the new mode, its feature and
workspace host tests. Cargo/Xcode paths, previous lipo fix, signing and IPA packaging
are retained. Cargo.lock, vendor sources, native/Xcode files, procedural scene,
triangle and terrain shaders/converter are unchanged by SHA-256 comparison with
the pre-stage-3 snapshot. No proprietary file was copied into this experiment.
No original engine/RSF file, active workflow or GitHub Desktop clone was modified.
No commit, push, Actions run or dependency installation was performed.

Python/Bash preflight could not be run locally: no usable interpreter was found
in the available commands. Source/hash/asset-presence checks were inspected on
Windows; macOS must run the existing preflight during the eventual build.
Pending: first stage-3 aarch64-apple-ios build, Metal uploads and panoramic image,
three-view/orientation/lifecycle regressions, seamless-looking road/terrain, frame
time and real peak/resident memory on the iPad. No screenshot, Metal execution or
physical success is claimed for stage 3.

## Automatic Harwood graphics benchmark: stage 4

Owner report, 2026-11-15: stage 3 has now compiled through Actions and rendered the
20-block set on the physical iPad mini A17 Pro. This supersedes the pending
stage-3 device status above. The new stage-4 benchmark has no Apple/device result.

Added `harwood-benchmark = ["harwood-neighbors"]` as a separate app mode. It loads
the unchanged 20-block workload and retains the actual selected LBS driveline and
road triangles in memory for a new app-owned camera. No imported/vendor source,
mesh converter, terrain shader, texture upload algorithm, Xcode/native file or
lockfile was changed. The runtime reads only the existing private Documents path;
the explicit command-line diagnostic reads the owner's Maps folder without writes.

Real-file CPU result, without saving assets or raw route/mesh data:

```text
original route points=71; interpolated camera path=588.53m; speed=29.43m/s
checked camera-ground clearance=1.400..1.432m
blocks=20; vertices=18983; triangles=9539
terrain draws/frame=124; additional HUD draws/frame=1; unique DDS=8
```

Catmull-Rom interpolation, an approximately 0.25m arc-length table and real elapsed
time determine position. A distance-based look-ahead determines smooth heading.
Ground comes from actual triangle-plane vertical intersections on the selected
road/terrain, not bounding boxes or the stage-3 3m road-coverage tolerance. A short
height smoother is clamped above the highest intersected terrain by 1.4m on every
frame. Dense 0.1m distance checks and live support checks reject missing/ambiguous
ground; unsupported road gaps never get an invented bridge. This validates the
eye point, not a complete near-plane or vehicle collision volume.

The app holds a two-second warmup before starting a monotonic-clock 20s run. It
retains actual stop overshoot at the first completed frame after the deadline;
there is no fixed-frame stopping rule. A touch resets warmup/metrics using cached
resources. Suspension, occlusion, surface loss or resolution changes invalidate a
warming/running measurement. The existing post-draw scheduling delay is removed
only in benchmark mode, retaining FIFO/Metal pacing and the previous modes' behavior.

An original generated ASCII HUD displays live FPS, time, distance, state, CPU
preparation time and a final summary. It adds no dependency or extracted font.
The summary reports host completion cadence mean FPS, min/max over twenty fixed
one-second windows, mean/P95/P99 intervals, total frames, real duration and CPU
preparation mean/P95. CPU preparation excludes drawable acquisition and submit/
present waits. No GPU timestamp feature is requested: GPU time is explicitly
unavailable, FPS is not utilization, and requests are not confirmed scanout. HUD
and ground-query work are timed; loading/upload and final summary work are excluded.

Windows checks used the pinned Rust 1.97.1 MSVC toolchain, existing cache,
`--offline --locked`, CARGO_TARGET_DIR/TEMP inside ignored `.validation/`:

| Check | Result |
| --- | --- |
| `cargo run --example harwood-benchmark-diagnostic --no-default-features --features harwood-benchmark -- <private Maps>` | PASS; actual supported route and workload, no GPU |
| Workspace tests with `--no-default-features --features harwood-benchmark` | PASS: 95 tests; three private tests ignored in normal run |
| Workspace tests with `--all-features` | PASS: 114 tests; five private tests ignored in normal run |
| Explicit workspace `--all-features -- --ignored --nocapture`, HARWOOD_MAPS set | PASS: five real-data tests, including the new moving route/ground test |
| Independent `harwood-neighbors` workspace tests | PASS: 84 tests; two private tests ignored |
| Independent `harwood-block` workspace tests | PASS: 83 tests; one private test ignored |
| Independent `rerbr-3d` workspace tests | PASS: 91 tests; one private test ignored |
| Default triangle app tests | PASS: one shader test |
| Workspace/all-targets Clippy for benchmark, all-features, neighbors, block, rerbr-3d; default app Clippy | PASS with `-D warnings` |
| `cargo fmt --all -- --check` | PASS |

Ten new synthetic unit tests exercise nonuniform route spacing, curved positions/
heading, triangle heights/top surfaces, unsupported gaps, real-time progress at
30/60 and 4/8 simulated FPS, fixed-window/percentile maths, stalls/stop overshoot,
warmup/restart/invalid clocks, interrupted runtime and bounded original HUD data.
Naga validates the added HUD WGSL with no optional shader capabilities. The new
private test checks the unchanged real workload, all support checks and 2,000
additional camera/heading samples. These are CPU/synthetic-clock tests; no Windows
GPU backend or physical display result is implied. A format check caught spacing
in two new assertions; formatting corrected it before the final passing checks.

Modified only local experiment files: Cargo.toml, README.md, HARWOOD-NEIGHBORS.md,
this record, github-actions/ios-probe.yml, scripts/{build-ios.sh,verify.py},
src/lib.rs, src/harwood/{mod.rs,gpu.rs,neighbors.rs,UPSTREAM.md} and tests/shader.rs.
Added HARWOOD-BENCHMARK.md, examples/harwood-benchmark-diagnostic.rs,
tests/harwood_benchmark.rs and src/harwood/benchmark/{mod.rs,route.rs,metrics.rs,
hud.rs,hud.wgsl,tests.rs}. Ignored `.validation/` holds baselines/check logs and
build products, not source resources or files for synchronization.

SHA-256 comparison confirms all 36 protected files unchanged: both vendors,
native/Xcode, Cargo.lock, existing asset reader, procedural scene, triangle shader,
terrain converter/texture layout/shader. Source inventory contains no private
assets. Original engine/RSF installation, GitHub clone and active workflows were
not modified. No commit, push, Actions run, download or dependency installation
was performed. This source archive has no Git metadata; integrity uses the saved
pre-stage-4 file hashes and explicit source inventory.

Pending: Apple compile/package, Metal/HUD execution, physical route/elevation and
near-plane inspection, live timing/summary/restart, lifecycle/rotation regression,
all previous device modes, real memory and thermal/frame-pacing measurement.
Python/Bash preflight could not be executed locally: available commands include
only WindowsApps Python aliases, with no usable Python/Bash interpreter found.
The updated preflight source was reviewed; macOS must execute it during the next
authorized Actions build. No stage-4 iPad FPS, GPU timing or visual success is claimed.

## Original Corolla chase benchmark: stage 5

Owner report, 2026-11-15: stage 4 now compiles and runs on the iPad mini A17 Pro,
covering approximately 588.53m in 20 seconds at 60 FPS with V-sync. This supersedes
the pending stage-4 device status above. Stage 5 has no Apple build or device result.

Added independent `car-chase-benchmark = ["harwood-benchmark"]`, with triangle still
the default. Four byte-identical original parsers (cars.rs, car_parts.rs, sgc.rs,
sg_framing.rs) retain their license and provenance in the asset-format vendor.
App-owned exterior selection, conversion and material states are adapted from
reRBR car/{parts,meshes,shading}.rs and lights.rs, with source hashes and differences
in src/car/UPSTREAM.md. No dependency or Cargo.lock change was needed.

Actual Windows read-only diagnostic against the private original installation:

```text
[Car06] COROLLA; SGC bytes=2029757; decoded records=636
converted exterior meshes=75; skipped meshes=47; shader/material pairs=16
car vertices=11225; triangles=11166; draws=99; unique DDS=3
opaque vertices=10851; triangles=10756; draws=83; unique DDS used=2
blended vertices=374; triangles=410; draws=16; unique DDS used=1
external DDS=BC2 1024x1024, 11 mips
dynamic DDS=BC1 512x256, 10 mips
transparent DDS=BC2 512x512, 10 mips
required private car files=8; all converted exterior diffuse paths resolved
car VB+IB payload=493192 bytes; BC mip payload=1835080 bytes
unchanged terrain vertices=18983; triangles=9539; draws=124; unique DDS=8
pose samples=1179; minimum wheel-vertex clearance=0.004989624m
maximum geometric wheel travel=0.06560516m; minimum accepted chase boom=6m
```

All eight private paths and transfer instructions are in CAR-CHASE-BENCHMARK.md.
The app accepts only Documents/RBR for the car and Documents/Harwood/Maps for the
terrain. Missing material/file/additional dependency fails explicitly. No source
fixture contains extracted model data, texture bytes, route coordinates or original
INI contents. The explicit CPU example accepts the owner's installation path;
there is no installation fallback inside the iOS app.

The original -Z car orientation, Y up, hierarchy, UVs, face ranges, absolute indices,
wheel pivots and inverse-transpose normals are retained. Four gravel wheels, body
and visible exterior parts are selected; interior, ghost, shadow planes, overlays,
unused surface variants and hidden initial lights are excluded. Draw ordering,
culling, blend and depth-write rules retain the original material states. The car
loads the terrain's reverse-Z depth; HUD draws last. The first WGSL is unlit diffuse
and opacity, without DirectX shaders. Full original lighting/reflections/Fresnel,
dirt/damage, shadows and dynamic lamps remain out of scope.

Vehicle support uses a separate geometric pose, not the old driver eye height.
Four contacts set pitch/roll; low body and wheel vertices query real terrain planes.
Wheel/brake presentation travel is bounded at 0.25m. This is not suspension physics,
a full swept-volume collision solver, steering or wheel rotation. A 2m cached-plane
candidate grid and exact terrain-segment tests avoid brute-force camera rays; the
previous benchmark keeps its 8m grid and original height arithmetic.

The path/clock/20-second session and cadence statistics are retained. Chase uses
6m distance, nominal 2.5m road-relative height, 0.5m target and original 65-degree
horizontal FOV. Support, line of sight and near-plane corners are checked at load
and on each frame. An initial real-data frustum test caught a small clip using the
old 50-degree driver FOV; the dedicated 65-degree chase projection fixed it without
changing other modes. All drawn car vertices pass three aspect ratios (0.65, 1.55,
2.0) at 41 route positions. Terrain-only A and terrain+car B use the same resources,
pose queries and camera; only car submissions are suppressed in A. HUD/log counters
separate terrain, opaque car, blended car and actual generated HUD geometry.

Windows checks used Rust 1.97.1 MSVC, existing cache, `--offline --locked`, and
build/log/temp outputs only in ignored .validation/:

| Check | Result |
| --- | --- |
| car-chase-diagnostic against private RBR | PASS: eight files, actual geometry/material/DDS conversion and full support path; no GPU |
| Workspace tests, car-chase-benchmark only | PASS: 130 tests; four private tests ignored |
| Workspace tests, all-features | PASS: 149 tests; six private tests ignored |
| Explicit all-features ignored tests with RBR_CAR_DATA/HARWOOD_MAPS | PASS: all six private tests; new car/route/frustum test completed in 30.04s in Windows debug |
| Independent harwood-benchmark / harwood-neighbors / harwood-block / rerbr-3d workspace tests | PASS: 115 / 104 / 103 / 111 tests respectively |
| Default triangle app tests | PASS: one shader test |
| Workspace/all-targets Clippy, each optional mode and all-features; default app Clippy | PASS: all with -D warnings |
| cargo fmt --all -- --check | PASS |
| SHA-256 provenance check | PASS: all 20 vendor provenance entries; four new parser copies equal original source bytes |

Fourteen new synthetic car tests cover parser-to-converter wiring, hierarchy,
cycles, selection/hidden switches, indices/UVs/normals/materials, wheel pivots,
pose transforms, ground clearance, exact visibility, unsupported cameras, A/B,
restart and cleared metrics. Existing real-time-duration/statistics tests remain
active. Naga validates the new car WGSL. These checks type-check the wgpu code but
do not create a Windows GPU device or establish Metal behavior.

Modified: Cargo.toml, README.md, HARWOOD-BENCHMARK.md, this record,
github-actions/ios-probe.yml (secondary template only), scripts/{build-ios.sh,
verify.py}, src/lib.rs, src/harwood/{mod.rs,gpu.rs},
src/harwood/benchmark/{mod.rs,hud.rs,route.rs}, tests/shader.rs and
vendor/rerbr-asset-formats/{src/lib.rs,UPSTREAM.md}.
Added: CAR-CHASE-BENCHMARK.md, examples/car-chase-diagnostic.rs,
tests/car_chase.rs, src/car/{mod.rs,selection.rs,materials.rs,mesh.rs,placement.rs,
gpu.rs,runtime.rs,shader.wgsl,tests.rs,UPSTREAM.md}, and
vendor/rerbr-asset-formats/src/{cars.rs,car_parts.rs,sgc.rs,sg_framing.rs}.

SHA-256 against the saved 73-file baseline leaves 57 existing files unchanged,
including Cargo.lock, render-core vendor, previously imported format readers,
native/Xcode, asset diagnostic, procedural scene, triangle/terrain shaders,
terrain mesh/texture converters and timing metrics. The 90-file source inventory
contains no proprietary resource, binary or temporary artifact; .validation/
contains ignored local build products and diagnostic logs only. Source/IPA
preflight now also rejects INI and DirectX shader resources. The script still
passes --no-default-features --features car-chase-benchmark to Cargo for this mode;
Xcode/lipo paths, signature settings and IPA packaging are retained.

Pending: macOS/iOS build and package preflight, actual Metal BC uploads, car image,
alpha/culling/depth inspection, support and camera motion on the physical iPad,
portrait/landscape, interruption/restart, repeated A/B timings, CPU cost, memory
and thermals. GPU timestamps are not implemented and are reported unavailable.
Python/Bash preflight could not run here because no usable local interpreter was
found; reviewed source must run on macOS. No new iPad FPS or visual success is
claimed. Original engine, RBR installation, GitHub clone and active workflow were
not modified. No commit, push, synchronization or Actions run was performed.

## Pure physics probe: Windows validation, 2026-11-15

`physics-probe` is a separate optional CPU mode. It imports the authentic
rbr-physics/rbr-hybrid dependency closure and the standalone boot; no physical
equation, tyre algorithm or integration order was replaced. Eight local crates
are necessary for the original boot/types. The source archive has no Git metadata,
so vendor/rerbr-sim/UPSTREAM-HASHES.json identifies 623 original files by SHA-256.
Of these, 620 are unchanged. PATCHES.json identifies the three host adaptations:
page storage, its test registration, and explicit private NGP data roots. The
original flat memory backend is retained byte-for-byte as a test-only reference.
The hybrid manifest builds rlib rather than the desktop harness cdylib. Original
equations and the standalone boot/handle sources remain byte-identical.

The worker requests a 32MiB native stack and replaces the 4GiB+8MiB contiguous
mapping with stable demand-allocated pages, preserving logical u32 addresses,
registers, byte/word/copy/snapshot/epoch semantics. A parity test exercises 1,000
deterministic writes, unaligned/page-boundary/wrapping addresses, unmapped reads,
restore and retained page bytes against the original backend. Metadata uses
16MiB on 64-bit hosts; allocated page payload has a caught 256MiB budget. This
does not guarantee recovery from actual allocator OOM, stack overflow or OS kill.

Real read-only private-data execution uses Corolla [Car06], Harwood's actual dry
O season row (33 on these files), normal wear, original NGP boot and Stage
transitions. Each independent pass runs 1,520 original loading/contact steps and
720 measured steps, with timestep bits 0x3ab60b61 (approximately 1/720s). Neutral,
brake/clutch and zero throttle deliberately test a stationary car. Two passes
execute 4,480 physical substeps altogether. No original DLL instruction is executed;
the pinned DLL supplies immutable constants/initializers to the authentic readers.

An isolated Release integration test used only 16 SHA-256-verified private file
copies (36,321,934 bytes), outside both repositories, in the exact Documents layout
listed in PHYSICS-PROBE.md. No installation/overlay fallback was available. Both
boots and all steps passed. Of the 16 files, Cars.ini, corolla.sgc and the O LBS
were already needed by stage 5; 13 files are additional. The compatible package's
physics.rbz is used along with its NGP.dll; RSF's installed 7.5 remains untouched.

| Isolated Windows Release run | Pass 1 | Pass 2 |
| --- | --- | --- |
| Boot, seconds | 0.141 | 0.136 |
| Measured steps | 720 | 720 |
| Mean host wall duration per step, ms | 0.349 | 0.325 |
| P95 / P99, ms | 0.703 / 0.900 | 0.646 / 0.952 |
| Maximum step, ms | 1.649 | 1.712 |
| Simulated / actual measured-loop duration | 3.98 | 4.27 |

These PC results are not A17 predictions, CPU utilization or GPU timestamps. They
exclude boot/settling; loop wall time includes progress/measurement overhead.
Both endpoint FRM1 car/wheel hashes were
`c7e1409b515dd69276132cde2c4992be166d35da262ababf49288ed5b73c454b`.
The Debug run and separate full-install Release runs agreed with this hash.
This tests endpoint repeatability, not every state word, an original recording
oracle, a moving load or cross-architecture bit equivalence. The actual boot
reported 50 registry entries, 7,625 placements, 604 interactive handles, 76 stage
meshes, nine car collision meshes, 13 car placements and 375 fence posts.

Allocated page payload was 38,313,984 bytes (36.54MiB), plus 16MiB metadata. This
is not resident memory. The new native TASK_VM_INFO.phys_footprint query reports
whole-process memory on iOS; Windows reports unavailable. The sampled maximum
cannot capture every transient peak. Public Mach/native compilation is untested.

Windows checks used Rust 1.97.1 MSVC and the existing offline cache, with build,
temp and log products kept in ignored .validation/:

| Check | Result |
| --- | --- |
| Workspace tests, physics-probe only | PASS: 996; one private test ignored |
| Script's selected physics/host/app packages | PASS: 194; one private test ignored |
| Independent physics app tests | PASS: five; one private test ignored |
| Actual private Release integration | PASS: full-install and isolated 16-file Documents variants |
| Default triangle app tests | PASS: one |
| Independent car-chase app regression | PASS: 34; four private tests ignored |
| All-features app tests | PASS: 56; seven private tests ignored |
| Workspace/all-targets Clippy, physics-probe and all-features | PASS with -D warnings |
| cargo fmt --all -- --check | PASS |
| Original flat/sparse memory parity | PASS |
| SHA-256 provenance | PASS: 623 imports, three explicit adaptations, original flat reference, 20 existing vendor entries and retained GPL licenses |

Tests requiring absent retail corpora may return without exercising those files;
the 996 count does not establish a full retail-file or recording-oracle sweep.
The original repository-wide rust-check.sh/hybrid recording checks were not run
against the untouched parent engine. No original recording set is present here.

The animated triangle remains the liveness background. A GPU HUD shows worker
phase, physical steps, allocated memory, timing/results or errors. Completion/error
tap starts fresh state; background/occlusion requests cancellation. Boot cannot
be interrupted mid-constructor. The physics device case alone enables Release
panic=unwind so the original pure-host stop can be caught and reported. Visual
build cases retain their prior panic policy and host-test scope.

Modified existing files: .gitignore, Cargo.toml, Cargo.lock, README.md, this file,
github-actions/ios-probe.yml (secondary template only), scripts/build-ios.sh,
scripts/verify.py, src/lib.rs, tests/shader.rs and xcode/ReRBRProbe/main.m.
New: PHYSICS-PROBE.md, examples/physics-diagnostic.rs, src/physics_probe.rs,
tests/physics_probe.rs and the complete vendor/rerbr-sim/ source tree. Existing
render/car/terrain conversions, shaders, camera, benchmark metrics, Xcode project
and previously imported vendors are unchanged. The source verifier checks the
new closure/licenses, disallows private files in source and IPA, and permits only
two hash-verified upstream CPU approximation tables (8KiB each) as source BIN
exceptions. These are not game assets or raw Xcode resources.

The secondary workflow template adds only the physics mode and its description.
The script retains Cargo/static-library/Xcode/lipo/signing/IPA paths and adds
--no-default-features --features physics-probe with the unwind override. The
active clone workflow and GitHub clone were not touched. No resources were added
to the project or IPA. The installed NGP 7.5 DLL and supplied compatible 7.4 DLL
retain their verified hashes. No commit, push, synchronization or Actions run.

Pending: macOS/iOS compilation/linking/unwinding, native memory reporting, ARM64
arithmetic and repeatability, stack/resident memory, boot/live HUD/cancellation/
restart on the iPad and sustained moving-car performance. Python/Bash preflight
could not run because no usable local interpreters were found; source/hash/license/
resource checks were additionally executed in PowerShell. No Apple build or device
success is claimed. Private files must be transferred after unsigned IPA signing
and installation; they are never packaged or uploaded.
