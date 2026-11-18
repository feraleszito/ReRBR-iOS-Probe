//! Authentic RBR visual pacenote sprite loader from `Documents/RBR/Generic/PC_Generic_<width>.bnk`.
//!
//! Extracts ONLY the user's private game icon bank (`PC_Generic_1280.bnk` / `1024` / `800` / `640`)
//! and decodes the sprite page `.dds` (DXT1/DXT3/DXT5) dynamically at runtime.
//! Never ships copyrighted sprites or substitutes hand-drawn fake icons.

use rerbr_sim::rbr_formats::{bnk, cars, dds, markers};
use std::path::{Path, PathBuf};

/// Primary page-0 curve/straight arrow band (`PC_*_01.dds`) and page-4 hazard/overlay band (`PC_*_05.dds`).
pub const CURVE_SPRITE_PROBE: usize = 0x503;
pub const HAZARD_SPRITE_PROBE: usize = 0x4c3;

#[derive(Clone, Debug, PartialEq)]
pub struct PacenoteHudSettings {
    pub enabled: bool,
    pub scale: f32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub anticipation_s: f32,
    pub debug_sprites: bool,
}

impl Default for PacenoteHudSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            scale: 1.0,
            pos_x: 0.50,
            pos_y: 0.14,
            anticipation_s: 3.5,
            debug_sprites: false,
        }
    }
}

impl PacenoteHudSettings {
    pub fn sanitized(mut self) -> Self {
        self.scale = if self.scale.is_finite() {
            self.scale.clamp(0.5, 2.0)
        } else {
            1.0
        };
        self.pos_x = if self.pos_x.is_finite() {
            self.pos_x.clamp(0.10, 0.90)
        } else {
            0.50
        };
        self.pos_y = if self.pos_y.is_finite() {
            self.pos_y.clamp(0.06, 0.55)
        } else {
            0.14
        };
        self.anticipation_s = if self.anticipation_s.is_finite() {
            self.anticipation_s.clamp(1.5, 8.0)
        } else {
            3.5
        };
        self
    }

    pub fn config_path() -> Option<PathBuf> {
        let root = std::env::var_os("RERBR_PROBE_DOCUMENTS").map(PathBuf::from)?;
        Some(root.join("RBR").join("pacenote_hud.txt"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        Self::parse(&text)
    }

    pub fn save(&self) {
        let Some(path) = Self::config_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, self.serialize());
    }

    pub fn serialize(&self) -> String {
        format!(
            "enabled={}\nscale={:.2}\npos_x={:.2}\npos_y={:.2}\nanticipation_s={:.1}\ndebug_sprites={}\n",
            self.enabled, self.scale, self.pos_x, self.pos_y, self.anticipation_s, self.debug_sprites
        )
    }

    pub fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k.trim() {
                "enabled" => {
                    if let Ok(b) = v.trim().parse() {
                        s.enabled = b;
                    }
                }
                "scale" => {
                    if let Ok(n) = v.trim().parse() {
                        s.scale = n;
                    }
                }
                "pos_x" => {
                    if let Ok(n) = v.trim().parse() {
                        s.pos_x = n;
                    }
                }
                "pos_y" => {
                    if let Ok(n) = v.trim().parse() {
                        s.pos_y = n;
                    }
                }
                "anticipation_s" => {
                    if let Ok(n) = v.trim().parse() {
                        s.anticipation_s = n;
                    }
                }
                "debug_sprites" => {
                    if let Ok(b) = v.trim().parse() {
                        s.debug_sprites = b;
                    }
                }
                _ => {}
            }
        }
        s.sanitized()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteQuad {
    pub uv: [f32; 4],
    pub aspect: f32,
    pub page: u32,
}

#[derive(Clone, Debug)]
struct AtlasPageSlice {
    page_index: u32,
    page_name: String,
    width: u32,
    height: u32,
    y_offset: u32,
}

#[derive(Clone, Debug)]
pub struct PacenoteIconAtlas {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
    pub bank_name: String,
    pub page_name: String,
    pages: Vec<AtlasPageSlice>,
    pub sprites: Vec<bnk::Sprite>,
}

impl PacenoteIconAtlas {
    pub fn load_from_documents(documents: &Path) -> Result<Self, String> {
        let rbr_root = documents.join("RBR");
        Self::load_from_rbr_root(&rbr_root)
    }

    pub fn load_from_rbr_root(rbr_root: &Path) -> Result<Self, String> {
        for width in [1280u32, 1024, 800, 640] {
            let bnk_rel = format!("Generic/PC_Generic_{width}.bnk");
            let Some(bnk_path) = cars::resolve(rbr_root, &bnk_rel) else {
                continue;
            };
            let bnk_bytes = std::fs::read(&bnk_path).map_err(|e| format!("{bnk_rel}: {e}"))?;
            let bank = bnk::parse(&bnk_bytes).map_err(|e| e.to_string())?;
            let curve_page = bank
                .sprites
                .get(CURVE_SPRITE_PROBE)
                .ok_or_else(|| format!("{bnk_rel}: missing curve sprite {CURVE_SPRITE_PROBE:#x}"))?
                .page;
            let hazard_page = bank
                .sprites
                .get(HAZARD_SPRITE_PROBE)
                .ok_or_else(|| {
                    format!("{bnk_rel}: missing hazard sprite {HAZARD_SPRITE_PROBE:#x}")
                })?
                .page;

            let mut target_pages = vec![curve_page];
            if hazard_page != curve_page {
                target_pages.push(hazard_page);
            }
            let mut slices = Vec::with_capacity(target_pages.len());
            let mut decoded_pages = Vec::with_capacity(target_pages.len());
            let mut total_w = 0u32;
            let mut total_h = 0u32;

            for page_index in target_pages {
                let page_name = bank
                    .pages
                    .get(page_index as usize)
                    .cloned()
                    .ok_or_else(|| format!("{bnk_rel}: bad page {page_index}"))?;
                let dds_rel = format!("Generic/{page_name}");
                let dds_path = cars::resolve(rbr_root, &dds_rel)
                    .ok_or_else(|| format!("missing {dds_rel}"))?;
                let dds_bytes = std::fs::read(&dds_path).map_err(|e| format!("{dds_rel}: {e}"))?;
                let parsed = dds::parse(&dds_bytes).map_err(|e| format!("{dds_rel}: {e}"))?;
                let pixels = decode_dds_top_rgba8(&parsed)?;
                slices.push(AtlasPageSlice {
                    page_index,
                    page_name,
                    width: parsed.width,
                    height: parsed.height,
                    y_offset: total_h,
                });
                total_w = total_w.max(parsed.width);
                total_h += parsed.height;
                decoded_pages.push((parsed.width, parsed.height, pixels));
            }

            let mut rgba8 = vec![0u8; total_w as usize * total_h as usize * 4];
            for (slice, (pw, ph, pixels)) in slices.iter().zip(decoded_pages) {
                for row in 0..ph as usize {
                    let src_start = row * pw as usize * 4;
                    let src_end = src_start + pw as usize * 4;
                    let dst_start = ((slice.y_offset as usize + row) * total_w as usize) * 4;
                    let dst_end = dst_start + pw as usize * 4;
                    rgba8[dst_start..dst_end].copy_from_slice(&pixels[src_start..src_end]);
                }
            }

            let page_name = slices
                .iter()
                .map(|s| s.page_name.as_str())
                .collect::<Vec<_>>()
                .join("+");
            return Ok(Self {
                width: total_w,
                height: total_h,
                rgba8,
                bank_name: bnk_rel,
                page_name,
                pages: slices,
                sprites: bank.sprites,
            });
        }
        Err("FALTA Generic/PC_Generic_*.bnk EN DOCUMENTS/RBR".into())
    }

    pub fn sprite_uv(&self, sprite_index: usize) -> Option<SpriteQuad> {
        let s = self.sprites.get(sprite_index)?;
        let slice = self.pages.iter().find(|p| p.page_index == s.page)?;
        let rect = s.rect();
        let uw = (rect[2] - rect[0]).abs();
        let vh = (rect[3] - rect[1]).abs();
        if uw <= 0.0 || vh <= 0.0 {
            return None;
        }
        let u0 = rect[0] * (slice.width as f32 / self.width.max(1) as f32);
        let u1 = rect[2] * (slice.width as f32 / self.width.max(1) as f32);
        let v0 =
            (slice.y_offset as f32 + rect[1] * slice.height as f32) / self.height.max(1) as f32;
        let v1 =
            (slice.y_offset as f32 + rect[3] * slice.height as f32) / self.height.max(1) as f32;
        let px_w = uw * slice.width as f32;
        let px_h = vh * slice.height as f32;
        let aspect = px_w / px_h.max(1.0);
        Some(SpriteQuad {
            uv: [u0, v0, u1, v1],
            aspect,
            page: s.page,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacenoteSpriteRef {
    pub sprite: usize,
    pub mirror_x: bool,
}

impl PacenoteSpriteRef {
    const fn direct(sprite: usize) -> Self {
        Self {
            sprite,
            mirror_x: false,
        }
    }
    const fn mirrored(sprite: usize) -> Self {
        Self {
            sprite,
            mirror_x: true,
        }
    }
}

/// Maps an authentic RBR pacenote `(tag, slot_word_id, flag_word_id)` to `PC_Generic_*.bnk` sprite indices.
///
/// Curve arrows come from page 0 (`0x503..=0x50a`, all natively pointing right -> mirrored for left).
/// Hazard / modifier overlays come from page 4 (`0x4c6`, `0x4cd`, `0x4d4`, `0x4d5`, `0x4df`) and page 0 (`0x4ff`, `0x502`).
/// Weather sprites (`0x545..=0x54b`) are explicitly excluded.
pub fn sprite_indices_for_callout(
    tag: u32,
    slot_word_id: Option<u32>,
    flag_word_id: Option<u32>,
) -> (Option<PacenoteSpriteRef>, Option<PacenoteSpriteRef>) {
    use PacenoteSpriteRef as S;
    let main = match tag {
        markers::TAG_START | markers::TAG_FINISH | markers::TAG_SPLIT | markers::TAG_END => None,
        0 => Some(S::mirrored(0x508)),     // HAIRPIN LEFT
        1 => Some(S::mirrored(0x507)),     // 90 LEFT
        2 => Some(S::mirrored(0x506)),     // K LEFT
        3 => Some(S::mirrored(0x505)),     // MED LEFT
        4 => Some(S::mirrored(0x504)),     // FAST LEFT
        5 => Some(S::mirrored(0x503)),     // EASY LEFT
        6 => Some(S::direct(0x503)),       // EASY RIGHT
        7 => Some(S::direct(0x504)),       // FAST RIGHT
        8 => Some(S::direct(0x505)),       // MED RIGHT
        9 => Some(S::direct(0x506)),       // K RIGHT
        10 => Some(S::direct(0x507)),      // 90 RIGHT
        11 => Some(S::direct(0x508)),      // HAIRPIN RIGHT
        12 => Some(S::direct(0x50a)),      // TWISTY
        13..=16 => Some(S::direct(0x509)), // FLAT / STRAIGHT
        17 => Some(S::direct(0x4cd)),      // CREST
        18 => Some(S::direct(0x4d5)),      // JUMP
        19 => Some(S::direct(0x4d4)),      // BUMP / DIP
        20 | 32 => Some(S::direct(0x4df)), // CAUTION (!)
        _ => slot_word_id.and_then(|id| match id {
            0x1d => Some(S::mirrored(0x508)),      // Hairpin Left
            0x1e => Some(S::direct(0x508)),        // Hairpin Right
            0x21 => Some(S::mirrored(0x506)),      // K Left
            0x22 => Some(S::direct(0x506)),        // K Right
            0x25 => Some(S::mirrored(0x505)),      // Med Left
            0x26 => Some(S::direct(0x505)),        // Med Right
            0x19 => Some(S::mirrored(0x504)),      // Fast Left
            0x1a => Some(S::direct(0x504)),        // Fast Right
            0x17 => Some(S::mirrored(0x503)),      // Easy Left
            0x18 => Some(S::direct(0x503)),        // Easy Right
            0x0e | 0x0f => Some(S::direct(0x4df)), // Caution
            0x1c => Some(S::direct(0x4e0)),        // Water / Splash
            0x20 => Some(S::direct(0x4d5)),        // Jump
            0x28 => Some(S::direct(0x4cd)),        // Crest
            _ => None,
        }),
    };
    let modifier = flag_word_id.and_then(|w| match w {
        0x12 => Some(S::direct(0x4c6)), // DON'T CUT (red slash)
        0x27 => Some(S::direct(0x4ff)), // NARROWS
        0x2f => Some(S::direct(0x502)), // WIDEOUT
        _ => None,
    });
    (main, modifier)
}

fn decode_rgb565(c: u16) -> [u8; 3] {
    let r = ((c >> 11) & 0x1f) as u8;
    let g = ((c >> 5) & 0x3f) as u8;
    let b = (c & 0x1f) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    ]
}

fn lerp_rgb(c0: [u8; 3], c1: [u8; 3], w0: u16, w1: u16, div: u16) -> [u8; 3] {
    [
        ((u16::from(c0[0]) * w0 + u16::from(c1[0]) * w1) / div) as u8,
        ((u16::from(c0[1]) * w0 + u16::from(c1[1]) * w1) / div) as u8,
        ((u16::from(c0[2]) * w0 + u16::from(c1[2]) * w1) / div) as u8,
    ]
}

/// Decodes the top mip level of a DXT1, DXT3, or DXT5 `.dds` image into tightly packed RGBA8 pixels.
pub fn decode_dds_top_rgba8(img: &dds::Dds<'_>) -> Result<Vec<u8>, String> {
    let mip = img.data;
    let w = img.width as usize;
    let h = img.height as usize;
    if w == 0 || h == 0 {
        return Err("DDS has zero dimensions".into());
    }
    let mut out = vec![0u8; w * h * 4];
    let bx_count = w.div_ceil(4);
    let by_count = h.div_ceil(4);

    match img.format {
        dds::Format::Dxt1 => {
            if mip.len() < bx_count * by_count * 8 {
                return Err("truncated DXT1 mip".into());
            }
            for by in 0..by_count {
                for bx in 0..bx_count {
                    let blk = &mip[(by * bx_count + bx) * 8..];
                    let c0v = u16::from_le_bytes([blk[0], blk[1]]);
                    let c1v = u16::from_le_bytes([blk[2], blk[3]]);
                    let idx = u32::from_le_bytes([blk[4], blk[5], blk[6], blk[7]]);
                    let c0 = decode_rgb565(c0v);
                    let c1 = decode_rgb565(c1v);
                    let palette = if c0v > c1v {
                        [
                            [c0[0], c0[1], c0[2], 255],
                            [c1[0], c1[1], c1[2], 255],
                            {
                                let c = lerp_rgb(c0, c1, 2, 1, 3);
                                [c[0], c[1], c[2], 255]
                            },
                            {
                                let c = lerp_rgb(c0, c1, 1, 2, 3);
                                [c[0], c[1], c[2], 255]
                            },
                        ]
                    } else {
                        [
                            [c0[0], c0[1], c0[2], 255],
                            [c1[0], c1[1], c1[2], 255],
                            {
                                let c = lerp_rgb(c0, c1, 1, 1, 2);
                                [c[0], c[1], c[2], 255]
                            },
                            [0, 0, 0, 0],
                        ]
                    };
                    for py in 0..4 {
                        for px in 0..4 {
                            let x = bx * 4 + px;
                            let y = by * 4 + py;
                            if x < w && y < h {
                                let code = ((idx >> ((py * 4 + px) * 2)) & 3) as usize;
                                let dst = (y * w + x) * 4;
                                out[dst..dst + 4].copy_from_slice(&palette[code]);
                            }
                        }
                    }
                }
            }
            Ok(out)
        }
        dds::Format::Dxt3 | dds::Format::Dxt5 => {
            if mip.len() < bx_count * by_count * 16 {
                return Err("truncated DXT3/5 mip".into());
            }
            let is_dxt3 = img.format == dds::Format::Dxt3;
            for by in 0..by_count {
                for bx in 0..bx_count {
                    let blk = &mip[(by * bx_count + bx) * 16..];
                    let alpha_bits = u64::from_le_bytes(blk[0..8].try_into().unwrap());
                    let a0 = blk[0];
                    let a1 = blk[1];
                    let a_payload = alpha_bits >> 16;

                    let c0 = decode_rgb565(u16::from_le_bytes([blk[8], blk[9]]));
                    let c1 = decode_rgb565(u16::from_le_bytes([blk[10], blk[11]]));
                    let idx = u32::from_le_bytes([blk[12], blk[13], blk[14], blk[15]]);
                    let rgb = [c0, c1, lerp_rgb(c0, c1, 2, 1, 3), lerp_rgb(c0, c1, 1, 2, 3)];
                    for py in 0..4 {
                        for px in 0..4 {
                            let x = bx * 4 + px;
                            let y = by * 4 + py;
                            if x < w && y < h {
                                let p = py * 4 + px;
                                let code = ((idx >> (p * 2)) & 3) as usize;
                                let alpha = if is_dxt3 {
                                    let n = ((alpha_bits >> (p * 4)) & 0xf) as u8;
                                    (n << 4) | n
                                } else {
                                    let ac = ((a_payload >> (p * 3)) & 7) as u8;
                                    dxt5_alpha(a0, a1, ac)
                                };
                                let dst = (y * w + x) * 4;
                                out[dst] = rgb[code][0];
                                out[dst + 1] = rgb[code][1];
                                out[dst + 2] = rgb[code][2];
                                out[dst + 3] = alpha;
                            }
                        }
                    }
                }
            }
            Ok(out)
        }
        other => Err(format!("unsupported DDS format for pacenotes: {other:?}")),
    }
}

fn dxt5_alpha(a0: u8, a1: u8, code: u8) -> u8 {
    match code {
        0 => a0,
        1 => a1,
        _ if a0 > a1 => {
            ((u16::from(a0) * u16::from(8 - code) + u16::from(a1) * u16::from(code - 1)) / 7) as u8
        }
        6 => 0,
        7 => 255,
        _ => {
            ((u16::from(a0) * u16::from(6 - code) + u16::from(a1) * u16::from(code - 1)) / 5) as u8
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip_and_clamp() {
        let cfg = PacenoteHudSettings {
            enabled: false,
            scale: 1.5,
            pos_x: 0.25,
            pos_y: 0.20,
            anticipation_s: 5.0,
            debug_sprites: true,
        };
        assert_eq!(PacenoteHudSettings::parse(&cfg.serialize()), cfg);
        let bad = PacenoteHudSettings::parse("scale=99\npos_x=-5\nanticipation_s=NaN");
        assert_eq!(bad.scale, 2.0);
        assert_eq!(bad.pos_x, 0.10);
        assert_eq!(bad.anticipation_s, 3.5);
        assert!(!bad.debug_sprites);
    }

    #[test]
    fn never_emits_weather_sprites_for_any_pacenote_tag_or_word() {
        let weather_range = 0x545usize..=0x54b;
        for tag in 0u32..=40 {
            for wid in [
                None,
                Some(0x0e),
                Some(0x17),
                Some(0x18),
                Some(0x19),
                Some(0x1a),
                Some(0x1c),
                Some(0x1d),
                Some(0x1e),
                Some(0x20),
                Some(0x21),
                Some(0x22),
                Some(0x25),
                Some(0x26),
                Some(0x28),
            ] {
                let (m, f) = sprite_indices_for_callout(tag, wid, Some(0x12));
                if let Some(r) = m {
                    assert!(
                        !weather_range.contains(&r.sprite),
                        "tag={tag} wid={wid:?} mapped to weather sprite {:#x}",
                        r.sprite
                    );
                }
                if let Some(r) = f {
                    assert!(!weather_range.contains(&r.sprite));
                }
            }
        }
        // Left vs right symmetry verification: left curves mirror right curves on X.
        for (left, right) in [(0, 11), (1, 10), (2, 9), (3, 8), (4, 7), (5, 6)] {
            let (l, _) = sprite_indices_for_callout(left, None, None);
            let (r, _) = sprite_indices_for_callout(right, None, None);
            let (l, r) = (l.unwrap(), r.unwrap());
            assert_eq!(l.sprite, r.sprite);
            assert!(l.mirror_x && !r.mirror_x);
        }
    }

    #[test]
    fn private_bnk_pacenote_atlas_decodes_and_maps_silhouettes() {
        let rbr_root = Path::new(r"C:\Richard Burns Rally");
        if !rbr_root.join("Generic/PC_Generic_1280.bnk").is_file() {
            return;
        }
        let atlas = PacenoteIconAtlas::load_from_rbr_root(rbr_root).unwrap();
        assert_eq!(atlas.width, 512);
        assert_eq!(atlas.height, 1024);
        assert_eq!(atlas.rgba8.len(), 512 * 1024 * 4);
        // Curve sprites live on page 0 (v in 0..0.5)
        let q90 = atlas.sprite_uv(0x507).unwrap();
        assert_eq!(q90.page, 0);
        assert!(q90.uv[1] < 0.5 && q90.uv[3] <= 0.5);
        // Hazard / modifier sprites live on page 4 (v in 0.5..1.0)
        let qdcut = atlas.sprite_uv(0x4c6).unwrap();
        assert_eq!(qdcut.page, 4);
        assert!(qdcut.uv[1] >= 0.5 && qdcut.uv[3] <= 1.0);
        let qjump = atlas.sprite_uv(0x4d5).unwrap();
        assert_eq!(qjump.page, 4);
        let (main, modif) = sprite_indices_for_callout(1, None, Some(0x12));
        assert_eq!(main, Some(PacenoteSpriteRef::mirrored(0x507)));
        assert_eq!(modif, Some(PacenoteSpriteRef::direct(0x4c6)));
    }
}
