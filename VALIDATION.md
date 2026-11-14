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
