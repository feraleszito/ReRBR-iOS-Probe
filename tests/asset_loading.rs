#![cfg(feature = "rerbr-3d")]

use rerbr_probe::asset_probe::{self, EXPECTED_BLOCKS};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

// These fixtures are generated from format descriptions, never extracted from a
// game. A single artificial triangle/4x4 DDS exercises the complete reader chain.
struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rerbr-harwood-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        let fixture = Self(path);
        fixture.write("TRACK-20_o.LBS", &geometry(EXPECTED_BLOCKS));
        fixture.write("track-20_O.ini", ini(1).as_bytes());
        fixture.remap("track-20_N_textures\\dry\\new\\test.dds");
        fixture.archive(&dds());
        fixture
    }

    fn write(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.0.join(name), bytes).unwrap();
    }

    fn remap(&self, target: &str) {
        let mut bytes = 1u32.to_le_bytes().to_vec();
        bytes.extend(b"track-20_o_textures\\dry\\new\\test.dds\0");
        bytes.extend(target.as_bytes());
        bytes.push(0);
        self.write("TextureFilenameMap.dat", &bytes);
    }

    fn archive(&self, bytes: &[u8]) {
        self.write(
            "TRACK-20_N_TEXTURES.RBZ",
            &stored_zip("TRACK-20_n_TEXTURES/DRY/NEW/TEST.DDS", bytes),
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ini(count: u32) -> String {
    format!("[TextureInfo]\nNumTextures={count}\nTexture0=TEST.tga\nTexture1=MISSING.tga\nNumSpecularTextures=0\nNumShadowTextures=0\n[TEST.tga]\nIsGroundTexture=true\n")
}

fn frame(kind: (u32, u32), payload: &[u8], out: &mut Vec<u8>) {
    for word in [8, kind.0, kind.1, payload.len() as u32] {
        out.extend(word.to_le_bytes());
    }
    out.extend(payload);
}

fn geometry(blocks: usize) -> Vec<u8> {
    let mut payload = (blocks as u32).to_le_bytes().to_vec();
    payload.extend(0u32.to_le_bytes());
    for block in 0..blocks {
        for slot in 0..9 {
            if block == 0 && slot == 5 {
                payload.extend(3u32.to_le_bytes());
                for i in [0u16, 1, 2] {
                    payload.extend(i.to_le_bytes());
                }
                payload.extend(3u32.to_le_bytes());
                payload.extend([0u8; 3 * 32]);
            } else {
                payload.extend([0u8; 8]);
            }
        }
        payload.extend(u32::from(block == 0).to_le_bytes());
        if block == 0 {
            let mut draw = [0u8; 0x70];
            for (offset, value) in [(0, 5u32), (4, 12), (8, 5), (0x10, 1), (0x14, 3), (0x44, 1)] {
                draw[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            draw[0x6c] = 1;
            payload.extend(draw);
        }
        payload.extend([0u8; 24]);
    }
    let mut file = Vec::new();
    frame((1, 0), &payload, &mut file);
    let mut cells = (blocks as u32).to_le_bytes().to_vec();
    cells.resize(4 + blocks * 24, 0);
    frame((2, 10), &cells, &mut file);
    file
}

fn dds() -> Vec<u8> {
    let mut file = vec![0u8; 128 + 8];
    file[..4].copy_from_slice(b"DDS ");
    for (offset, value) in [
        (4, 124u32),
        (8, 0x21007),
        (12, 4),
        (16, 4),
        (28, 1),
        (76, 32),
        (80, 4),
        (108, 0x1000),
    ] {
        file[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    file[84..88].copy_from_slice(b"DXT1");
    file
}

// Tiny stored ZIP fixture writer avoids another test dependency or binary fixture.
fn stored_zip(name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    let crc = !crc;
    let mut out = 0x0403_4b50u32.to_le_bytes().to_vec();
    for word in [20u16, 0, 0, 0, 0] {
        out.extend(word.to_le_bytes());
    }
    for word in [crc, bytes.len() as u32, bytes.len() as u32] {
        out.extend(word.to_le_bytes());
    }
    out.extend((name.len() as u16).to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out.extend(name.as_bytes());
    out.extend(bytes);
    let central = out.len() as u32;
    out.extend(0x0201_4b50u32.to_le_bytes());
    for word in [20u16, 20, 0, 0, 0, 0] {
        out.extend(word.to_le_bytes());
    }
    for word in [crc, bytes.len() as u32, bytes.len() as u32] {
        out.extend(word.to_le_bytes());
    }
    for word in [name.len() as u16, 0, 0, 0, 0] {
        out.extend(word.to_le_bytes());
    }
    out.extend([0u8; 8]);
    out.extend(name.as_bytes());
    let central_size = out.len() as u32 - central;
    out.extend(0x0605_4b50u32.to_le_bytes());
    for word in [0u16, 0, 1, 1] {
        out.extend(word.to_le_bytes());
    }
    out.extend(central_size.to_le_bytes());
    out.extend(central.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

#[test]
fn resolves_o_to_n_and_validates_dds_with_case_and_slash_differences() {
    let fixture = Fixture::new();
    let report = asset_probe::inspect(&fixture.0).unwrap();
    assert!(report.complete(), "{report}");
    assert_eq!(report.blocks, 282);
    assert_eq!(report.visibility_cells, 282);
    assert_eq!(report.redirected_to_n(), 1);
    let image = report.textures[0].result.as_ref().unwrap();
    assert_eq!((image.width, image.height, image.mip_count), (4, 4, 1));
    assert!(report.gpu_issues(true, 2048).is_empty());
    assert_eq!(report.gpu_issues(false, 2048).len(), 1);
    assert_eq!(report.gpu_issues(true, 2).len(), 1);
}

#[test]
fn missing_required_archive_is_reported_without_panicking() {
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.0.join("TRACK-20_N_TEXTURES.RBZ")).unwrap();
    let report = asset_probe::inspect(&fixture.0).unwrap();
    assert!(!report.complete());
    assert!(report.textures[0]
        .result
        .as_ref()
        .unwrap_err()
        .contains("missing Maps/"));
}

#[test]
fn rejects_bad_dds_and_corrupt_zip_crc() {
    let fixture = Fixture::new();
    fixture.archive(b"not a DDS");
    assert!(!asset_probe::inspect(&fixture.0).unwrap().complete());
    let mut zip = stored_zip("track-20_n_textures/dry/new/test.dds", &dds());
    let data_at = 30 + "track-20_n_textures/dry/new/test.dds".len();
    zip[data_at + 128] ^= 1; // Keep the old checksum to exercise read-to-EOF validation.
    fixture.write("TRACK-20_N_TEXTURES.RBZ", &zip);
    assert!(!asset_probe::inspect(&fixture.0).unwrap().complete());
}

#[test]
fn rejects_traversal_in_remap_and_wrong_geometry_count() {
    let fixture = Fixture::new();
    fixture.remap("..\\outside\\test.dds");
    let report = asset_probe::inspect(&fixture.0).unwrap();
    assert!(!report.complete());
    assert!(report.textures[0]
        .result
        .as_ref()
        .unwrap_err()
        .contains("invalid archive path"));
    fixture.remap("track-20_n_textures\\dry\\new\\test.dds");
    fixture.write("TRACK-20_o.LBS", &geometry(1));
    assert!(!asset_probe::inspect(&fixture.0).unwrap().complete());
}

#[test]
fn unused_missing_ini_texture_is_distinct_from_required_failure() {
    let fixture = Fixture::new();
    fixture.write("track-20_O.ini", ini(2).as_bytes());
    let report = asset_probe::inspect(&fixture.0).unwrap();
    assert!(report.complete(), "{report}");
    assert!(!report.textures[1].required);
    assert!(report.textures[1].result.is_err());
}

#[test]
fn truncated_lbs_is_a_parse_error() {
    let fixture = Fixture::new();
    fixture.write("TRACK-20_o.LBS", &[8, 0, 0, 0]);
    assert!(asset_probe::inspect(&fixture.0).is_err());
}

#[test]
#[ignore = "requires the owner's private Maps folder; set HARWOOD_MAPS and run explicitly"]
fn real_harwood_installation() {
    let maps = std::env::var_os("HARWOOD_MAPS").expect("set HARWOOD_MAPS to a private Maps folder");
    let report = asset_probe::inspect(&PathBuf::from(maps)).unwrap();
    println!("{report}");
    assert!(report.complete());
    assert_eq!((report.blocks, report.visibility_cells), (282, 282));
    assert_eq!((report.declared_diffuse, report.declared_specular), (39, 5));
    assert!(report.textures.iter().all(|t| t.result.is_ok()));
    assert!(report.redirected_to_n() > 0);
}
