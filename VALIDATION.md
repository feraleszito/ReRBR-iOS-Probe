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
