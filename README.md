# ReRBRProbe

An independent iPadOS graphics experiment: Rust, winit **0.30.13**, wgpu **30.0.1**,
and the **Metal** backend. It draws a dark background and a rotating, coloured triangle.
Touch the screen to switch the background. It includes no reRBR crates, physics, SDL,
audio, Richard Burns Rally files, RallySimFans files, textures or downloads of game content.

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
| `xcode/ReRBRProbe/main.m` | Native entry, diagnostic file, call into the Rust static library |
| `xcode/ReRBRProbe/Info.plist`, `LaunchScreen.storyboard` | iPad bundle configuration and launch screen |
| `xcode/ReRBRProbe.xcodeproj/` | Minimal Xcode project and shared Release scheme |
| `scripts/build-ios.sh` | Build the static library and app, package and inspect the unsigned IPA |
| `scripts/verify.py` | Standard-library source/package checks (Python 3.11+) |
| `github-actions/ios-probe.yml` | Manual GitHub Actions workflow template |

## Build on GitHub Actions from Windows

The workflow runs on the standard `macos-15` runner and explicitly selects
`/Applications/Xcode_16.4.app/Contents/Developer`. Xcode 16.4 includes the iOS 18.5
SDK; the deployment target is **iPadOS 18.0**, and the device architecture is **arm64**.
The runner must continue to offer that Xcode installation. Its first step checks this;
if GitHub removes it, deliberately update both workflow copies and the cache key to a
supported Xcode version. No Apple account, signing certificate, paid membership or
GitHub signing secrets are used to build the unsigned IPA.

1. Put this experiment in a GitHub repository, either alongside reRBR or by itself.
   The workflow paths assume it is a directory named `ios-probe` at the repository root.
2. GitHub discovers workflows **only** under the repository root's `.github/workflows/`.
   The activated file must be `.github/workflows/ios-probe.yml`, copied from
   `ios-probe/github-actions/ios-probe.yml`. The template alone is not discoverable.
   Keep the two copies identical. Writing outside `ios-probe` requires the owner's permission.
3. Commit the new files, including `ios-probe/Cargo.lock`, and push to your own repository.
   Ensure the workflow is present on the **default branch** so the manual Run workflow
   button appears. Enable Actions if this is a fork with workflows disabled.
4. Open **Actions > ReRBRProbe unsigned iPadOS > Run workflow**.
5. Download `ReRBRProbe-unsigned-<run number>`. Extract the artifact ZIP on Windows to obtain
   `ReRBRProbe-unsigned.ipa` and `SHA256SUMS`. The artifact ZIP itself is not the IPA.
6. Check the IPA hash with `Get-FileHash .\ReRBRProbe-unsigned.ipa -Algorithm SHA256`.

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

From `ios-probe`, on a machine with Rust 1.97.1 and the required build tools:

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
```

The script writes only this directory's `target/` and `build/`. It refuses to overwrite a
previous Payload/IPA; start from a fresh checkout or move `ios-probe/build` aside to rerun.
Do not run the parent reRBR build/check script for this isolated experiment.

## Validation status

The iOS/Xcode build, unsigned IPA generation, installation, Metal rendering and visual
checks **have not been executed in this Windows environment**. The first Actions run is
required to validate Apple-specific compilation, native framework linking and storyboard
compilation. Hardware verification then requires the actual iPad. Local check results are
recorded in `VALIDATION.md`; they do not establish iOS compatibility.

## References

- [Winit 0.30.13 iOS entry-point and lifecycle guidance](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform/ios.rs)
- [wgpu 30.0.1 features: Metal and WGSL](https://github.com/gfx-rs/wgpu/blob/v30.0.1/wgpu/Cargo.toml)
- [Rust iOS device and simulator targets](https://doc.rust-lang.org/rustc/platform-support/apple-ios.html)
- [macOS 15 ARM64 runner inventory](https://github.com/actions/runner-images/blob/main/images/macos/macos-15-arm64-Readme.md)
- [Apple Xcode requirements](https://developer.apple.com/xcode/system-requirements)
