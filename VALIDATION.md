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
