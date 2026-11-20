//! Lightweight 3D Rally Showroom / Garage renderer for Title Screen, Horizontal Main Menu,
//! and interactive Garage inspection. Loads at most ONE 3D car on demand via a background
//! worker thread and immediately releases GPU memory when a driving stage starts.

use super::{gpu::Gpu, placement::Frame, Car};
use crate::{driving::loading::Loader, harwood::Camera};
use rerbr_content::VehicleDescriptor;
use rerbr_render_core::{coords, Mat4, Vec3};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShowroomMode {
    #[default]
    Title,
    MainMenu,
    Garage,
}

/// Interactive orbital camera + smooth transitions between Title, MainMenu, and Garage.
#[derive(Debug, Clone)]
pub struct ShowroomCamera {
    pub mode: ShowroomMode,
    pub yaw: f32,
    pub pitch: f32,
    pub yaw_vel: f32,
    pub pitch_vel: f32,
    pub distance_scale: f32,
    pub target_y_bias: f32,
    pub dragging: bool,
    last_frame: Instant,
    user_interacted_at: Option<Instant>,
}

impl Default for ShowroomCamera {
    fn default() -> Self {
        Self {
            mode: ShowroomMode::Title,
            yaw: 0.68,
            pitch: 0.21,
            yaw_vel: 0.0,
            pitch_vel: 0.0,
            distance_scale: 1.05,
            target_y_bias: -0.18,
            dragging: false,
            last_frame: Instant::now(),
            user_interacted_at: None,
        }
    }
}

impl ShowroomCamera {
    pub fn set_mode(&mut self, mode: ShowroomMode) {
        if self.mode == mode {
            return;
        }
        let prev = self.mode;
        self.mode = mode;
        if prev == ShowroomMode::Title && mode == ShowroomMode::MainMenu {
            // Smooth visual sweep into the hero 3/4 front-right main menu angle.
            self.yaw_vel = 0.0;
            self.pitch_vel = 0.0;
        }
    }

    /// Applies normalized touch drag deltas (`dx, dy` in `0..1` screen fraction).
    pub fn drag(&mut self, dx: f32, dy: f32) {
        if !dx.is_finite() || !dy.is_finite() {
            return;
        }
        let dyaw = -dx * 4.4;
        let dpitch = dy * 2.2;
        self.yaw = (self.yaw + dyaw).rem_euclid(std::f32::consts::TAU);
        self.pitch = (self.pitch + dpitch).clamp(0.10, 0.64);
        self.yaw_vel = dyaw * 24.0;
        self.pitch_vel = dpitch * 20.0;
        self.dragging = true;
        self.user_interacted_at = Some(Instant::now());
    }

    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.user_interacted_at = Some(Instant::now());
    }

    pub fn step(&mut self, now: Instant) {
        let dt = now
            .saturating_duration_since(self.last_frame)
            .as_secs_f32()
            .clamp(0.0, 0.05);
        self.last_frame = now;
        if dt <= 0.0 {
            return;
        }

        // Smooth camera framing targets per mode so the car is always centered in the
        // unobstructed viewport above the bottom horizontal dock.
        let (target_dist_scale, target_y_bias, target_pitch) = match self.mode {
            ShowroomMode::Title => (1.06, -0.18, 0.19),
            ShowroomMode::MainMenu => (1.02, -0.24, 0.22),
            ShowroomMode::Garage => (0.94, -0.20, 0.24),
        };
        let smooth = 1.0 - (-dt * 5.5).exp();
        self.distance_scale += (target_dist_scale - self.distance_scale) * smooth;
        self.target_y_bias += (target_y_bias - self.target_y_bias) * smooth;

        if !self.dragging {
            let decay = (-dt * 6.0).exp();
            self.yaw = (self.yaw + self.yaw_vel * dt).rem_euclid(std::f32::consts::TAU);
            self.pitch = (self.pitch + self.pitch_vel * dt).clamp(0.10, 0.64);
            self.yaw_vel *= decay;
            self.pitch_vel *= decay;

            match self.mode {
                ShowroomMode::Title => {
                    // Slow cinematic turntable orbit on the Title Screen.
                    self.yaw = (self.yaw + dt * 0.16).rem_euclid(std::f32::consts::TAU);
                    self.pitch += (target_pitch - self.pitch) * (1.0 - (-dt * 2.5).exp());
                }
                ShowroomMode::MainMenu | ShowroomMode::Garage => {
                    // Gently resume a very slow ambient drift only after 5s of idle touch.
                    let idle = self
                        .user_interacted_at
                        .is_none_or(|t| now.saturating_duration_since(t) >= Duration::from_secs(5));
                    if idle {
                        self.yaw = (self.yaw + dt * 0.055).rem_euclid(std::f32::consts::TAU);
                    }
                }
            }
        }
    }

    pub fn camera_and_fov(&self, center_y: f32, radius: f32, aspect: f32) -> (Camera, f32) {
        let safe_aspect = if aspect.is_finite() && aspect > 0.3 {
            aspect
        } else {
            1.55
        };
        // Wide iPhone 15 (~2.16:1) gets a slightly wider fit so the car never clips vertically;
        // iPad mini (~1.53:1) uses a classic 46° studio lens.
        let fov: f32 = if safe_aspect > 1.85 { 44.0 } else { 46.0 };
        let base_radius = radius.clamp(1.75, 3.2);
        let dist = (base_radius * 2.38 * self.distance_scale).clamp(3.6, 8.2);
        // Shift camera target slightly downward so the car sits in the upper-middle studio area,
        // cleanly above the bottom horizontal menu bar.
        let target = Vec3::new(0.0, center_y + self.target_y_bias, 0.0);
        let cos_p = self.pitch.cos();
        let eye = target
            + Vec3::new(
                self.yaw.sin() * cos_p * dist,
                (self.pitch.sin() * dist).max(0.35),
                self.yaw.cos() * cos_p * dist,
            );
        (
            Camera {
                eye,
                target,
                up: Vec3::Y,
            },
            fov,
        )
    }
}

/// CPU-prepared showroom car metrics + GPU upload payload.
struct LoadedShowroomCar {
    vehicle_id: String,
    vehicle_name: String,
    gpu: Gpu,
    model: Mat4,
    wheel_models: [Mat4; 4],
    door_dorsals: Option<[(Vec3, Vec3); 2]>,
    center_y: f32,
    radius: f32,
    half_x: f32,
    half_z: f32,
}

impl LoadedShowroomCar {
    fn from_car(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        car: Car,
    ) -> Result<Self, String> {
        let vehicle_id = car.vehicle.id.clone();
        let vehicle_name = car.vehicle.name.clone();

        // Seat tyres flush on y = 0.0 and center on XZ using authentic wheel/pivot geometry.
        let low_y = car
            .geometry
            .wheel_points
            .iter()
            .filter(|pts| !pts.is_empty())
            .map(|pts| pts.iter().map(|p| p.y).fold(f32::INFINITY, f32::min))
            .collect::<Vec<_>>();
        let floor_y = if low_y.is_empty() {
            car.geometry
                .vertices
                .iter()
                .map(|v| v.position[1])
                .fold(f32::INFINITY, f32::min)
        } else {
            low_y.iter().sum::<f32>() / low_y.len() as f32
        };
        let avg_pivot = car.geometry.pivots.iter().copied().sum::<Vec3>() * 0.25;
        let reference = Vec3::new(avg_pivot.x, floor_y, avg_pivot.z);
        let mut model = coords::car_to_stage(Vec3::ZERO, Vec3::Z);
        model.w_axis = (-model.transform_vector3(reference)).extend(1.0);
        let wheel_models = std::array::from_fn(|k| {
            model * Mat4::from_translation(car.geometry.wheel_visual_offsets[k])
        });

        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for v in &car.geometry.vertices {
            let p = model.transform_point3(Vec3::from(v.position));
            if p.is_finite() {
                min = min.min(p);
                max = max.max(p);
            }
        }
        let (center_y, radius, half_x, half_z) = if min.is_finite() && max.is_finite() {
            let ext = (max - min) * 0.5;
            let cy = ((min.y + max.y) * 0.5).clamp(0.45, 1.35);
            let r = ext.length().clamp(1.8, 3.1);
            (cy, r, ext.x.clamp(0.75, 1.35), ext.z.clamp(1.6, 2.8))
        } else {
            (0.72, 2.35, 0.95, 2.15)
        };

        let door_dorsals = car.geometry.door_dorsals;
        let gpu = Gpu::new(device, queue, format, car)?;
        Ok(Self {
            vehicle_id,
            vehicle_name,
            gpu,
            model,
            wheel_models,
            door_dorsals,
            center_y,
            radius,
            half_x,
            half_z,
        })
    }
}

struct StudioPipelines {
    format: wgpu::TextureFormat,
    backdrop: wgpu::RenderPipeline,
    floor: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
}

impl StudioPipelines {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rally showroom studio"),
            source: wgpu::ShaderSource::Wgsl(include_str!("showroom.wgsl").into()),
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("showroom studio uniform"),
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("showroom studio layout"),
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
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("showroom studio bind group"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("showroom studio pipeline layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let backdrop = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("showroom backdrop"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_backdrop"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_backdrop"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let floor = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("showroom floor"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_floor"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_floor"),
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
        Self {
            format,
            backdrop,
            floor,
            uniform,
            group,
        }
    }
}

/// Manages on-demand background loading, fallback retries, and 3D rendering for the Showroom.
#[derive(Default)]
pub(crate) struct Showroom {
    pipelines: Option<StudioPipelines>,
    depth: Option<wgpu::TextureView>,
    depth_size: (u32, u32),
    loader: Loader<LoadedShowroomCar>,
    active: Option<LoadedShowroomCar>,
    requested_id: Option<String>,
    fallback_queue: Vec<VehicleDescriptor>,
    pub last_error: Option<String>,
    pub loading: bool,
}

impl Showroom {
    /// Immediately releases GPU buffers/textures (e.g. before loading a driving stage).
    pub fn unload(&mut self) {
        self.loader.cancel();
        self.active = None;
        self.requested_id = None;
        self.fallback_queue.clear();
        self.loading = false;
        self.last_error = None;
    }

    pub fn active_vehicle_id(&self) -> Option<&str> {
        self.active.as_ref().map(|c| c.vehicle_id.as_str())
    }

    #[allow(dead_code)]
    pub fn active_vehicle_name(&self) -> Option<&str> {
        self.active.as_ref().map(|c| c.vehicle_name.as_str())
    }

    /// Requests displaying `primary`, automatically falling back through `fallbacks` if
    /// `primary` fails to parse or upload. Does nothing if `primary` is already loaded/loading.
    pub fn ensure_vehicle(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        primary: &VehicleDescriptor,
        fallbacks: Vec<VehicleDescriptor>,
    ) {
        if self.requested_id.as_deref() == Some(primary.id.as_str()) {
            return;
        }
        self.fallback_queue = fallbacks;
        self.start_load(device, queue, format, primary.clone());
    }

    fn start_load(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        vehicle: VehicleDescriptor,
    ) {
        self.requested_id = Some(vehicle.id.clone());
        self.loading = true;
        self.last_error = None;
        let device = device.clone();
        let queue = queue.clone();
        self.loader.request(Box::new(move |cancel| {
            let documents =
                std::env::var_os("RERBR_PROBE_DOCUMENTS").ok_or("Documents unavailable")?;
            let car_root = super::private_root(&PathBuf::from(documents));
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("showroom load cancelled".into());
            }
            let car = super::load_showroom_model(&car_root, &vehicle)?;
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("showroom load cancelled".into());
            }
            LoadedShowroomCar::from_car(&device, &queue, format, car)
        }));
    }

    #[allow(clippy::too_many_arguments)]
    /// Projects the active showroom car's door-side dorsal anchors (`e_door_l`, `e_door_r`) into
    /// normalized screen coordinates `[0..1]` when facing the camera, without altering original assets.
    pub fn dorsal_screen_positions(
        &self,
        camera_state: &ShowroomCamera,
        size: (u32, u32),
    ) -> Vec<(f32, f32)> {
        let Some(car) = &self.active else {
            return Vec::new();
        };
        let Some(dorsals) = car.door_dorsals else {
            return Vec::new();
        };
        let aspect = size.0.max(1) as f32 / size.1.max(1) as f32;
        let (camera, fov) = camera_state.camera_and_fov(car.center_y, car.radius, aspect);
        let vp = super::placement::view_projection_with_fov(camera, aspect, fov);
        let mut out = Vec::new();
        for (center, normal) in dorsals {
            let wp = car.model.transform_point3(center + normal * 0.025);
            let wn = car.model.transform_vector3(normal).normalize_or_zero();
            let to_cam = (camera.eye - wp).normalize_or_zero();
            if wn.dot(to_cam) <= 0.30 {
                continue;
            }
            let clip = vp * wp.extend(1.0);
            if clip.w <= 0.1 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            let nx = ndc.x * 0.5 + 0.5;
            let ny = 0.5 - ndc.y * 0.5;
            if (0.06..=0.94).contains(&nx) && (0.14..=0.72).contains(&ny) {
                out.push((nx, ny));
            }
        }
        out
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
        camera_state: &mut ShowroomCamera,
    ) {
        if let Some(res) = self.loader.poll() {
            self.loading = false;
            match res {
                Ok(loaded) => {
                    eprintln!(
                        "showroom LOAD PASS: {} ({}) radius={:.2}m",
                        loaded.vehicle_id, loaded.vehicle_name, loaded.radius
                    );
                    self.active = Some(loaded);
                    self.last_error = None;
                    self.fallback_queue.clear();
                }
                Err(err) => {
                    eprintln!(
                        "showroom LOAD FAIL ({:?}): {err}",
                        self.requested_id.as_deref().unwrap_or("none")
                    );
                    self.last_error = Some(err);
                    if let Some(next) = self.fallback_queue.first().cloned() {
                        self.fallback_queue.remove(0);
                        self.start_load(device, queue, format, next);
                    } else {
                        // Keep previous active car cleared if none available; studio backdrop stays alive.
                        self.active = None;
                    }
                }
            }
        }

        if self.pipelines.as_ref().is_none_or(|p| p.format != format) {
            self.pipelines = Some(StudioPipelines::new(device, format));
        }
        if size != self.depth_size || self.depth.is_none() {
            let w = size.0.max(1);
            let h = size.1.max(1);
            self.depth = Some(
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("showroom depth"),
                        size: wgpu::Extent3d {
                            width: w,
                            height: h,
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
        }

        camera_state.step(Instant::now());
        let aspect = size.0.max(1) as f32 / size.1.max(1) as f32;
        let (center_y, radius, half_x, half_z) =
            self.active.as_ref().map_or((0.72, 2.35, 0.95, 2.15), |c| {
                (c.center_y, c.radius, c.half_x, c.half_z)
            });
        let (camera, fov) = camera_state.camera_and_fov(center_y, radius, aspect);
        let vp = super::placement::view_projection_with_fov(camera, aspect, fov);

        let pipelines = self.pipelines.as_ref().expect("showroom pipelines");
        let mut uniform_bytes = Vec::with_capacity(80);
        for f in vp.to_cols_array() {
            uniform_bytes.extend(f.to_ne_bytes());
        }
        let ped_radius = (radius * 1.18).max(2.65);
        for f in [
            half_x,
            half_z,
            ped_radius,
            if format.is_srgb() { 1.0 } else { 0.0 },
        ] {
            uniform_bytes.extend(f.to_ne_bytes());
        }
        queue.write_buffer(&pipelines.uniform, 0, &uniform_bytes);

        // Pass 1: Studio gradient backdrop (no depth).
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("showroom backdrop"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.04,
                            g: 0.05,
                            b: 0.07,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&pipelines.backdrop);
            pass.set_bind_group(0, &pipelines.group, &[]);
            pass.draw(0..3, 0..1);
        }

        // Pass 2: Reflective turntable floor with depth clear.
        let depth = self.depth.as_ref().expect("showroom depth");
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("showroom floor"),
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
                        load: wgpu::LoadOp::Clear(coords::DEPTH_CLEAR),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&pipelines.floor);
            pass.set_bind_group(0, &pipelines.group, &[]);
            pass.draw(0..6, 0..1);
        }

        // Pass 3: Real 3D car rendered with studio lighting onto the shared reverse-Z depth buffer.
        if let Some(car) = &self.active {
            let frame = Frame {
                models: [
                    car.model,
                    car.wheel_models[0],
                    car.wheel_models[1],
                    car.wheel_models[2],
                    car.wheel_models[3],
                ],
                camera,
                max_wheel_travel: 0.0,
                min_wheel_clearance: 0.0,
                boom: 0.0,
                interior: false,
            };
            car.gpu
                .draw_showroom(queue, encoder, target, depth, size, format, &frame, fov);
        }
    }
}
