//! Small original 5x7 ASCII overlay. No font files or UI dependencies.
const WIDTH: u32 = 96;
const HEIGHT: u32 = 32;
const MAX_VERTICES: usize = 8192;

pub struct Hud {
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
}
impl Hud {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("benchmark ASCII HUD"),
            source: wgpu::ShaderSource::Wgsl(include_str!("hud.wgsl").into()),
        });
        const ATTRS: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Unorm8x4];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("benchmark HUD alpha overlay"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 20,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &ATTRS,
                })],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("own ASCII bitmap"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(WIDTH),
                rows_per_image: Some(HEIGHT),
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HUD nearest"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HUD font"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bounded HUD vertices"),
            size: (MAX_VERTICES * 20) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if let Some(error) = futures_executor::block_on(scope.pop()) {
            return Err(format!("HUD validation: {error}"));
        }
        Ok(Self {
            pipeline,
            group,
            vertices,
        })
    }
    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        lines: &[String],
    ) -> usize {
        let bytes = vertices(lines, size);
        queue.write_buffer(&self.vertices, 0, &bytes);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("benchmark HUD (included in timing)"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..(bytes.len() / 20) as u32, 0..1);
        bytes.len() / 20
    }
}

pub fn vertices(lines: &[String], size: (u32, u32)) -> Vec<u8> {
    let mut out = Vec::with_capacity(16384);
    // 640 virtual pixels wide; independent of high-DPI drawable pixel dimensions.
    let h = 640.0 * size.1.max(1) as f32 / size.0.max(1) as f32;
    let mut quad = |x: f32, y: f32, w: f32, height: f32, uv: [f32; 4], color: [u8; 4]| {
        for (dx, dy) in [
            (0.0, 0.0),
            (1.0, 0.0),
            (0.0, 1.0),
            (0.0, 1.0),
            (1.0, 0.0),
            (1.0, 1.0),
        ] {
            for value in [
                (x + dx * w) / 320.0 - 1.0,
                1.0 - (y + dy * height) * 2.0 / h,
                uv[0] + dx * (uv[2] - uv[0]),
                uv[1] + dy * (uv[3] - uv[1]),
            ] {
                out.extend(value.to_ne_bytes());
            }
            out.extend(color);
        }
    };
    let white = [
        0.5 / WIDTH as f32,
        0.5 / HEIGHT as f32,
        0.5 / WIDTH as f32,
        0.5 / HEIGHT as f32,
    ];
    quad(
        10.0,
        24.0,
        620.0,
        lines.len().min(12) as f32 * 20.0 + 16.0,
        white,
        [5, 8, 14, 235],
    );
    for (row, line) in lines.iter().take(12).enumerate() {
        for (column, c) in line.to_ascii_uppercase().bytes().take(42).enumerate() {
            if c == b' ' {
                continue;
            }
            let c = if (32..96).contains(&c) { c } else { b'?' };
            let i = u32::from(c - 32);
            let x = (i % 16) * 6;
            let y = (i / 16) * 8;
            quad(
                20.0 + column as f32 * 14.0,
                32.0 + row as f32 * 20.0,
                12.0,
                16.0,
                [
                    x as f32 / WIDTH as f32,
                    y as f32 / HEIGHT as f32,
                    (x + 5) as f32 / WIDTH as f32,
                    (y + 7) as f32 / HEIGHT as f32,
                ],
                [240, 248, 255, 255],
            );
        }
    }
    debug_assert!(out.len() / 20 <= MAX_VERTICES);
    out
}

pub fn atlas() -> Vec<u8> {
    let mut out = vec![0; (WIDTH * HEIGHT) as usize];
    for c in 32..96u8 {
        let id = u32::from(c - 32);
        for (y, bits) in glyph(c).iter().enumerate() {
            for x in 0..5 {
                if bits & (1 << (4 - x)) != 0 {
                    out[((id / 16 * 8 + y as u32) * WIDTH + id % 16 * 6 + x) as usize] = 255;
                }
            }
        }
    }
    out[0] = 255; // White sample for the panel; spaces themselves are not drawn.
    out
}
fn glyph(c: u8) -> [u8; 7] {
    match c {
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 15],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'J' => [7, 2, 2, 2, 2, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [30, 1, 1, 14, 1, 1, 30],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 16, 30, 1, 1, 30],
        b'6' => [14, 16, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 1, 14],
        b'.' => [0, 0, 0, 0, 0, 12, 12],
        b':' => [0, 12, 12, 0, 12, 12, 0],
        b'/' => [1, 2, 2, 4, 8, 8, 16],
        b'-' => [0, 0, 0, 31, 0, 0, 0],
        b'(' => [2, 4, 8, 8, 8, 4, 2],
        b')' => [8, 4, 2, 2, 2, 4, 8],
        b' ' => [0; 7],
        _ => [14, 17, 1, 2, 4, 0, 4],
    }
}
