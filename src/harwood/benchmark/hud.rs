//! Modern anti-aliased sans-serif HUD overlay (Ubuntu-Light 512x512 R8 atlas).
#[path = "font_metrics.rs"]
mod font_metrics;

const WIDTH: u32 = font_metrics::ATLAS_WIDTH;
const HEIGHT: u32 = font_metrics::ATLAS_HEIGHT;
const MAX_VERTICES: usize = 32768;

fn find_glyph(ch: char) -> &'static font_metrics::GlyphMetric {
    let code = ch as u32;
    font_metrics::GLYPHS
        .iter()
        .find(|g| g.code == code)
        .unwrap_or_else(|| {
            font_metrics::GLYPHS
                .iter()
                .find(|g| g.code == b'?' as u32)
                .unwrap_or(&font_metrics::GLYPHS[0])
        })
}

/// Virtual-pixel overlay builder using the licensed proportional font atlas.
/// Coordinates are 640 units wide, origin top-left, like `vertices`.
#[cfg(feature = "physics-drive")]
pub struct Canvas {
    size: (u32, u32),
    height: f32,
    ndc_scale: [f32; 2],
    ndc_origin: [f32; 2],
    out: Vec<u8>,
}
#[cfg(feature = "physics-drive")]
impl Canvas {
    pub fn new(size: (u32, u32)) -> Self {
        Self::in_viewport(size, [0.0, 0.0, 1.0, 1.0])
    }
    /// The viewport is normalized to the drawable, with a top-left origin.
    pub fn in_viewport(size: (u32, u32), [x, y, w, h]: [f32; 4]) -> Self {
        let height = 640.0 * (h * size.1.max(1) as f32) / (w * size.0.max(1) as f32);
        Self {
            size,
            height,
            ndc_scale: [w / 320.0, -2.0 * h / height],
            ndc_origin: [2.0 * x - 1.0, 1.0 - 2.0 * y],
            out: Vec::with_capacity(32768),
        }
    }
    fn ndc(&self, x: f32, y: f32) -> [f32; 2] {
        [
            self.ndc_origin[0] + x * self.ndc_scale[0],
            self.ndc_origin[1] + y * self.ndc_scale[1],
        ]
    }
    pub fn height(&self) -> f32 {
        self.height
    }
    pub fn size(&self) -> (u32, u32) {
        self.size
    }
    pub fn quad(&mut self, x: f32, y: f32, w: f32, h: f32, uv: [f32; 4], color: [u8; 4]) {
        for (dx, dy) in [
            (0.0, 0.0),
            (1.0, 0.0),
            (0.0, 1.0),
            (0.0, 1.0),
            (1.0, 0.0),
            (1.0, 1.0),
        ] {
            let [nx, ny] = self.ndc(x + dx * w, y + dy * h);
            for value in [
                nx,
                ny,
                uv[0] + dx * (uv[2] - uv[0]),
                uv[1] + dy * (uv[3] - uv[1]),
            ] {
                self.out.extend(value.to_ne_bytes());
            }
            self.out.extend(color);
        }
    }
    /// Solid rectangle; samples the atlas's reserved white texel.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        self.quad(
            x,
            y,
            w,
            h,
            [
                0.5 / WIDTH as f32,
                0.5 / HEIGHT as f32,
                0.5 / WIDTH as f32,
                0.5 / HEIGHT as f32,
            ],
            color,
        );
    }
    /// Measures proportional text width in virtual pixels.
    pub fn measure_width(text: &str, scale: f32) -> f32 {
        let k = scale.max(0.1) * 0.62;
        text.chars().map(|ch| find_glyph(ch).advance * k).sum()
    }
    /// IEC power symbol, drawn independently of the HUD font atlas.
    pub fn power_symbol(&mut self, cx: f32, cy: f32, radius: f32, color: [u8; 4]) {
        let white = [
            1.5 / WIDTH as f32,
            1.5 / HEIGHT as f32,
            1.5 / WIDTH as f32,
            1.5 / HEIGHT as f32,
        ];
        let start = -std::f32::consts::FRAC_PI_2 + 0.60;
        let step = (std::f32::consts::TAU - 1.20) / 16.0;
        for i in 0..16 {
            let a = start + i as f32 * step;
            self.ring_segment(cx, cy, radius, 1.3, a, a + step, white, color);
        }
        self.rect(cx - 0.65, cy - radius - 1.0, 1.3, radius, color);
    }
    /// Draws proportional anti-aliased sans-serif text (supporting accents and symbols) and returns the advance.
    pub fn text(&mut self, x: f32, y: f32, text: &str, scale: f32, color: [u8; 4]) -> f32 {
        let k = scale.max(0.1) * 0.62;
        let mut cursor = x;
        for ch in text.chars() {
            let g = find_glyph(ch);
            if ch != ' ' {
                let u0 = g.x as f32 / WIDTH as f32;
                let v0 = g.y as f32 / HEIGHT as f32;
                let u1 = (g.x + g.w) as f32 / WIDTH as f32;
                let v1 = (g.y + g.h) as f32 / HEIGHT as f32;
                self.quad(
                    cursor,
                    y,
                    g.w as f32 * k,
                    g.h as f32 * k,
                    [u0, v0, u1, v1],
                    color,
                );
            }
            cursor += g.advance * k;
        }
        cursor - x
    }
    /// Draws a smooth circular ring spinner (for the animated loading screen).
    pub fn spinner(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        thickness: f32,
        phase: f32,
        color: [u8; 4],
    ) {
        let segments = 24usize;
        let arc_segments = 8usize;
        let white = [
            1.5 / WIDTH as f32,
            1.5 / HEIGHT as f32,
            1.5 / WIDTH as f32,
            1.5 / HEIGHT as f32,
        ];
        // Subtle dark track ring
        for i in 0..segments {
            let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
            self.ring_segment(cx, cy, radius, thickness, a0, a1, white, [42, 50, 64, 180]);
        }
        // Rotating bright arc
        for i in 0..arc_segments {
            let frac = (i + 1) as f32 / arc_segments as f32;
            let a0 = phase + (i as f32 / segments as f32) * std::f32::consts::TAU;
            let a1 = phase + ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
            let c = [color[0], color[1], color[2], (color[3] as f32 * frac) as u8];
            self.ring_segment(cx, cy, radius, thickness, a0, a1, white, c);
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn ring_segment(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        thickness: f32,
        a0: f32,
        a1: f32,
        uv: [f32; 4],
        color: [u8; 4],
    ) {
        let r_in = (radius - thickness * 0.5).max(1.0);
        let r_out = radius + thickness * 0.5;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        let p00 = (cx + c0 * r_in, cy + s0 * r_in);
        let p01 = (cx + c0 * r_out, cy + s0 * r_out);
        let p10 = (cx + c1 * r_in, cy + s1 * r_in);
        let p11 = (cx + c1 * r_out, cy + s1 * r_out);
        for (px, py) in [p00, p01, p10, p10, p01, p11] {
            let [nx, ny] = self.ndc(px, py);
            for value in [nx, ny, uv[0], uv[1]] {
                self.out.extend(value.to_ne_bytes());
            }
            self.out.extend(color);
        }
    }
    pub fn finish(self) -> Vec<u8> {
        debug_assert!(self.out.len() / 20 <= MAX_VERTICES);
        self.out
    }
}
#[cfg(all(test, feature = "physics-drive"))]
pub const MAX_CANVAS_VERTICES: usize = MAX_VERTICES;

pub struct Hud {
    pipeline: wgpu::RenderPipeline,
    rgba_pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    sprite_group: Option<wgpu::BindGroup>,
    vertices: wgpu::Buffer,
    sprite_vertices: wgpu::Buffer,
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
        let rgba_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HUD RGBA sprite overlay"),
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
                entry_point: Some("fs_rgba"),
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
            label: Some("Ubuntu-Light sans-serif atlas"),
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
            font_metrics::ATLAS_BYTES,
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
            label: Some("HUD linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
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
        let sprite_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bounded HUD sprite vertices"),
            size: 16384,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if let Some(error) = futures_executor::block_on(scope.pop()) {
            return Err(format!("HUD validation: {error}"));
        }
        Ok(Self {
            pipeline,
            rgba_pipeline,
            group,
            sprite_group: None,
            vertices,
            sprite_vertices,
        })
    }

    #[cfg(feature = "physics-drive")]
    pub fn upload_rgba_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        rgba8: &[u8],
    ) {
        let limit = device.limits().max_texture_dimension_2d;
        let size = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4));
        if width == 0 || height == 0 || width.max(height) > limit || size != Some(rgba8.len()) {
            return;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("RBR PC_Generic pacenote sprite sheet"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
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
            rgba8,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HUD sprite linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        self.sprite_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HUD RGBA sprites"),
            layout: &self.rgba_pipeline.get_bind_group_layout(0),
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
        }));
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
        self.draw_bytes(queue, encoder, target, &bytes)
    }
    pub fn draw_bytes(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        bytes: &[u8],
    ) -> usize {
        self.draw_bytes_and_sprites(queue, encoder, target, bytes, &[])
    }
    pub fn draw_bytes_and_sprites(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        bytes: &[u8],
        sprite_bytes: &[u8],
    ) -> usize {
        assert!(bytes.len() / 20 <= MAX_VERTICES);
        queue.write_buffer(&self.vertices, 0, bytes);
        let draw_sprites =
            !sprite_bytes.is_empty() && sprite_bytes.len() <= 16384 && self.sprite_group.is_some();
        if draw_sprites {
            queue.write_buffer(&self.sprite_vertices, 0, sprite_bytes);
        }
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
        if draw_sprites {
            if let Some(group) = &self.sprite_group {
                pass.set_pipeline(&self.rgba_pipeline);
                pass.set_bind_group(0, group, &[]);
                pass.set_vertex_buffer(0, self.sprite_vertices.slice(..));
                pass.draw(0..(sprite_bytes.len() / 20) as u32, 0..1);
            }
        }
        (bytes.len() + sprite_bytes.len()) / 20
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
        let mut cursor = 20.0;
        let scale = 0.55;
        for ch in line.chars().take(80) {
            let g = find_glyph(ch);
            if cursor + g.w as f32 * scale > 620.0 {
                break;
            }
            if ch != ' ' {
                quad(
                    cursor,
                    32.0 + row as f32 * 20.0,
                    g.w as f32 * scale,
                    g.h as f32 * scale,
                    [
                        g.x as f32 / WIDTH as f32,
                        g.y as f32 / HEIGHT as f32,
                        (g.x + g.w) as f32 / WIDTH as f32,
                        (g.y + g.h) as f32 / HEIGHT as f32,
                    ],
                    [240, 248, 255, 255],
                );
            }
            cursor += g.advance * scale;
        }
    }
    debug_assert!(out.len() / 20 <= MAX_VERTICES);
    out
}

#[allow(dead_code)]
pub fn atlas() -> Vec<u8> {
    font_metrics::ATLAS_BYTES.to_vec()
}
#[allow(dead_code)]
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
        b'+' => [0, 4, 4, 31, 4, 4, 0],
        b'<' => [2, 4, 8, 16, 8, 4, 2],
        b'>' => [8, 4, 2, 1, 2, 4, 8],
        b'%' => [25, 25, 2, 4, 8, 19, 19],
        b'=' => [0, 0, 31, 0, 31, 0, 0],
        b'(' => [2, 4, 8, 8, 8, 4, 2],
        b')' => [8, 4, 2, 2, 2, 4, 8],
        b' ' => [0; 7],
        _ => [14, 17, 1, 2, 4, 0, 4],
    }
}

#[cfg(test)]
mod atlas_tests {
    use super::*;
    #[test]
    fn licensed_atlas_metrics_stay_inside_the_texture() {
        assert_eq!(font_metrics::ATLAS_BYTES.len(), (WIDTH * HEIGHT) as usize);
        assert_eq!(font_metrics::ATLAS_BYTES[0], 255);
        for g in font_metrics::GLYPHS {
            assert!(u32::from(g.x) + u32::from(g.w) <= WIDTH);
            assert!(u32::from(g.y) + u32::from(g.h) <= HEIGHT);
            assert!(g.advance.is_finite() && g.advance > 0.0);
        }
    }
}
