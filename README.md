# ReRBRProbe

An independent iPadOS graphics experiment: Rust, winit **0.30.13**, wgpu **30.0.1**,
and the **Metal** backend. It draws a dark background and a rotating, coloured triangle.
Touch the screen to switch the background. The optional `rerbr-3d` feature now compiles
original reRBR coordinate and camera mathematics from a small local library. It does not
change the triangle or introduce a 3D drawing mode in this batch. There are no original
game-system crates, physics, SDL, audio, Richard Burns Rally files, RallySimFans files,
textures or downloads of game content.

The Cargo manifest declares its own workspace, lockfile and toolchain (**Rust 1.97.1**).
Building this directory does not build or alter the parent workspace. Runtime dependencies
are pinned exactly; `Cargo.lock` also fixes transitive dependencies. Naga 30.0.1 is a
development dependency for validating the embedded WGSL shader.

## Files

| Path | Purpose |
| --- | --- |
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `rustfmt.toml` | Independent Rust build and fixed versions |
| `src/lib.rs` | C entry point, winit lifecycle, Metal device, drawing and touch handling |
| `src/triangle.wgsl` | Embedded animated triangle shader; no optional GPU features |
| `tests/shader.rs` | Naga parsing and validation without optional shader capabilities |
| `tests/core_math.rs` | Optional-dependency projection/orbit integration tests, enabled by `rerbr-3d` |
| `vendor/rerbr-render-core/` | Original coordinate/camera sources and tests, license and provenance hashes |
| `xcode/ReRBRProbe/main.m` | Native entry, diagnostic file, call into the Rust static library |
| `xcode/ReRBRProbe/Info.plist`, `LaunchScreen.storyboard` | iPad bundle configuration and launch screen |
| `xcode/ReRBRProbe.xcodeproj/` | Minimal Xcode project and shared Release scheme |
| `scripts/build-ios.sh` | Build the static library and app, package and inspect the unsigned IPA |
| `scripts/verify.py` | Standard-library source/package checks (Python 3.11+) |
| `github-actions/ios-probe.yml` | Manual GitHub Actions workflow template |

## First reRBR integration batch: camera and coordinates only

The app's default feature set is empty. Its default workspace member is the app itself,
so the existing build still selects the triangle. `rerbr-3d` enables the optional path
dependency `vendor/rerbr-render-core`; **both feature configurations still show the
same triangle**. The app's `src/lib.rs`, shader, touch handling and native/Xcode files
were not changed for this batch.

The core library contains verbatim copies of `coords.rs`, `camera.rs` and `triple.rs`
from the original `rbr-render` source. `triple.rs` supplies types used by the camera;
it is only mathematics and does not create windows. The files retain their 21 upstream
unit tests. The two additional host integration tests check reverse-depth ordering and
orbit projection across portrait/landscape aspect ratios without opening a GPU device.
The library depends only on pinned `glam 0.33.12` and `wgpu 30.0.1` (the latter supplies
the front-face enum; the core creates no GPU resources). No renderer pass, mesh, ground,
cube, new shader, asset loader, physics or sound was added.

See `vendor/rerbr-render-core/UPSTREAM.md` for original paths and SHA-256 hashes, and
`vendor/rerbr-render-core/LICENSE` for the retained license. The preflight verifier allows
only this additional local optional dependency and checks the imported source hashes.

From the standalone repository root, run:

```sh
# Existing app and triangle test, without enabling the imported library.
cargo test --locked -p rerbr-probe --no-default-features
# Imported unit tests, two integration tests, and the unchanged triangle shader test.
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

The workflow now offers a manual `build_mode` choice. The build script reads the same
selection from `PROBE_MODE` (default `triangle`) and rejects any other value:

| Selection | Cargo feature flags | macOS host tests | Image shown |
| --- | --- | --- | --- |
| `triangle` (default) | `--no-default-features` | Existing triangle shader test | Triangle |
| `rerbr-3d` | `--no-default-features --features rerbr-3d` | Full workspace: upstream math, integration and shader tests | Same triangle |

The selected feature flags are passed to the host tests, iOS Clippy, the **release
static-library build for `aarch64-apple-ios`**, and a dependency-tree diagnostic. The
existing Xcode library search path, ARM64 checks, unsigned IPA packaging and signing
restrictions are unchanged. The feature still adds no geometry or drawing pass.

On a macOS host with Xcode and the iOS target, an explicit feature-only type check is:

```sh
cargo check --locked --lib --target aarch64-apple-ios --features rerbr-3d
```

The new Apple-target feature build has not been performed here. Windows mathematics
and type checks do not establish iPadOS compatibility; run Actions with `rerbr-3d` to
verify the selected configuration.

## Build on GitHub Actions from Windows

The workflow runs on the standard `macos-15` runner and explicitly selects
`/Applications/Xcode_16.4.app/Contents/Developer`. Xcode 16.4 includes the iOS 18.5
SDK; the deployment target is **iPadOS 18.0**, and the device architecture is **arm64**.
The runner must continue to offer that Xcode installation. An early step checks this;
if GitHub removes it, deliberately update both workflow copies and the cache key to a
supported Xcode version. No Apple account, signing certificate, paid membership or
GitHub signing secrets are used to build the unsigned IPA.

1. Put the **contents** of this experiment directly at the root of your standalone
   `ReRBR-iOS-Probe` repository. `Cargo.toml`, `Cargo.lock`, `scripts/`, `src/`, `tests/`,
   `vendor/` and `xcode/` must be at the root. Do not create another `ios-probe/` directory there.
2. GitHub discovers workflows **only** under the repository root's `.github/workflows/`.
   The activated file must be `.github/workflows/ios-probe.yml`, copied from
   `github-actions/ios-probe.yml`. The template alone is not discoverable.
   Keep the two copies identical. This workflow targets the standalone repository;
   its layout check deliberately fails at the root of the original reRBR repository.
3. Commit the new files, including the root `Cargo.lock`, and push to your own repository.
   Ensure the workflow is present on the **default branch** so the manual Run workflow
   button appears. Enable Actions if this is a fork with workflows disabled.
4. Open **Actions > ReRBRProbe unsigned iPadOS > Run workflow**. Choose `build_mode`:
   **triangle** for the default configuration, or **rerbr-3d** to compile the optional
   coordinate/camera library for the iPad target. Start a new run using the updated
   commit; re-running an old job uses its old workflow/script revision.
5. Download `ReRBRProbe-unsigned-<build_mode>-<run number>`. Extract the artifact ZIP on Windows to obtain
   `ReRBRProbe-unsigned.ipa` and `SHA256SUMS`. The artifact ZIP itself is not the IPA.
6. Check the IPA hash with `Get-FileHash .\ReRBRProbe-unsigned.ipa -Algorithm SHA256`.

The expected standalone layout is:

```text
ReRBR-iOS-Probe/
  .github/workflows/ios-probe.yml
  github-actions/ios-probe.yml
  Cargo.toml
  Cargo.lock
  rust-toolchain.toml
  rustfmt.toml
  scripts/build-ios.sh
  scripts/verify.py
  src/
  tests/
  vendor/rerbr-render-core/
  xcode/
```

All workflow paths are relative to that repository root: the cache uses `. -> target`,
the build runs `bash scripts/build-ios.sh`, and artifacts come from `build/`.
The build script finds its own parent directory and passes that directory's manifest
explicitly to Cargo. Xcode's `../target/aarch64-apple-ios/release/librerbr_probe.a`
is relative to `xcode/`, so it already resolves to the root `target/` directory.
`scripts/verify.py` also finds the root from its own location; neither requires an
`ios-probe` directory name.

The archive reference alone is not sufficient for linking: Xcode links it by name as
`-lrerbr_probe`, so the project explicitly adds `$(RUST_LIBRARY_DIR)` to
`LIBRARY_SEARCH_PATHS`. That setting defaults to
`$(SRCROOT)/../target/aarch64-apple-ios/release`. The build script passes the matching
absolute `RUST_LIBRARY_DIR` to Xcode after checking that Cargo actually created a
nonempty `librerbr_probe.a` containing ARM64 code. It records that archive's location
and architecture in `build/rust-library.txt`, and Xcode's effective build settings in
`build/xcode-settings.txt`. Both files are included in the existing diagnostics artifact.
If `ld: library 'rerbr_probe' not found` recurs, compare `Static library:` in the first
file with `RUST_LIBRARY_DIR` and `LIBRARY_SEARCH_PATHS` in the second, then inspect the
link command in `build/xcode-build.log` for the corresponding `-L` argument.

The job installs the pinned Rust toolchain and `aarch64-apple-ios`, caches only this
project's target directory, runs source preflight, formatting, a shader validation test
on the macOS host, iOS clippy, the iOS release build and Xcode. It checks that the IPA
contains an **unsigned, physical-iOS ARM64 executable**, an expanded plist and a compiled
launch storyboard. A simulator executable is rejected even when its CPU is also ARM64.
It uses `xcodebuild build` with signing disabled and packages `Payload/ReRBRProbe.app`
directly; it does not use signing-dependent archive export.

Logs are uploaded separately even if a build step fails. Artifacts expire after seven
days. A green job proves compilation and package checks, **not** execution on an iPad.
There is no simulator or physical-device execution in this workflow.

For the feature build, `build/environment.txt` should report `Probe build mode: rerbr-3d`
and `Cargo feature flags: --no-default-features --features rerbr-3d`.
`build/rust-dependencies.txt` should include `rerbr-render-core` and `glam`; the default
configuration's normal dependency tree should not. Host-test results are in
`build/host-tests.log`, and iOS Rust compilation output is in `build/rust-build.log`.
These files appear in `ReRBRProbe-logs-<build_mode>-<run number>`.

Standard GitHub-hosted runners are free for public repositories; private repositories
consume the account's included usage and may incur charges. Larger runners are not
needed. Artifact storage has its own allowance. Use manual runs and short retention.
See [GitHub Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions).

## Sign and install from Windows

An unsigned IPA **cannot run on the iPad by downloading or opening it in Files**.
It must first be signed for a permitted device and installed. This prototype requires
no JIT, jailbreak, restricted entitlements, extended virtual addressing or additional
hardware capabilities beyond Metal.

For the first attempt, use [Sideloadly](https://sideloadly.io/) on Windows:

1. Use its current official Windows instructions and required Apple device drivers.
2. Connect the iPad by USB, unlock it and trust the computer.
3. Select the extracted `ReRBRProbe-unsigned.ipa`, the iPad and your Apple account.
4. Let Sideloadly sign and install the app. Enter credentials locally, not into Actions.
5. Trust the developer profile in Settings if requested. Enable **Developer Mode** under
   Privacy & Security if required, restart the iPad and confirm the prompt.
6. Open **ReRBRProbe** from the home screen.

[AltStore Classic with AltServer for Windows](https://faq.altstore.io/altstore-classic/how-to-install-altstore-windows)
is an alternative. AltStore Classic normally occupies one of the free account's app slots.
Use **Classic**, not an assumption that a marketplace app can install any unsigned IPA.

Apple's free Personal Team provisioning has **seven-day profiles**, **three installed
apps per device**, and limits of **ten App IDs** and **three devices**. Renewal/re-signing
is required before the app expires; keep the identifier stable to avoid wasting IDs.
Availability and authentication of third-party installation tools must be confirmed with
your actual iPadOS version. Successful IPA creation does not guarantee successful signing.
See [Apple's account limits](https://developer.apple.com/help/account/basics/about-your-developer-account)
and [Developer Mode](https://developer.apple.com/documentation/xcode/enabling-developer-mode-on-a-device).

TestFlight and App Store distribution are separate, paid-program workflows; they are not
configured here. A macOS ad-hoc signature is not an iPad provisioning profile.

## Verify on the iPad

1. A dark blue background and a smoothly rotating, colour-pulsing triangle appear.
   The launch screen alone is a static dark background: it does **not** prove rendering.
2. Touch the screen: the background changes between blue and red tones.
3. Rotate between portrait and landscape: the triangle keeps its shape and stays centred.
4. Switch apps, lock/unlock, and return several times: drawing resumes without a black
   screen or crash. GPU work stops while the app is suspended. This is a manual requirement,
   not a claim that lifecycle handling has already passed on hardware.
5. After returning to Files, inspect **On My iPad > ReRBRProbe > rerbr-probe.log**.
   Look for the native main-thread diagnostic, `GPU: ... / Metal`, surface dimensions,
   `presented frame 1`, touches, and suspend/resume messages. The BC-support line is
   diagnostic only and does not affect drawing. Export the log when reporting a failure.

The animation aims for roughly 60 redraws per second; no hardware frame-rate measurement
has been made. Device loss beyond surface recreation is not recovered automatically.
The log appends across launches and is intended only for this small experiment. This
project has no production icons, menus, automated visual captures or distribution setup.

## Optional local checks

From the standalone repository root (or the local `ios-probe` directory), on a machine
with Rust 1.97.1 and the required build tools:

```sh
cargo fmt --all -- --check
cargo check --locked --lib
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --test shader
python3 scripts/verify.py
```

Checking the Rust code on Windows does not exercise the Metal or UIKit backend. Only
macOS with full Xcode and the iPhoneOS SDK can perform the intended Apple build here:

```sh
rustup target add aarch64-apple-ios
DEVELOPER_DIR=/Applications/Xcode_16.4.app/Contents/Developer bash scripts/build-ios.sh
# Optional camera/math feature; the same unsigned IPA packaging and triangle image.
PROBE_MODE=rerbr-3d DEVELOPER_DIR=/Applications/Xcode_16.4.app/Contents/Developer bash scripts/build-ios.sh
```

The script writes only this directory's `target/` and `build/`. It refuses to overwrite a
previous Payload/IPA; start from a fresh checkout or move the probe's `build/` aside to rerun.
Do not run the parent reRBR build/check script for this isolated experiment.

## Validation status

The owner reports that the triangle runs on their iPad mini A17 Pro, and that the
default Actions build also succeeded after adding the local math library. Those are
the baseline results, not new hardware results from this editing environment.

The new selector and the `rerbr-3d` configuration have only been checked from Windows.
A new Actions run selecting `rerbr-3d` is required before claiming that feature compiles
for iOS. Subsequent device checks remain separate. Local results are recorded in
`VALIDATION.md`; they do not establish new iPadOS compatibility.

## References

- [Winit 0.30.13 iOS entry-point and lifecycle guidance](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform/ios.rs)
- [wgpu 30.0.1 features: Metal and WGSL](https://github.com/gfx-rs/wgpu/blob/v30.0.1/wgpu/Cargo.toml)
- [Rust iOS device and simulator targets](https://doc.rust-lang.org/rustc/platform-support/apple-ios.html)
- [macOS 15 ARM64 runner inventory](https://github.com/actions/runner-images/blob/main/images/macos/macos-15-arm64-Readme.md)
- [Apple Xcode requirements](https://developer.apple.com/xcode/system-requirements)
- [Apple library search path build setting](https://developer.apple.com/documentation/xcode/build-settings-reference)
