//! Shared block/connected-terrain Metal pass. Triangle/cube passes remain separate.
use super::{texture::upload_plan, Camera, LoadedBlock, MODE_NAME};
use crate::asset_probe::TextureData;
use rerbr_asset_formats::dds;
use rerbr_render_core::coords;
use std::{collections::BTreeMap, path::PathBuf, sync::mpsc};
use wgpu::util::DeviceExt;

#[cfg(not(feature = "harwood-benchmark"))]
type Payload = LoadedBlock;
#[cfg(all(feature = "harwood-benchmark", not(feature = "car-chase-benchmark")))]
type Payload = super::benchmark::Prepared;
#[cfg(feature = "car-chase-benchmark")]
type Payload = crate::car::Prepared;
#[cfg(all(feature = "harwood-benchmark", not(feature = "car-chase-benchmark")))]
use super::benchmark::Runtime as BenchmarkRuntime;
#[cfg(feature = "car-chase-benchmark")]
use crate::car::runtime::Runtime as BenchmarkRuntime;

pub(crate) struct Scene {
    pending: Option<mpsc::Receiver<Result<Payload, String>>>,
    gpu: Option<Block>,
    failed: bool,
    mode: u32,
    render_scope: Option<wgpu::ErrorScopeGuard>,
    first_frame: bool,
    #[cfg(feature = "harwood-benchmark")]
    benchmark: BenchmarkRuntime,
}

impl Scene {
    #[cfg(feature = "physics-drive")]
    pub fn driving_touch(&mut self, touch: winit::event::Touch, size: (u32, u32)) {
        self.benchmark.driving.touch(touch, size);
    }
    pub fn new(bc_enabled: bool) -> Self {
        let mut scene = Self {
            pending: None,
            gpu: None,
            failed: false,
            mode: 0,
            render_scope: None,
            first_frame: true,
            #[cfg(feature = "harwood-benchmark")]
            benchmark: BenchmarkRuntime::new(),
        };
        if !bc_enabled {
            scene.fail("DEVICE: BC is unavailable; texture upload not attempted".into());
            return scene;
        }
        let Some(documents) = std::env::var_os("RERBR_PROBE_DOCUMENTS") else {
            scene.fail("READ: native Documents path unavailable".into());
            return scene;
        };
        #[cfg(feature = "car-chase-benchmark")]
        let car_root = crate::car::private_root(std::path::Path::new(&documents));
        let maps = PathBuf::from(documents).join("Harwood/Maps");
        let (tx, rx) = mpsc::channel();
        match std::thread::Builder::new()
            .name(format!("{MODE_NAME}-load"))
            .spawn(move || {
                #[cfg(not(feature = "harwood-neighbors"))]
                let result = super::load(&maps);
                #[cfg(all(feature = "harwood-neighbors", not(feature = "harwood-benchmark")))]
                let result = super::neighbors::load(&maps).map(|set| set.render);
                #[cfg(all(feature = "harwood-benchmark", not(feature = "car-chase-benchmark")))]
                let result = super::benchmark::load(&maps);
                #[cfg(feature = "car-chase-benchmark")]
                let result = crate::car::prepare(&car_root, &maps);
                let _ = tx.send(result);
            }) {
            Ok(_) => scene.pending = Some(rx),
            Err(e) => scene.fail(format!("READ: could not start worker: {e}")),
        }
        scene
    }

    fn fail(&mut self, error: String) {
        eprintln!("{MODE_NAME} FAIL: {error}; red background; see log; existing fallback modes remain separate");
        self.failed = true;
        self.gpu = None;
        #[cfg(feature = "harwood-benchmark")]
        self.benchmark.fail();
    }

    #[cfg(not(feature = "harwood-benchmark"))]
    pub fn cycle_mode(&mut self) {
        self.mode = (self.mode + 1) % 3;
        eprintln!(
            "{MODE_NAME} VIEW: {} (0=textures, 1=vertex colours, 2=UV checker)",
            self.mode
        );
    }

    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        data: Payload,
    ) -> Result<Block, String> {
        #[cfg(all(feature = "harwood-benchmark", not(feature = "car-chase-benchmark")))]
        let data = {
            self.benchmark.install(data.route);
            data.render
        };
        #[cfg(feature = "car-chase-benchmark")]
        let data = {
            let stats = crate::car::mesh::Stats {
                vertices: data.terrain.mesh.vertices.len(),
                triangles: data.terrain.mesh.indices.len() / 3,
                draws: data.terrain.mesh.draws.len(),
                textures: data.terrain.textures.len(),
            };
            self.benchmark
                .install(data.path, data.car, stats, device, queue, format)?;
            data.terrain
        };
        Block::new(device, queue, format, data)
    }

    #[cfg(all(feature = "harwood-benchmark", not(feature = "physics-drive")))]
    pub fn restart_benchmark(&mut self) {
        self.benchmark.restart();
    }
    #[cfg(feature = "harwood-benchmark")]
    pub fn interrupt_benchmark(&mut self, reason: &'static str) {
        self.benchmark.interrupt(reason);
    }
    #[cfg(feature = "harwood-benchmark")]
    pub fn benchmark_submitted(&mut self, cpu: std::time::Duration) {
        self.benchmark.submitted(cpu);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        format: wgpu::TextureFormat,
        background: wgpu::Color,
    ) {
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    match result.and_then(|data| self.upload(device, queue, format, data)) {
                        Ok(block) => self.gpu = Some(block),
                        Err(e) => self.fail(e),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.fail("READ: worker disconnected".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        #[cfg(feature = "harwood-benchmark")]
        if let Some(block) = &mut self.gpu {
            if let Err(error) = self.benchmark.before_frame(&mut block.camera) {
                self.fail(error);
            }
        }
        if let Some(block) = &mut self.gpu {
            if self.first_frame {
                self.render_scope = Some(device.push_error_scope(wgpu::ErrorFilter::Validation));
                eprintln!("{MODE_NAME} RENDER: encoding first frame with {} indexed draws; CW/back cull, Depth32Float Greater clear=0", block.draws.len());
            }
            block.draw(
                device,
                queue,
                encoder,
                target,
                size,
                self.mode,
                format.is_srgb(),
                background,
            );
            #[cfg(feature = "car-chase-benchmark")]
            self.benchmark.draw_car(
                queue,
                encoder,
                target,
                block.depth.as_ref().expect("shared active terrain depth"),
                size,
                format,
            );
        } else {
            let color = if self.failed {
                wgpu::Color {
                    r: 0.3,
                    g: 0.0,
                    b: 0.03,
                    a: 1.0,
                }
            } else {
                background
            };
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("harwood loading/error background"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        #[cfg(feature = "harwood-benchmark")]
        self.benchmark
            .overlay(device, queue, encoder, target, size, format);
    }

    /// Called after the host's encoder finish, submit and present request.
    pub fn finish_frame(&mut self) {
        if let Some(scope) = self.render_scope.take() {
            self.first_frame = false;
            match futures_executor::block_on(scope.pop()) {
                Some(error) => self.fail(format!("RENDER validation: {error}")),
                None => eprintln!("{MODE_NAME} RENDER validation PASS: commands submitted; present requested; image must be checked physically"),
            }
        }
    }
}

struct Block {
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    draws: Vec<(std::ops::Range<u32>, wgpu::BindGroup)>,
    camera: Camera,
    uniform: wgpu::Buffer,
    camera_group: wgpu::BindGroup,
    depth: Option<wgpu::TextureView>,
    depth_size: (u32, u32),
}

impl Block {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        data: LoadedBlock,
    ) -> Result<Self, String> {
        eprintln!(
            "{MODE_NAME} UPLOAD: payload anchored at block {} with {} unique DDS allocations",
            data.index,
            data.textures.len()
        );
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let result = Self::build(device, queue, format, data);
        if let Some(error) = futures_executor::block_on(scope.pop()) {
            return Err(format!("UPLOAD/pipeline validation: {error}"));
        }
        if result.is_ok() {
            eprintln!("{MODE_NAME} UPLOAD validation PASS: write_texture calls accepted; GPU completion/visible result not established");
        }
        result
    }

    fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        data: LoadedBlock,
    ) -> Result<Self, String> {
        let pipeline_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("harwood diffuse diagnostic"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        const ATTRS: [wgpu::VertexAttribute; 4] =
            wgpu::vertex_attr_array![0=>Float32x3, 1=>Unorm8x4, 2=>Float32x2, 3=>Float32x2];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("harwood CW reverse-Z"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 32,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &ATTRS,
                })],
            },
            primitive: wgpu::PrimitiveState {
                front_face: coords::FRONT_FACE,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        if let Some(error) = futures_executor::block_on(pipeline_scope.pop()) {
            return Err(format!("PIPELINE/shader validation: {error}"));
        }
        eprintln!("{MODE_NAME} PIPELINE validation PASS: diffuse/vertex-colour/UV views");
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("harwood camera and view options"),
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("harwood camera"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let mut textures = BTreeMap::new();
        for (id, texture) in &data.textures {
            textures.insert(*id, upload(device, queue, texture)?);
        }
        let white = solid(device, queue, [255; 4]);
        let transparent = solid(device, queue, [0; 4]);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("harwood wrap/trilinear"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let mut draws = Vec::new();
        for draw in &data.mesh.draws {
            let get = |id: Option<u32>,
                       fallback: &wgpu::TextureView|
             -> Result<wgpu::TextureView, String> {
                match id {
                    Some(id) => textures
                        .get(&id)
                        .cloned()
                        .ok_or_else(|| format!("missing uploaded Texture{id}")),
                    None => Ok(fallback.clone()),
                }
            };
            let t0 = get(draw.textures[0], &white)?;
            let t1 = get(draw.textures[1], &transparent)?;
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("harwood two diffuse textures"),
                layout: &pipeline.get_bind_group_layout(1),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&t0),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&t1),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            draws.push((draw.indices.start as u32..draw.indices.end as u32, group));
        }
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("harwood authentic vertices"),
            contents: &data.mesh.vertex_bytes(),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_bytes: Vec<_> = data
            .mesh
            .indices
            .iter()
            .flat_map(|i| i.to_ne_bytes())
            .collect();
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("harwood authentic indices"),
            contents: &index_bytes,
            usage: wgpu::BufferUsages::INDEX,
        });
        Ok(Self {
            pipeline,
            vertices,
            indices,
            draws,
            camera: data.camera,
            uniform,
            camera_group,
            depth: None,
            depth_size: (0, 0),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        mode: u32,
        srgb: bool,
        background: wgpu::Color,
    ) {
        if size != self.depth_size {
            self.depth = Some(
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("harwood depth"),
                        size: wgpu::Extent3d {
                            width: size.0,
                            height: size.1,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Depth32Float,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                    .create_view(&Default::default()),
            );
            self.depth_size = size;
            #[cfg(feature = "harwood-neighbors")]
            eprintln!(
                "harwood-neighbors MEMORY: depth={} bytes at {}x{}; one shared depth target",
                u64::from(size.0) * u64::from(size.1) * 4,
                size.0,
                size.1
            );
            eprintln!(
                "{MODE_NAME} RENDER: depth resized {}x{}; surface sRGB={srgb}",
                size.0, size.1
            );
        }
        #[cfg(not(feature = "car-chase-benchmark"))]
        let vp = self.camera.view_projection(size.0 as f32 / size.1 as f32);
        #[cfg(feature = "car-chase-benchmark")]
        let vp = crate::car::placement::view_projection(self.camera, size.0 as f32 / size.1 as f32);
        let mut bytes: Vec<_> = vp
            .to_cols_array()
            .into_iter()
            .flat_map(f32::to_ne_bytes)
            .collect();
        bytes.extend(
            [mode, u32::from(srgb), 0, 0]
                .into_iter()
                .flat_map(u32::to_ne_bytes),
        );
        queue.write_buffer(&self.uniform, 0, &bytes);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("harwood real block"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(background),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: self.depth.as_ref().expect("active depth"),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(coords::DEPTH_CLEAR),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        for (indices, material) in &self.draws {
            pass.set_bind_group(1, material, &[]);
            pass.draw_indexed(indices.clone(), 0, 0..1);
        }
    }
}

pub(crate) fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &TextureData,
) -> Result<wgpu::TextureView, String> {
    let plan = upload_plan(&data.bytes, device.limits().max_texture_dimension_2d)?;
    let first = plan.mips.first().ok_or("DDS without mips")?;
    let format = match plan.format {
        dds::Format::Dxt1 => wgpu::TextureFormat::Bc1RgbaUnorm,
        dds::Format::Dxt3 => wgpu::TextureFormat::Bc2RgbaUnorm,
        _ => return Err("unsupported BC format".into()),
    };
    eprintln!(
        "{MODE_NAME} UPLOAD: {} | {format:?} {}x{} mips={}",
        data.info.resolved,
        first.width,
        first.height,
        plan.mips.len()
    );
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(&data.info.resolved),
        size: wgpu::Extent3d {
            width: first.width,
            height: first.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: plan.mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, mip) in plan.mips.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data.bytes[mip.offset..mip.offset + mip.len],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(mip.bytes_per_row),
                rows_per_image: Some(mip.block_rows),
            },
            wgpu::Extent3d {
                width: mip.copy_width,
                height: mip.copy_height,
                depth_or_array_layers: 1,
            },
        );
    }
    Ok(texture.create_view(&Default::default()))
}

fn solid(device: &wgpu::Device, queue: &wgpu::Queue, color: [u8; 4]) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("harwood untextured dummy"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
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
        &color,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    texture.create_view(&Default::default())
}
