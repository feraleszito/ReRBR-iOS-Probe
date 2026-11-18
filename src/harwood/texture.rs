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
    pub mips: Vec<Mip>,
}

pub fn upload_plan(bytes: &[u8], max_dimension: u32) -> Result<UploadPlan, String> {
    let image = dds::parse(bytes).map_err(|e| e.to_string())?;
    let is_bc = matches!(
        image.format,
        dds::Format::Dxt1 | dds::Format::Dxt3 | dds::Format::Dxt5 | dds::Format::Bc7
    );
    if !is_bc && image.format != dds::Format::Rgba8 {
        return Err("unsupported DDS format".into());
    }
    if image.width.max(image.height) > max_dimension
        || (is_bc && (!image.width.is_multiple_of(4) || !image.height.is_multiple_of(4)))
    {
        return Err("base DDS size exceeds device limit or is not whole BC blocks".into());
    }
    let mut offset = bytes.len() - image.data.len();
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
        mips,
    })
}
