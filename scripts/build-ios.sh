#!/bin/bash
# Run with bash; no executable bit is required when checked out from Windows.
set -euo pipefail

PROBE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROBE_ROOT"
PROBE_MODE="${PROBE_MODE:-triangle}"
# Keep arrays nonempty for macOS's system Bash 3.2 with `set -u`.
CARGO_FEATURE_ARGS=(--no-default-features)
HOST_TEST_ARGS=(--test shader)
case "$PROBE_MODE" in
    triangle) ;;
    rerbr-3d)
        CARGO_FEATURE_ARGS+=(--features rerbr-3d)
        HOST_TEST_ARGS=(--workspace)
        ;;
    harwood-block)
        CARGO_FEATURE_ARGS+=(--features harwood-block)
        HOST_TEST_ARGS=(--workspace)
        ;;
    harwood-neighbors)
        CARGO_FEATURE_ARGS+=(--features harwood-neighbors)
        HOST_TEST_ARGS=(--workspace)
        ;;
    harwood-benchmark)
        CARGO_FEATURE_ARGS+=(--features harwood-benchmark)
        HOST_TEST_ARGS=(--workspace)
        ;;
    *)
        echo "Unknown PROBE_MODE: $PROBE_MODE (expected triangle, rerbr-3d, harwood-block, harwood-neighbors or harwood-benchmark)." >&2
        exit 1
        ;;
esac
if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "This build requires macOS and full Xcode with the iPhoneOS SDK." >&2
    exit 1
fi

# Resolve everything from the script's parent, whether the probe is a repository root
# or a local subdirectory. Never use the caller's Cargo workspace or target directory.
MANIFEST="$PROBE_ROOT/Cargo.toml"
RUST_TARGET=aarch64-apple-ios
export CARGO_TARGET_DIR="$PROBE_ROOT/target"
RUST_LIBRARY_DIR="$CARGO_TARGET_DIR/$RUST_TARGET/release"
RUST_STATIC_LIBRARY="$RUST_LIBRARY_DIR/librerbr_probe.a"
export IPHONEOS_DEPLOYMENT_TARGET=18.0
export SDKROOT="$(xcrun --sdk iphoneos --show-sdk-path)"
mkdir -p build

{
    echo "Probe build mode: $PROBE_MODE"
    printf 'Cargo feature flags:'
    printf ' %s' "${CARGO_FEATURE_ARGS[@]}"
    printf '\n'
    uname -m
    xcodebuild -version
    xcrun --sdk iphoneos --show-sdk-version
    rustc --version
    cargo --version
} | tee build/environment.txt

python3 scripts/verify.py
cargo fmt --manifest-path "$MANIFEST" --all -- --check
# Run shader validation on the macOS host. Optional modes also run the imported
# camera/reader unit tests and their feature-gated integration tests.
# These are not device/rendering tests.
# SDKROOT must not point to iPhoneOS when compiling macOS test executables.
env -u SDKROOT -u IPHONEOS_DEPLOYMENT_TARGET cargo test --manifest-path "$MANIFEST" --locked "${CARGO_FEATURE_ARGS[@]}" "${HOST_TEST_ARGS[@]}" 2>&1 | tee build/host-tests.log
cargo clippy --manifest-path "$MANIFEST" --locked --lib --target "$RUST_TARGET" "${CARGO_FEATURE_ARGS[@]}" -- -D warnings
cargo rustc --manifest-path "$MANIFEST" --locked --release --lib --target "$RUST_TARGET" "${CARGO_FEATURE_ARGS[@]}" -- --print native-static-libs 2>&1 | tee build/rust-build.log
cargo tree --manifest-path "$MANIFEST" --locked --target "$RUST_TARGET" "${CARGO_FEATURE_ARGS[@]}" --edges normal --depth 2 > build/rust-dependencies.txt

# Xcode links the archive as -lrerbr_probe. Verify Cargo produced it before asking
# Xcode to link, and supply the same directory to its LIBRARY_SEARCH_PATHS setting.
if [[ ! -s "$RUST_STATIC_LIBRARY" ]]; then
    echo "::error::Cargo did not produce the expected static library: $RUST_STATIC_LIBRARY" >&2
    exit 1
fi
{
    echo "Rust target: $RUST_TARGET"
    echo "Cargo target directory: $CARGO_TARGET_DIR"
    echo "Static library: $RUST_STATIC_LIBRARY"
    ls -lh "$RUST_STATIC_LIBRARY"
    xcrun lipo -info "$RUST_STATIC_LIBRARY"
    # -verify_arch consumes all following arguments as architecture names.
    xcrun lipo "$RUST_STATIC_LIBRARY" -verify_arch arm64
} 2>&1 | tee build/rust-library.txt

plutil -lint xcode/ReRBRProbe/Info.plist xcode/ReRBRProbe.xcodeproj/project.pbxproj
xcodebuild -list -project xcode/ReRBRProbe.xcodeproj
XCODE_ARGS=(
    -project "$PROBE_ROOT/xcode/ReRBRProbe.xcodeproj"
    -scheme ReRBRProbe
    -configuration Release
    -sdk iphoneos
    -destination 'generic/platform=iOS'
    -derivedDataPath "$PROBE_ROOT/build/DerivedData"
    "RUST_LIBRARY_DIR=$RUST_LIBRARY_DIR"
    CODE_SIGNING_ALLOWED=NO
    CODE_SIGNING_REQUIRED=NO
    'CODE_SIGN_IDENTITY='
)
# Use identical arguments for diagnosis and build, including the absolute Rust path.
xcodebuild "${XCODE_ARGS[@]}" -showBuildSettings > build/xcode-settings.txt
xcodebuild "${XCODE_ARGS[@]}" build 2>&1 | tee build/xcode-build.log

APP="$PROBE_ROOT/build/DerivedData/Build/Products/Release-iphoneos/ReRBRProbe.app"
test -f "$APP/ReRBRProbe"
test "$(xcrun lipo -archs "$APP/ReRBRProbe")" = arm64
xcrun vtool -show-build "$APP/ReRBRProbe" | tee build/macho-build.txt
grep -Eq 'platform[[:space:]]+IOS$' build/macho-build.txt

# A directory called .ipa is not sufficient: this must be a zip with Payload/*.app.
# Refuse a leftover package so a failed rerun cannot upload an old IPA.
if [[ -e build/Payload || -e build/ReRBRProbe-unsigned.ipa ]]; then
    echo "Old package exists. Use a fresh checkout or move $PROBE_ROOT/build aside before retrying." >&2
    exit 1
fi
mkdir build/Payload
ditto "$APP" build/Payload/ReRBRProbe.app
(
    cd build
    /usr/bin/zip -qry ReRBRProbe-unsigned.ipa Payload
)
python3 scripts/verify.py --ipa build/ReRBRProbe-unsigned.ipa
(
    cd build
    shasum -a 256 ReRBRProbe-unsigned.ipa > SHA256SUMS
)
echo "Unsigned IPA packaged and inspected. Device execution has NOT been tested."
