//! Original material states over the terrain's shared reverse-Z attachment.
use super::{
    materials::Pass,
    mesh::{Draw, Stats},
    placement::Frame,
    Car,
};
use crate::harwood::upload;
use rerbr_render_core::coords;
use std::collections::{BTreeMap, BTreeSet};
use wgpu::util::DeviceExt;

pub(crate) struct Gpu {
    pipelines: BTreeMap<Pass, wgpu::RenderPipeline>,
    groups: Vec<wgpu::BindGroup>,
    camera_group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    uniform: wgpu::Buffer,
    draws: Vec<(Draw, Pass)>,
    pub opaque: Stats,
    pub transparent: Stats,
    cockpit: Option<rerbr_render_core::Mat4>,
}
impl Gpu {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        car: Car,
    ) -> Result<Self, String> {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let result = Self::build(device, queue, format, car);
        if let Some(e) = futures_executor::block_on(scope.pop()) {
            return Err(format!("CAR GPU pipeline/upload: {e}"));
        }
        if result.is_ok() {
            eprintln!("car-chase GPU validation PASS: pipeline/DDS commands accepted; no GPU-completion or visible result claimed");
        }
        result
    }
    fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        car: Car,
    ) -> Result<Self, String> {
        let opaque = car.geometry.stats(Some(false));
        let transparent = car.geometry.stats(Some(true));
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("original Corolla diffuse/alpha"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("car matrices"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("car diffuse"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shared car pipeline layouts"),
            bind_group_layouts: &[Some(&camera_layout), Some(&material_layout)],
            immediate_size: 0,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("car VP and five rigid models"),
            size: 464,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("car matrices"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        const ATTRS: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2];
        let mut pipelines = BTreeMap::new();
        for pass in car
            .geometry
            .materials
            .iter()
            .map(|m| m.pass)
            .collect::<BTreeSet<_>>()
        {
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Corolla material state"),
                layout: Some(&layout),
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
                    cull_mode: if pass.cull() {
                        Some(wgpu::Face::Back)
                    } else {
                        None
                    },
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(pass.depth_write()),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(if pass.blended() {
                        "fs_blend"
                    } else {
                        "fs_opaque"
                    }),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: pass.blended().then_some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
            pipelines.insert(pass, pipeline);
        }
        let mut textures = BTreeMap::new();
        for (path, data) in &car.textures {
            textures.insert(path, upload(device, queue, data)?);
        }
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("car wrap trilinear"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let mut groups = Vec::new();
        for m in &car.geometry.materials {
            let texture = textures
                .get(&m.path)
                .ok_or("CAR GPU: required diffuse missing")?;
            groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&m.name),
                layout: &material_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(texture),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            }));
        }
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("authentic Corolla vertices/normals/UV"),
            contents: &car.geometry.vertex_bytes(),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_bytes: Vec<_> = car
            .geometry
            .indices
            .iter()
            .flat_map(|i| i.to_ne_bytes())
            .collect();
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("authentic Corolla indices"),
            contents: &index_bytes,
            usage: wgpu::BufferUsages::INDEX,
        });
        let draws = car
            .geometry
            .draws
            .iter()
            .map(|d| (d.clone(), car.geometry.materials[d.material].pass))
            .collect();
        Ok(Self {
            pipelines,
            groups,
            camera_group,
            vertices,
            indices,
            uniform,
            draws,
            opaque,
            transparent,
            cockpit: car.cockpit,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        size: (u32, u32),
        format: wgpu::TextureFormat,
        frame: &Frame,
        interior: bool,
        steering_angle: f32,
    ) {
        let mut bytes: Vec<_> =
            super::placement::view_projection(frame.camera, size.0 as f32 / size.1 as f32)
                .to_cols_array()
                .into_iter()
                .flat_map(f32::to_ne_bytes)
                .collect();
        for model in frame.models {
            bytes.extend(model.to_cols_array().into_iter().flat_map(f32::to_ne_bytes));
        }
        let wheel = self
            .cockpit
            .map_or(rerbr_render_core::Mat4::IDENTITY, |node| {
                super::cockpit::steering_transform(node, steering_angle)
            });
        bytes.extend(
            (frame.models[0] * wheel)
                .to_cols_array()
                .into_iter()
                .flat_map(f32::to_ne_bytes),
        );
        bytes.extend(
            [u32::from(format.is_srgb()), 0, 0, 0]
                .into_iter()
                .flat_map(u32::to_ne_bytes),
        );
        queue.write_buffer(&self.uniform, 0, &bytes);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Corolla original material order"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        if let Some((_, state)) = self.draws.first() {
            pass.set_pipeline(&self.pipelines[state]);
        }
        pass.set_bind_group(0, &self.camera_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        for (draw, state) in &self.draws {
            if draw.interior != interior {
                continue;
            }
            pass.set_pipeline(&self.pipelines[state]);
            pass.set_bind_group(1, &self.groups[draw.material], &[]);
            pass.draw_indexed(
                draw.indices.start as u32..draw.indices.end as u32,
                0,
                draw.slot..draw.slot + 1,
            );
        }
    }
}
