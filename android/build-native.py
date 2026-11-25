"""Build the full Beta Rust library with the Android NDK selected by the caller."""
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
target = "aarch64-linux-android"
ndk = Path(os.environ["ANDROID_NDK_HOME"])
host = {"win32": "windows-x86_64", "darwin": "darwin-x86_64", "linux": "linux-x86_64"}[sys.platform]
toolbin = ndk / "toolchains/llvm/prebuilt" / host / "bin"
clang = toolbin / (target + "30-clang" + (".cmd" if os.name == "nt" else ""))
if not clang.is_file():
    sys.exit(f"NDK arm64 compiler missing: {clang}")
env = dict(os.environ, CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=str(clang),
           CC_aarch64_linux_android=str(clang), CARGO_PROFILE_RELEASE_PANIC="unwind",
           CARGO_TARGET_DIR=str(ROOT / "target"))
subprocess.run(["cargo", "rustc", "--locked", "--release", "--lib", "--target", target,
                "--no-default-features", "--features", "public-beta", "--crate-type", "cdylib",
                "--", "-C", "link-arg=-Wl,-z,max-page-size=16384"], cwd=ROOT, env=env, check=True)
dest = ROOT / "android/app/build/rust-jni/arm64-v8a/librerbr_probe.so"
dest.parent.mkdir(parents=True, exist_ok=True)
shutil.copyfile(ROOT / "target" / target / "release" / dest.name, dest)
strip = toolbin / ("llvm-strip.exe" if os.name == "nt" else "llvm-strip")
subprocess.run([strip, "--strip-unneeded", dest], check=True)
