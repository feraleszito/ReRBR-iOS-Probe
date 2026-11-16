"""Source/package preflight using only Python 3.11+ standard-library modules."""

import argparse
import hashlib
import json
import os
import plistlib
import re
import struct
import tomllib
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PRIVATE_ASSET_SUFFIXES = {".lbs", ".dds", ".rbz", ".sgc", ".sgs", ".lsp", ".dll", ".exe", ".bin", ".trk", ".dls", ".col", ".fnc", ".mat", ".zip", ".vso", ".pso"}
CPU_TABLES = {"vendor/rerbr-sim/upstream/rbr-physics/data/rsqrt.bin",
              "vendor/rerbr-sim/upstream/rbr-physics/data/rcp.bin"}


def is_private_asset(name):
    path = Path(name)
    return (path.suffix.lower() in PRIVATE_ASSET_SUFFIXES
            or path.suffix.lower() == ".ini"
            or path.name.lower() == "texturefilenamemap.dat"
            or (path.name.lower().startswith("track-") and path.suffix.lower() == ".ini"))


def require(condition, message):
    if not condition:
        raise SystemExit(f"FAIL: {message}")


def verify_sources():
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    require("workspace" in manifest, "probe must have an independent workspace")
    require("staticlib" in manifest["lib"]["crate-type"], "Rust library must include staticlib")
    dependencies = manifest["dependencies"]
    require(set(dependencies) == {"wgpu", "winit", "futures-executor", "rerbr-render-core", "rerbr-asset-formats", "rerbr-sim"},
            "unexpected runtime dependency")
    require(dependencies["rerbr-render-core"] == {"path": "vendor/rerbr-render-core", "optional": True},
            "render core must be a local optional dependency")
    require(dependencies["rerbr-asset-formats"] == {"path": "vendor/rerbr-asset-formats", "optional": True},
            "asset readers must be a local optional dependency")
    require(dependencies["rerbr-sim"] == {"path": "vendor/rerbr-sim", "optional": True},
            "physics must remain an optional local dependency")
    require(manifest["features"]["default"] == [], "triangle must remain the default")
    require(manifest["features"]["rerbr-3d"] == ["dep:rerbr-render-core", "dep:rerbr-asset-formats"],
            "unexpected 3D feature dependencies")
    require(manifest["features"]["harwood-block"] == ["dep:rerbr-render-core", "dep:rerbr-asset-formats"],
            "unexpected Harwood feature dependencies")
    require(manifest["features"]["harwood-neighbors"] == ["dep:rerbr-render-core", "dep:rerbr-asset-formats"],
            "unexpected neighbor feature dependencies")
    require(manifest["features"]["harwood-benchmark"] == ["harwood-neighbors"],
            "benchmark must reuse the same neighbor workload dependencies")
    require(manifest["features"]["car-chase-benchmark"] == ["harwood-benchmark"],
            "car benchmark must reuse the current terrain/clock dependencies")
    require(manifest["features"]["physics-probe"] == ["dep:rerbr-sim"],
            "CPU probe must not activate the terrain/car benchmark")
    require(manifest["workspace"]["members"] == ["vendor/rerbr-render-core", "vendor/rerbr-asset-formats", "vendor/rerbr-sim", "vendor/rerbr-sim/upstream/*"],
            "unexpected workspace member")
    require(manifest["workspace"]["default-members"] == ["."], "default build must select the triangle app")
    require(dependencies["wgpu"]["version"] == "=30.0.1", "wgpu version must match the probe")
    require(dependencies["winit"]["version"] == "=0.30.13", "winit version must match the probe")
    require(set(dependencies["wgpu"]["features"]) == {"std", "metal", "wgsl"}, "Metal-only wgpu features")
    require(dependencies["wgpu"]["default-features"] is False, "disable default wgpu backends")
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    versions = {p["name"]: p["version"] for p in lock["package"]}
    for name, version in (("wgpu", "30.0.1"), ("winit", "0.30.13"), ("futures-executor", "0.3.34"),
                          ("rerbr-render-core", "0.1.0"), ("glam", "0.33.12"),
                          ("rerbr-asset-formats", "0.1.0"), ("zip", "9.0.3")):
        require(versions.get(name) == version, f"locked {name} version")
    expected_sim = {"rbr-physics", "rbr-hybrid", "rbr-formats", "rbr-gameplay", "rbr-recording", "rbr-engine", "rbr-tyre", "rbr-school"}
    require({name for name in versions if name.startswith("rbr-")} == expected_sim,
            "unexpected physics dependency closure")
    require(versions.get("rerbr-sim") == "0.1.0", "locked local physics adapter")
    sim = ROOT / "vendor/rerbr-sim"
    upstream = json.loads((sim / "UPSTREAM-HASHES.json").read_text(encoding="utf-8-sig"))
    patches = json.loads((sim / "PATCHES.json").read_text(encoding="utf-8-sig"))
    require(len(upstream) == 623, "missing upstream physics sources or CPU reference tables")
    require(set(patches) == {"upstream/rbr-physics/src/mem.rs", "upstream/rbr-physics/src/mem/pages.rs",
                             "upstream/rbr-formats/src/ngp/mod.rs"}, "unexpected adapted upstream file")
    for entry in upstream:
        relative = entry["Path"]
        expected = patches.get(relative, entry["SHA256"])
        require(hashlib.sha256((sim / relative).read_bytes()).hexdigest() == expected,
                f"physics source/provenance mismatch: {relative}")
    original_pages = next(e["SHA256"] for e in upstream if e["Path"] == "upstream/rbr-physics/src/mem/pages.rs")
    require(hashlib.sha256((sim / "upstream/rbr-physics/src/mem/pages_flat.rs").read_bytes()).hexdigest() == original_pages,
            "original host-memory reference changed")
    for name in expected_sim:
        package = tomllib.loads((sim / "upstream" / name / "Cargo.toml").read_text())
        require(package["package"]["license"] == "GPL-3.0-or-later", "physics license changed")
        if name == "rbr-hybrid":
            require(package["lib"]["crate-type"] == ["rlib"], "device dependency must not build the desktop harness cdylib")
        require((sim / "upstream" / name / "LICENSE").read_bytes() == (sim / "LICENSE").read_bytes(),
                "missing retained GPL license")
    core = ROOT / "vendor/rerbr-render-core"
    core_manifest = tomllib.loads((core / "Cargo.toml").read_text())
    require(core_manifest["package"]["license"] == "GPL-3.0-or-later", "retain the upstream license")
    require(core_manifest["dependencies"] == {
        "glam": "=0.33.12",
        "wgpu": {"version": "=30.0.1", "default-features": False, "features": ["std"]},
    }, "render core must contain only coordinate/camera dependencies")
    provenance = (core / "UPSTREAM.md").read_text()
    imported = dict(re.findall(
        r"^\| (src/\S+|LICENSE|rustfmt\.toml) \| [^|]+ \| `([0-9a-f]{64})` \|$",
        provenance, re.MULTILINE,
    ))
    require(set(imported) == {"src/coords.rs", "src/camera.rs", "src/triple.rs", "LICENSE", "rustfmt.toml"},
            "missing upstream provenance hashes")
    for relative, expected in imported.items():
        require(hashlib.sha256((core / relative).read_bytes()).hexdigest() == expected,
                f"upstream bytes changed: {relative}")
    assets = ROOT / "vendor/rerbr-asset-formats"
    asset_manifest = tomllib.loads((assets / "Cargo.toml").read_text())
    require(asset_manifest["package"]["license"] == "GPL-3.0-or-later", "retain asset-reader license")
    require(asset_manifest["dependencies"] == {
        "zip": {"version": "=9.0.3", "default-features": False, "features": ["deflate-flate2-zlib-rs"]},
    }, "asset readers must use only the selected pure-Rust ZIP backend")
    asset_imports = dict(re.findall(
        r"^\| (src/\S+|LICENSE|rustfmt\.toml) \| [^|]+ \| `([0-9a-f]{64})` \|$",
        (assets / "UPSTREAM.md").read_text(), re.MULTILINE,
    ))
    require(set(asset_imports) == {
        "LICENSE", "rustfmt.toml", "src/le.rs", "src/ini.rs", "src/dds.rs", "src/rbz.rs",
        "src/stage_textures.rs", "src/damage.rs", "src/lbs/mod.rs", "src/lbs/read.rs", "src/lbs/tests.rs",
        "src/sgc.rs", "src/sg_framing.rs", "src/cars.rs", "src/car_parts.rs",
    }, "missing asset-reader provenance hashes")
    for relative, expected in asset_imports.items():
        require(hashlib.sha256((assets / relative).read_bytes()).hexdigest() == expected,
                f"asset-reader upstream bytes changed: {relative}")
    for directory, children, files in os.walk(ROOT):
        children[:] = [name for name in children if name not in {".git", ".validation", "target", "build"}]
        for name in files:
            relative = (Path(directory) / name).relative_to(ROOT).as_posix()
            require(not is_private_asset(name) or relative in CPU_TABLES,
                    f"private game resource in source tree: {Path(directory) / name}")
    for relative in (
        "xcode/ReRBRProbe/LaunchScreen.storyboard",
        "xcode/ReRBRProbe.xcodeproj/xcshareddata/xcschemes/ReRBRProbe.xcscheme",
    ):
        ET.parse(ROOT / relative)
    with (ROOT / "xcode/ReRBRProbe/Info.plist").open("rb") as stream:
        info = plistlib.load(stream)
    require(info["CFBundlePackageType"] == "APPL", "application package type")
    require("UIApplicationSceneManifest" not in info, "winit owns the application lifecycle")
    require(info.get("UIFileSharingEnabled") is True and info.get("LSSupportsOpeningDocumentsInPlace") is True,
            "private Documents transfer must remain enabled")
    project = (ROOT / "xcode/ReRBRProbe.xcodeproj/project.pbxproj").read_text()
    definitions = re.findall(r"^\s*([A-F0-9]{24})(?: /\*.*?\*/)? = \{", project, re.MULTILINE)
    require(len(definitions) == len(set(definitions)), "duplicate Xcode object IDs")
    references = set(re.findall(r"\b[A-F0-9]{24}\b", project))
    require(references == set(definitions), "unresolved Xcode object reference")
    require((ROOT / "src/triangle.wgsl").is_file(), "missing embedded shader")
    for relative in ("src/scene3d.rs", "src/scene_geometry.rs", "src/scene3d.wgsl"):
        require((ROOT / relative).is_file(), f"missing optional scene source: {relative}")
    require((ROOT / "src/asset_probe.rs").is_file(), "missing optional private-file diagnostic")
    for relative in ("mod.rs", "mesh.rs", "texture.rs", "gpu.rs", "shader.wgsl", "UPSTREAM.md",
                     "neighbors.rs", "neighbors_tests.rs"):
        require((ROOT / "src/harwood" / relative).is_file(), f"missing Harwood source: {relative}")
    for relative in ("mod.rs", "route.rs", "metrics.rs", "hud.rs", "hud.wgsl", "tests.rs"):
        require((ROOT / "src/harwood/benchmark" / relative).is_file(), f"missing benchmark source: {relative}")
    for relative in ("mod.rs", "selection.rs", "materials.rs", "mesh.rs", "placement.rs", "gpu.rs",
                     "runtime.rs", "shader.wgsl", "UPSTREAM.md", "tests.rs"):
        require((ROOT / "src/car" / relative).is_file(), f"missing Corolla source: {relative}")
    print("PASS: independent manifest, optional libraries, upstream hashes, no game resources, locked versions, Xcode references, plist and XML")


def verify_ipa(path):
    with zipfile.ZipFile(path) as archive:
        require(archive.testzip() is None, "corrupt IPA zip")
        names = set(archive.namelist())
        require(not any(is_private_asset(name) for name in names), "private game resource found in IPA")
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
