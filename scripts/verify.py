"""Source/package preflight using only Python 3.11+ standard-library modules."""

import argparse
import plistlib
import re
import struct
import tomllib
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise SystemExit(f"FAIL: {message}")


def verify_sources():
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    require("workspace" in manifest, "probe must have an independent workspace")
    require("staticlib" in manifest["lib"]["crate-type"], "Rust library must include staticlib")
    dependencies = manifest["dependencies"]
    require(set(dependencies) == {"wgpu", "winit", "futures-executor"}, "unexpected runtime dependency")
    require(dependencies["wgpu"]["version"] == "=30.0.1", "wgpu version must match the probe")
    require(dependencies["winit"]["version"] == "=0.30.13", "winit version must match the probe")
    require(set(dependencies["wgpu"]["features"]) == {"std", "metal", "wgsl"}, "Metal-only wgpu features")
    require(dependencies["wgpu"]["default-features"] is False, "disable default wgpu backends")
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    versions = {p["name"]: p["version"] for p in lock["package"]}
    for name, version in (("wgpu", "30.0.1"), ("winit", "0.30.13"), ("futures-executor", "0.3.34")):
        require(versions.get(name) == version, f"locked {name} version")
    require(not any(name.startswith("rbr-") for name in versions), "probe must not depend on game crates")
    for relative in (
        "xcode/ReRBRProbe/LaunchScreen.storyboard",
        "xcode/ReRBRProbe.xcodeproj/xcshareddata/xcschemes/ReRBRProbe.xcscheme",
    ):
        ET.parse(ROOT / relative)
    with (ROOT / "xcode/ReRBRProbe/Info.plist").open("rb") as stream:
        info = plistlib.load(stream)
    require(info["CFBundlePackageType"] == "APPL", "application package type")
    require("UIApplicationSceneManifest" not in info, "winit owns the application lifecycle")
    project = (ROOT / "xcode/ReRBRProbe.xcodeproj/project.pbxproj").read_text()
    definitions = re.findall(r"^\s*([A-F0-9]{24})(?: /\*.*?\*/)? = \{", project, re.MULTILINE)
    require(len(definitions) == len(set(definitions)), "duplicate Xcode object IDs")
    references = set(re.findall(r"\b[A-F0-9]{24}\b", project))
    require(references == set(definitions), "unresolved Xcode object reference")
    require((ROOT / "src/triangle.wgsl").is_file(), "missing embedded shader")
    print("PASS: independent manifest, locked versions, Xcode references, plist and XML")


def verify_ipa(path):
    with zipfile.ZipFile(path) as archive:
        require(archive.testzip() is None, "corrupt IPA zip")
        names = set(archive.namelist())
        prefix = "Payload/ReRBRProbe.app/"
        require(all(name.startswith("Payload/") for name in names), "IPA contains files outside Payload")
        require(not any("_CodeSignature/" in name or name.endswith("embedded.mobileprovision") for name in names),
                "experimental IPA must have no signing profile/signature resources")
        info = plistlib.loads(archive.read(prefix + "Info.plist"))
        require(info["CFBundleIdentifier"] == "org.rerbr.probe", "bundle ID must be expanded")
        require(info["CFBundleExecutable"] == "ReRBRProbe", "executable name must be expanded")
        require(info["UIDeviceFamily"] == [2], "iPad-only device family")
        require(info["MinimumOSVersion"] == "18.0", "deployment target must be iPadOS 18.0")
        require(any(name.startswith(prefix + "LaunchScreen.storyboardc/") for name in names), "compiled launch screen missing")
        binary = archive.read(prefix + "ReRBRProbe")
        require(len(binary) >= 32, "truncated executable")
        magic, cpu, _, filetype, commands, _, _, _ = struct.unpack_from("<8I", binary)
        require(magic == 0xFEEDFACF and cpu == 0x0100000C and filetype == 2, "not an ARM64 Mach-O executable")
        offset = 32
        platforms = []
        for _ in range(commands):
            require(offset + 8 <= len(binary), "truncated Mach-O load command")
            command, size = struct.unpack_from("<2I", binary, offset)
            require(size >= 8 and offset + size <= len(binary), "invalid Mach-O load command size")
            require(command != 0x1D, "LC_CODE_SIGNATURE found: IPA is not unsigned")
            if command == 0x32:  # LC_BUILD_VERSION: platform 2 = physical iOS, 7 = simulator.
                require(size >= 24, "invalid LC_BUILD_VERSION")
                platforms.append(struct.unpack_from("<I", binary, offset + 8)[0])
            offset += size
        require(platforms == [2], "executable must target physical iOS, not simulator or macOS")
    print("PASS: unsigned IPA, compiled launch screen, arm64 executable, physical iOS platform")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ipa", type=Path)
    args = parser.parse_args()
    verify_sources()
    if args.ipa:
        verify_ipa(args.ipa)
