//! BC upload layout, adapted from reRBR renderer/textures.rs::dds_texture.
use rerbr_asset_formats::dds;

#[derive(Debug, Clone, Copy)]
pub struct Mip {
    pub width: u32,
    pub height: u32,
    pub copy_width: u32,
    pub copy_height: u32,
    pub bytes_per_row: u32,
    pub block_rows: u32,
    pub offset: usize,
    pub len: usize,
}

#[derive(Debug)]
pub struct UploadPlan {
    pub format: dds::Format,
    pub color_space: dds::ColorSpace,
    pub mips: Vec<Mip>,
}

/// A fixed-size band keeps CPU scratch independent of the mip height.
pub const DECODE_BAND_ROWS: u32 = 32;

impl UploadPlan {
    pub fn needs_decode(&self, bc_enabled: bool) -> bool {
        self.format.block_edge() == 4
            && (!bc_enabled
                || !self.mips[0].width.is_multiple_of(4)
                || !self.mips[0].height.is_multiple_of(4))
    }

    pub fn gpu_format(&self, bc_enabled: bool) -> wgpu::TextureFormat {
        use wgpu::TextureFormat::*;
        let format = if self.needs_decode(bc_enabled) {
            dds::Format::Rgba8
        } else {
            self.format
        };
        match (format, self.color_space == dds::ColorSpace::Srgb) {
            (dds::Format::Rgba8, false) => Rgba8Unorm,
            (dds::Format::Rgba8, true) => Rgba8UnormSrgb,
            (dds::Format::Dxt1, false) => Bc1RgbaUnorm,
            (dds::Format::Dxt1, true) => Bc1RgbaUnormSrgb,
            (dds::Format::Dxt3, false) => Bc2RgbaUnorm,
            (dds::Format::Dxt3, true) => Bc2RgbaUnormSrgb,
            (dds::Format::Dxt5, false) => Bc3RgbaUnorm,
            (dds::Format::Dxt5, true) => Bc3RgbaUnormSrgb,
            (dds::Format::Bc7, false) => Bc7RgbaUnorm,
            (dds::Format::Bc7, true) => Bc7RgbaUnormSrgb,
        }
    }

    /// Texture bytes before GPU-specific allocation and upload overhead.
    pub fn resident_bytes(&self, bc_enabled: bool) -> usize {
        if self.needs_decode(bc_enabled) {
            self.mips
                .iter()
                .map(|m| m.width as usize * m.height as usize * 4)
                .sum()
        } else {
            self.mips.iter().map(|m| m.len).sum()
        }
    }
}

pub fn upload_plan(bytes: &[u8], max_dimension: u32) -> Result<UploadPlan, String> {
    let image = dds::parse(bytes).map_err(|e| e.to_string())?;
    let is_bc = image.format.block_edge() == 4;
    if image.width.max(image.height) > max_dimension {
        return Err(format!(
            "DDS size {}x{} exceeds device limit {max_dimension}",
            image.width, image.height
        ));
    }
    let mut offset = image.data_offset;
    let mips = image
        .levels()
        .iter()
        .map(|level| {
            let (copy_width, copy_height, bytes_per_row, block_rows) = if is_bc {
                let blocks_x = level.width.div_ceil(4);
                let blocks_y = level.height.div_ceil(4);
                (
                    blocks_x * 4,
                    blocks_y * 4,
                    blocks_x * image.format.block_len() as u32,
                    blocks_y,
                )
            } else {
                (level.width, level.height, level.width * 4, level.height)
            };
            let mip = Mip {
                width: level.width,
                height: level.height,
                copy_width,
                copy_height,
                bytes_per_row,
                block_rows,
                offset,
                len: level.data.len(),
            };
            offset += level.data.len();
            mip
        })
        .collect();
    Ok(UploadPlan {
        format: image.format,
        color_space: image.color_space,
        mips,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_probe::{TextureData, TextureInfo};

    // Several rows, a partial block, sub-4x4 mip levels and trailing metadata.
    fn fixture(width: u32, height: u32, dxgi: u32) -> Vec<u8> {
        let mut bytes = vec![0; 148];
        bytes[..4].copy_from_slice(b"DDS ");
        bytes[84..88].copy_from_slice(b"DX10");
        for (offset, value) in [
            (4, 124),
            (12, height),
            (16, width),
            (28, 4),
            (76, 32),
            (80, 4),
            (128, dxgi),
            (132, 3),
            (140, 1),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        let mut block = [0x88; 16]; // BC2 alpha = 136.
        block[8..10].copy_from_slice(&0xf800u16.to_le_bytes());
        block[12..16].fill(0); // All color indices use the red endpoint.
        for level in 0..4 {
            let blocks = (width >> level).max(1).div_ceil(4) * (height >> level).max(1).div_ceil(4);
            for _ in 0..blocks {
                bytes.extend_from_slice(&block);
            }
        }
        bytes.extend_from_slice(b"unused tail");
        bytes
    }

    #[test]
    fn dds_layout_preserves_mips_alpha_and_dxgi_color_space() {
        let bytes = fixture(5, 35, 75);
        let plan = upload_plan(&bytes, 64).unwrap();
        assert_eq!(plan.gpu_format(false), wgpu::TextureFormat::Rgba8UnormSrgb);
        assert_eq!(plan.gpu_format(true), wgpu::TextureFormat::Rgba8UnormSrgb);
        assert_eq!(plan.mips[0].offset, 148);
        assert_eq!(
            plan.mips
                .iter()
                .map(|m| (m.width, m.height))
                .collect::<Vec<_>>(),
            [(5, 35), (2, 17), (1, 8), (1, 4)]
        );
        assert_eq!(plan.resident_bytes(false), (5 * 35 + 2 * 17 + 8 + 4) * 4);
        let dds = dds::parse(&bytes).unwrap();
        for mip in dds.levels() {
            for y in (0..mip.height).step_by(DECODE_BAND_ROWS as usize) {
                let rows = (mip.height - y).min(DECODE_BAND_ROWS);
                let mut out = vec![0; (mip.width * rows * 4) as usize];
                dds::decode_rgba_rows(dds.format, mip, y, rows, &mut out).unwrap();
                assert!(out.chunks_exact(4).all(|p| p == [255, 0, 0, 136]));
            }
        }
        assert!(upload_plan(&bytes, 32).is_err());
        let linear = upload_plan(&fixture(12, 36, 74), 64).unwrap();
        assert_eq!(linear.color_space, dds::ColorSpace::Linear);
        assert_eq!(linear.gpu_format(false), wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(linear.gpu_format(true), wgpu::TextureFormat::Bc2RgbaUnorm);
    }

    #[test]
    fn bc_decoder_keeps_bc1_cutouts_and_bc3_and_bc7_alpha() {
        let mut bc1 = [0u8; 8];
        bc1[4..].fill(0xff); // BC1 endpoint order selects the transparent index.
        let mut bc3 = [0u8; 16];
        bc3[0] = 127; // Alpha index zero selects the first endpoint.
        bc3[8..10].copy_from_slice(&0x07e0u16.to_le_bytes());

        // BC7 mode 6, matching endpoints, all interpolation indices zero.
        let mut bc7 = 64u128;
        let mut bit = 7;
        for endpoint in [127u128, 127, 0, 0, 0, 0, 127, 127] {
            bc7 |= endpoint << bit;
            bit += 7;
        }
        bc7 |= 3 << bit;
        let bc7 = bc7.to_le_bytes();
        for (format, data, expected) in [
            (dds::Format::Dxt1, bc1.as_slice(), [0, 0, 0, 0]),
            (dds::Format::Dxt5, bc3.as_slice(), [0, 255, 0, 127]),
            (dds::Format::Bc7, bc7.as_slice(), [255, 1, 1, 255]),
        ] {
            let mip = dds::Level {
                width: 4,
                height: 4,
                data,
            };
            let mut out = [0; 64];
            dds::decode_rgba_rows(format, mip, 0, 4, &mut out).unwrap();
            assert!(
                out.chunks_exact(4).all(|pixel| pixel == expected),
                "{format:?}: {:?}",
                &out[..4]
            );
        }
    }

    #[test]
    fn wgpu_validates_dds_uploads_with_and_without_bc() {
        for features in [
            wgpu::Features::empty(),
            wgpu::Features::TEXTURE_COMPRESSION_BC,
        ] {
            let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor {
                required_features: features,
                ..Default::default()
            });
            let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let data = TextureData {
                info: TextureInfo {
                    request: "synthetic DDS".into(),
                    resolved: "synthetic DDS".into(),
                    width: 12,
                    height: 36,
                    mip_count: 4,
                    format: dds::Format::Dxt3,
                },
                bytes: fixture(12, 36, 75),
            };
            let (_, linear) = super::super::gpu::upload(&device, &queue, &data).unwrap();
            assert!(linear);
            queue.submit([]);
            let error = futures_executor::block_on(scope.pop());
            assert!(error.is_none(), "{error:?}");
        }
    }
}
