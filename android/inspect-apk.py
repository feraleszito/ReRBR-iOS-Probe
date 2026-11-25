"""Inspect the packaged payload, ARM64 ELF, native entry points and load alignment."""
from pathlib import Path
import os
import subprocess
import sys
import tempfile
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from verify import is_private_asset

apk = Path(sys.argv[1])
abi_lib = "lib/arm64-v8a/librerbr_probe.so"
tool = Path(os.environ["ANDROID_NDK_HOME"]) / "toolchains/llvm/prebuilt"
readelf = next(tool.glob("*/bin/llvm-readelf*"))
with zipfile.ZipFile(apk) as archive, tempfile.TemporaryDirectory() as tmp:
    names = archive.namelist()
    if archive.testzip() or len(set(names)) != len(names):
        sys.exit("Corrupt APK or duplicate entries")
    if {n for n in names if n.endswith(".so")} != {abi_lib}:
        sys.exit("Unexpected native libraries")
    for name in names:
        if is_private_asset(name):
            sys.exit(f"Unexpected resource: {name}")
    assets = {n for n in names if n.startswith("assets/")}
    if assets != {"assets/UFL.txt", "assets/NOTICE.md", "assets/LICENSE"}:
        sys.exit(f"Unexpected game assets: {assets}")
    for name in ("classes.dex", "AndroidManifest.xml"):
        archive.getinfo(name)
    binary = archive.extract(abi_lib, tmp)
    def inspect(*args):
        return subprocess.check_output([readelf, *args, binary], text=True)
    header = inspect("-h")
    if "AArch64" not in header or "ELF64" not in header or "DYN" not in header:
        sys.exit("Invalid ARM64 shared library")
    loads = [line.split() for line in inspect("-lW").splitlines() if line.lstrip().startswith("LOAD ")]
    if not loads or any(int(row[-1], 16) < 16384 for row in loads):
        sys.exit("ELF load segment alignment below 16 KiB")
    symbols = {line.split()[-1] for line in inspect("--dyn-syms", "--wide").splitlines() if line.split() and " UND " not in line}
    for symbol in (
        "ANativeActivity_onCreate",
        "Java_org_rerbr_mobile_GameActivity_sensorRoll",
        "Java_org_rerbr_mobile_GameActivity_safeArea",
        "Java_org_rerbr_mobile_GameActivity_importStatus",
        "Java_org_rerbr_mobile_GameActivity_persistedUriState",
        "Java_org_rerbr_mobile_GameActivity_nativeNormalizeRbrRel",
        "Java_org_rerbr_mobile_GameActivity_nativeCheckPreserveCoreFile",
        "Java_org_rerbr_mobile_GameActivity_nativeImportZipFd",
        "Java_org_rerbr_mobile_GameAudio_render",
    ):
        if symbol not in symbols:
            sys.exit(f"Missing native entry point: {symbol}")
    print(inspect("-d"))
print("PASS: ARM64 game library, activity and JNI exports, 16 KiB segments and asset inventory")
