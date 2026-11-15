# Upstream source and scope

Selected file readers from the owner's local ReRBR source snapshot, inspected on
2026-11-14. The source repository named by its development guide is
<https://github.com/JodusNodus/rerbr>. The original rbr-formats package and this
integration are **GPL-3.0-or-later**; the complete license is in LICENSE.

The local source checkout has no Git metadata.
No upstream commit ID is claimed. These SHA-256 values identify the imported
bytes. Every listed file, including unit tests and formatting configuration,
is copied unchanged.

| Local file | Original source | SHA-256 |
| --- | --- | --- |
| LICENSE | LICENSE | `3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986` |
| rustfmt.toml | rustfmt.toml | `93ed1668bb2ca659e88711456b72320bf2f53369ecf278c4a61af0cb63cc3a1e` |
| src/damage.rs | crates/rbr-formats/src/damage.rs | `1a9ebcc4729d4cf9fc1ec1c4bed98d436e3c65e3240c850c6e22ff0bdc465d46` |
| src/dds.rs | crates/rbr-formats/src/dds.rs | `61ce085c58c063ee81e1127146d31731c4a5299f0570e06f0b33326d9b2f316b` |
| src/ini.rs | crates/rbr-formats/src/ini.rs | `ddc7aede9b45b30dded36d67800179cf4ea68f9da1293bfb6b14cd1798e0f162` |
| src/le.rs | crates/rbr-formats/src/le.rs | `043b0d9b1f8537721adfb9d5d3670a3f3089ed158141bbe52542acfe14793880` |
| src/rbz.rs | crates/rbr-formats/src/rbz.rs | `c0f858ad759ec7c6a1f4e136a09a616c2f66551e6c2dcfa1dd899fd9c2460188` |
| src/stage_textures.rs | crates/rbr-formats/src/stage_textures.rs | `dd8b149084fe353f2746c1ef1e69bcfe419c75767e1e3d5394f951164b0d15eb` |
| src/lbs/mod.rs | crates/rbr-formats/src/lbs/mod.rs | `63f7ee66a4228aeca35849513e47aabb8f9344ec5cf8e699cbbcd61f474910c7` |
| src/lbs/read.rs | crates/rbr-formats/src/lbs/read.rs | `6c602e3042a5c9150420caaac93b6f5c50dd17083ced73e6dbea6d785d5462ee` |
| src/lbs/tests.rs | crates/rbr-formats/src/lbs/tests.rs | `8999a34ebc9f0aacc182f481f62ab3e88dfc602c5008e3fd0c01f75f586fb118` |

The new Cargo.toml and src/lib.rs select only LBS, INI, texture-name mapping,
RBZ/ZIP and DDS readers. The shared le module retains helpers unused by this
selection; its dead-code allowance keeps the original file intact. damage.rs
is only a malformed-input test helper, not car damage or physics.

The upstream manifest requests zip 9.0.1 with deflate-flate2-zlib-rs; that compatible
version range includes the pinned zip 9.0.3 available in the local Cargo cache.
No executable reader, iced-x86, NGP, SDL, sound, car loader, TRK/DLS reader or
renderer is imported. Some upstream comments refer to the complete project.

App-side integration lives in src/asset_probe.rs of the host package. Its request
selection follows crates/rbr-render/src/stage/textures.rs and reference gathering
follows stage/mesh.rs: stage geometry, object lanes, sky and interactive references.
It audits all declared diffuse/specular entries too. Water's runtime manager keys
and global clouds are outside this diagnostic; no rendering coverage is claimed.
These adapter files are new GPL-3.0-or-later code, not byte-identical imports.

No proprietary data, extracted textures, game binaries or RSF content is included.
Runtime files stay in the owner's installation or the iPad app's Documents folder.

Stage 5 adds the following byte-identical original readers, including their
synthetic unit tests. They use the existing INI/le helpers and the standard library,
with no new dependency. The framing cursor retains unused SGS/SGA methods under a
module-level dead-code allowance in our lib.rs; no imported file is edited.

| Local file | Original source | SHA-256 |
| --- | --- | --- |
| src/sgc.rs | crates/rbr-formats/src/sgc.rs | `1a4304016a9a8b90f0f52893540e585575da67155ec7b74efc8e980a04a7129e` |
| src/sg_framing.rs | crates/rbr-formats/src/sg_framing.rs | `7612205644acdafe07c3d39793308b15ed2791b5b21653445080321a1306dfe8` |
| src/cars.rs | crates/rbr-formats/src/cars.rs | `96223b1f147579d2fccf8f3a84e3550d7999ed50899a8e98c4e7912d56696c6f` |
| src/car_parts.rs | crates/rbr-formats/src/car_parts.rs | `62570b710fa93e00283bfb2a2de9a524f7ad3a660309da989c82da6c91082846` |

The app adapts car mesh conversion/material states under src/car/UPSTREAM.md.
Only data parsing is added here: no executable game code, DirectX shader loader,
car physics, RSF importer or renderer is pulled into this crate.

