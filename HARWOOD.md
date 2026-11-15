# Harwood Forest: private-file diagnostic, stage 1

The owner reports that this stage passed on the physical iPad mini A17 Pro on
2026-11-15, with the counts below, adapter BC=true and device BC=false. The new
independent visual experiment is documented in [HARWOOD-BLOCK.md](HARWOOD-BLOCK.md).

This batch parses real files without drawing them. The default triangle and the
feature's procedural floor/cube retain their existing shaders, buffers and drawing.
`rerbr-3d` also starts a one-shot file audit on a background thread after GPU creation.
The diagnostic never extracts, edits, repairs or downloads game content. A missing
folder skips the audit; incomplete data reports an error and leaves the scene running.

## What was actually checked on Windows

The original imported Rust LBS parser read `track-20_O.lbs` directly from the owner's
RSF Maps folder. The same code used by the iPad diagnostic produced:

```text
blocks=282 (expected 282), visibility=282, object items=442
INI diffuse=39, specular=5; required DDS=38/38; declared DDS=44/44; redirected to N=41
required asset diagnostic PASS
```

These counts are texture **references**, not unique image files. The 38 references
used by geometry/object/sky/interactive draws are 37 diffuse references and one
specular reference. All 44 declared diffuse/specular references were readable,
including the six not used by those draws. All DDS reads go through the original
parser and the complete ZIP entry is read, checking its CRC. Referenced images are
DXT1/BC1 or DXT3/BC2 with four mips, up to 2048 x 2048.

The request selection is dry/new. `TextureFilenameMap.dat` redirects some of those
requests to `damp` inside N. That is an explicit filename mapping, not a weather
fallback added by the probe. Transfer both RBZ files intact, including their internal
prefix directory and every weather folder.

Only stage-local LBS references and the INI table are audited. Water's runtime
texture-manager keys and global clouds are not interpreted; no complete stage
rendering, camera, car, physics, audio or benchmark timing is implemented.

## Reproduce the CPU diagnostic

Run from the standalone probe root in PowerShell, with your own Maps path:

```powershell
cargo run --locked --example harwood-diagnostic --no-default-features --features rerbr-3d -- 'C:\Richard Burns Rally\Maps'
```

It prints each request, resolved archive path, required/unused status, DDS format,
dimensions and mip count. Missing required data or a geometry count other than
282 blocks/282 visibility cells returns exit code 1. Missing unused INI entries
remain visible in the report but do not fail the required-data result.

The private-install test is ignored in normal CI. Run it explicitly:

```powershell
$env:HARWOOD_MAPS = 'C:\Richard Burns Rally\Maps'
cargo test --locked -p rerbr-probe --no-default-features --features rerbr-3d --test asset_loading real_harwood_installation -- --ignored --nocapture
```

Ordinary workspace tests use generated synthetic bytes, not extracted game fixtures.
They cover redirection, case/slash differences, missing archives, malformed DDS/LBS,
ZIP checksum failure, path traversal rejection, block counts, unused references,
and advertised BC/texture-size limits without opening a GPU.

## Transfer private data to the iPad

Build Actions with the existing **rerbr-3d** selection and install the app through
your existing signing/install process. GitHub never needs the game files. In a
private folder outside your checkout, prepare exactly these files from your install:

```text
Harwood/
  Maps/
    track-20_O.lbs
    track-20_O.ini
    TextureFilenameMap.dat
    track-20_O_textures.rbz
    track-20_N_textures.rbz
```

TRK, DLS, COL, FNC, MAT, Clouds, TrackSettings and Cars are not needed for stage 1.
Do not put the files, or a ZIP containing them, in the public repository or IPA.
The ignore rules and source/IPA preflight reject game resource files.

Transfer the private folder using Files or Apple Devices file sharing. If transferring
a ZIP, unpack it with Files first; the probe reads the folder, not an outer ZIP.
The final location must be:

```text
On My iPad / ReRBRProbe / Harwood / Maps / track-20_O.lbs
```

Avoid an extra `Harwood/Harwood` folder. Close and relaunch the app after transfer;
there is no hot reload. Keep both complete RBZs even though one is much smaller.
The existing Info.plist already enables file sharing. `main.m` obtains the sandbox's
Documents URL and passes it through `RERBR_PROBE_DOCUMENTS` before Rust/threads start.
No desktop install path is compiled into the app.

## Inspect the device log

Open `On My iPad / ReRBRProbe / rerbr-probe.log`. The cube and touch background
toggle should remain as before. Look for:

```text
harwood: adapter BC supported=true; device BC enabled=false; texture limit=...
harwood: ... required DDS=38/38; declared DDS=44/44; redirected to N=41
harwood: required asset diagnostic PASS
harwood: parsed data fits advertised capabilities; upload/rendering UNTESTED
```

The first line matches the owner's physical stage-1 report; the app queries it again on each run.
The adapter's actual flag is queried at runtime. The device's optional features
remain unchanged, so BC is not requested in this CPU-only batch; `enabled=false`
is expected and does not mean the adapter lacks BC. A capability warning reports
missing BC support or a required image larger than the device's configured limit.
No image is uploaded, and no Metal BC sampling test is performed.

Without private data, expect `SKIP: copy private files ... then relaunch`. A missing
archive or invalid entry produces a readable error instead of changing the scene.
Triangle builds do not start the Harwood diagnostic.

Stage 1's Apple build and CPU audit are now owner-confirmed. That does not verify
the new stage-2 upload/render path. The stage-1 batch did not change the workflow,
build script, Xcode project, lipo command, signature handling or IPA layout.

## Source changes to copy

Copy these paths from local `ios-probe` to the same paths at the root of
`ReRBR-iOS-Probe`, without adding an `ios-probe/` wrapper:

- Modified: `.gitignore`, `Cargo.toml`, `Cargo.lock`, `src/lib.rs`,
  `xcode/ReRBRProbe/main.m`, `scripts/verify.py`, `README.md`, `VALIDATION.md`.
- New: `HARWOOD.md`, `src/asset_probe.rs`, `examples/harwood-diagnostic.rs`,
  `tests/asset_loading.rs`, and the complete `vendor/rerbr-asset-formats/` directory.

The new vendor directory contains Cargo.toml, LICENSE, rustfmt.toml, UPSTREAM.md,
src/lib.rs, src/le.rs, src/ini.rs, src/dds.rs, src/rbz.rs, src/stage_textures.rs,
src/damage.rs and src/lbs/{mod.rs,read.rs,tests.rs}. Its provenance document records
the original paths and hashes. The existing `vendor/rerbr-render-core` is unchanged.
