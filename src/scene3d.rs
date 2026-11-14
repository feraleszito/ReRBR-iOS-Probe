//! Optional floor/cube pass. The host retains its Metal device, surface and lifecycle.

use rerbr_render_core::coords;
use wgpu::util::DeviceExt;

use crate::scene_geometry::{floor_and_cube, view_projection};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const VERTEX_STRIDE: u64 = 6 * 4;

pub(crate) struct Scene3d {
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    camera: wgpu::Buffer,
    camera_group: wgpu::BindGroup,
    depth: Option<wgpu::TextureView>,
    depth_size: (u32, u32),
}

impl Scene3d {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let mesh = floor_and_cube();
        // Serialize explicitly: no unsafe casts, extra crate, or struct-padding assumptions.
        let vertices: Vec<u8> = mesh
            .vertices
            .iter()
            .flat_map(|v| {
                v.position
                    .into_iter()
                    .chain(v.color)
                    .flat_map(f32::to_ne_bytes)
            })
            .collect();
        let indices: Vec<u8> = mesh.indices.iter().flat_map(|i| i.to_ne_bytes()).collect();
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("procedural floor/cube vertices"),
            contents: &vertices,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("procedural floor/cube indices"),
            contents: &indices,
            usage: wgpu::BufferUsages::INDEX,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("floor/cube diagnostic shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene3d.wgsl").into()),
        });
        const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("floor/cube reverse-Z pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: VERTEX_STRIDE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: coords::FRONT_FACE,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fixed reRBR camera matrix"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fixed camera"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        eprintln!(
            "scene: rerbr-3d; fixed reRBR camera; {} vertices, {} triangles; reverse-Z",
            mesh.vertices.len(),
            mesh.indices.len() / 3
        );
        Self {
            pipeline,
            vertices,
            indices,
            index_count: mesh.indices.len() as u32,
            camera,
            camera_group,
            depth: None,
            depth_size: (0, 0),
        }
    }

    pub(crate) fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        background: wgpu::Color,
    ) {
        // The host calls us only for an active, nonzero drawable. Recreate depth after
        // rotation/resize; a view owns its texture's lifetime, as in reRBR's depth_view.
        if self.depth_size != size {
            self.depth = Some(
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("floor/cube reverse depth"),
                        size: wgpu::Extent3d {
                            width: size.0,
                            height: size.1,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: DEPTH_FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                    .create_view(&Default::default()),
            );
            self.depth_size = size;
            eprintln!("scene depth resized: {}x{}", size.0, size.1);
        }
        let matrix = view_projection(size.0 as f32 / size.1 as f32);
        let mut bytes = [0u8; 64];
        for (out, value) in bytes.chunks_exact_mut(4).zip(matrix.to_cols_array()) {
            out.copy_from_slice(&value.to_ne_bytes());
        }
        queue.write_buffer(&self.camera, 0, &bytes);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("floor/cube"),
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
                view: self
                    .depth
                    .as_ref()
                    .expect("depth allocated for the drawable"),
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
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}
