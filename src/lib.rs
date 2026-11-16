//! The native executable calls this library on the main thread, before UIApplicationMain.
//! Winit owns UIKit's application loop. There is no second app delegate or SwiftUI loop.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_executor::block_on;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
#[cfg(not(feature = "physics-drive"))]
use winit::event::TouchPhase;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

#[cfg(all(feature = "physics-probe", not(feature = "physics-drive")))]
mod physics_probe;

#[cfg(feature = "physics-drive")]
pub mod driving;

#[cfg(feature = "car-chase-benchmark")]
pub mod car;

#[cfg(any(
    feature = "rerbr-3d",
    feature = "harwood-block",
    feature = "harwood-neighbors"
))]
pub mod asset_probe;
#[cfg(any(feature = "harwood-block", feature = "harwood-neighbors"))]
pub mod harwood;
#[cfg(all(
    feature = "rerbr-3d",
    not(any(feature = "harwood-block", feature = "harwood-neighbors"))
))]
mod scene3d;
#[cfg(all(
    feature = "rerbr-3d",
    not(any(feature = "harwood-block", feature = "harwood-neighbors"))
))]
mod scene_geometry;

const FRAME_PERIOD: Duration = Duration::from_nanos(16_666_667);

/// Called once by xcode/ReRBRProbe/main.m on the process's main thread.
/// On iOS run_app takes over the process; a normal return is not expected.
#[no_mangle]
pub extern "C" fn rerbr_probe_main() -> i32 {
    eprintln!("ReRBRProbe 0.1.0: starting winit 0.30.13 / wgpu 30.0.1 (Metal only)");
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("FATAL: event loop: {error}");
            return 1;
        }
    };
    let mut app = Probe::default();
    match event_loop.run_app(&mut app) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("FATAL: application: {error}");
            1
        }
    }
}

#[derive(Default)]
struct Probe {
    renderer: Option<Renderer>,
    active: bool,
    occluded: bool,
    next_frame: Option<Instant>,
}

impl ApplicationHandler for Probe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        eprintln!("lifecycle: resumed");
        if self.renderer.is_none() {
            let attributes = Window::default_attributes().with_title("ReRBRProbe - Metal");
            let window = Arc::new(
                event_loop
                    .create_window(attributes)
                    .expect("create probe window"),
            );
            self.renderer = Some(block_on(Renderer::new(window)).unwrap_or_else(|error| {
                eprintln!("FATAL: GPU initialization: {error}");
                panic!("GPU initialization: {error}");
            }));
        }
        self.active = true;
        self.occluded = false;
        let renderer = self.renderer.as_mut().expect("renderer initialized");
        renderer.resize(renderer.window.inner_size());
        self.next_frame = Some(Instant::now());
        renderer.window.request_redraw();
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        eprintln!("lifecycle: suspended; GPU submissions paused");
        #[cfg(all(feature = "physics-probe", not(feature = "physics-drive")))]
        if let Some(renderer) = &mut self.renderer {
            renderer.physics.interrupt();
        }
        #[cfg(feature = "harwood-benchmark")]
        if let Some(renderer) = &mut self.renderer {
            renderer.harwood.interrupt_benchmark("APP SUSPENDED");
        }
        self.active = false;
        self.next_frame = None;
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        if renderer.window.id() != id {
            return;
        }
        match event {
            WindowEvent::Resized(size) => {
                renderer.resize(size);
                #[cfg(feature = "physics-drive")]
                renderer
                    .harwood
                    .interrupt_benchmark("SIZE / ORIENTATION CHANGE");
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                #[cfg(all(feature = "physics-probe", not(feature = "physics-drive")))]
                if occluded {
                    renderer.physics.interrupt();
                }
                #[cfg(feature = "harwood-benchmark")]
                if occluded {
                    renderer.harwood.interrupt_benchmark("APP OCCLUDED");
                }
                self.next_frame = Some(Instant::now());
            }
            #[cfg(feature = "physics-drive")]
            WindowEvent::Touch(touch) => {
                let size = renderer.window.inner_size();
                renderer
                    .harwood
                    .driving_touch(touch, (size.width, size.height));
            }
            #[cfg(not(feature = "physics-drive"))]
            WindowEvent::Touch(touch) if touch.phase == TouchPhase::Started => {
                #[cfg(all(feature = "physics-probe", not(feature = "physics-drive")))]
                renderer.physics.restart();
                #[cfg(feature = "harwood-benchmark")]
                if !cfg!(feature = "physics-probe") {
                    renderer.harwood.restart_benchmark();
                }
                #[cfg(not(any(feature = "harwood-benchmark", feature = "physics-probe")))]
                {
                    renderer.alternate_background = !renderer.alternate_background;
                    #[cfg(any(feature = "harwood-block", feature = "harwood-neighbors"))]
                    renderer.harwood.cycle_mode();
                    eprintln!("touch: background changed");
                }
            }
            WindowEvent::RedrawRequested if self.active && !self.occluded => {
                renderer.draw();
                self.next_frame = Some(Instant::now() + FRAME_PERIOD);
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.active || self.occluded {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        #[cfg(feature = "harwood-benchmark")]
        {
            // Let FIFO/Metal and UIKit pace rendering; do not add a 16.7ms sleep
            // after CPU work, which would artificially cap the benchmark.
            if let Some(renderer) = &self.renderer {
                renderer.window.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::Poll);
        }
        #[cfg(not(feature = "harwood-benchmark"))]
        {
            let next = self.next_frame.unwrap_or_else(Instant::now);
            if Instant::now() >= next {
                if let Some(renderer) = &self.renderer {
                    renderer.window.request_redraw();
                }
                // Avoid a busy loop if UIKit coalesces the redraw request.
                self.next_frame = Some(Instant::now() + FRAME_PERIOD);
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame.unwrap_or(next)));
        }
    }
}

struct Renderer {
    #[cfg(all(feature = "physics-probe", not(feature = "physics-drive")))]
    physics: physics_probe::Scene,
    surface: wgpu::Surface<'static>,
    window: Arc<Window>,
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    #[cfg(not(any(
        feature = "rerbr-3d",
        feature = "harwood-block",
        feature = "harwood-neighbors"
    )))]
    pipeline: wgpu::RenderPipeline,
    #[cfg(not(any(
        feature = "rerbr-3d",
        feature = "harwood-block",
        feature = "harwood-neighbors"
    )))]
    uniform: wgpu::Buffer,
    #[cfg(not(any(
        feature = "rerbr-3d",
        feature = "harwood-block",
        feature = "harwood-neighbors"
    )))]
    bind_group: wgpu::BindGroup,
    #[cfg(all(
        feature = "rerbr-3d",
        not(any(feature = "harwood-block", feature = "harwood-neighbors"))
    ))]
    scene: scene3d::Scene3d,
    #[cfg(any(feature = "harwood-block", feature = "harwood-neighbors"))]
    harwood: harwood::Scene,
    size: PhysicalSize<u32>,
    reconfigure: bool,
    #[cfg(not(any(
        feature = "rerbr-3d",
        feature = "harwood-block",
        feature = "harwood-neighbors"
    )))]
    started: Instant,
    frames: u64,
    alternate_background: bool,
}

impl Renderer {
    async fn new(window: Arc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;
        let info = adapter.get_info();
        if info.backend != wgpu::Backend::Metal {
            return Err("the probe requires a Metal adapter".into());
        }
        eprintln!("GPU: {} / {:?}", info.name, info.backend);
        eprintln!(
            "BC textures supported: {}",
            adapter
                .features()
                .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
        );
        eprintln!("GPU limits: {:?}", adapter.limits());
        // No BC textures or other optional features are needed to draw the triangle.
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("ReRBRProbe Metal device"),
                #[cfg(any(feature = "harwood-block", feature = "harwood-neighbors"))]
                required_features: adapter.features() & wgpu::Features::TEXTURE_COMPRESSION_BC,
                ..Default::default()
            })
            .await?;
        #[cfg(all(
            feature = "rerbr-3d",
            not(any(feature = "harwood-block", feature = "harwood-neighbors"))
        ))]
        asset_probe::start_from_documents(
            adapter
                .features()
                .contains(wgpu::Features::TEXTURE_COMPRESSION_BC),
            device
                .features()
                .contains(wgpu::Features::TEXTURE_COMPRESSION_BC),
            device.limits().max_texture_dimension_2d,
        );
        #[cfg(any(feature = "harwood-block", feature = "harwood-neighbors"))]
        eprintln!(
            "{} DEVICE: adapter BC={}, device BC enabled={}",
            harwood::MODE_NAME,
            adapter
                .features()
                .contains(wgpu::Features::TEXTURE_COMPRESSION_BC),
            device
                .features()
                .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
        );
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("no supported surface configuration")?;
        config.present_mode = wgpu::PresentMode::Fifo;
        eprintln!(
            "surface: {}x{} {:?}",
            config.width, config.height, config.format
        );

        #[cfg(not(any(
            feature = "rerbr-3d",
            feature = "harwood-block",
            feature = "harwood-neighbors"
        )))]
        let (pipeline, uniform, bind_group) = {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("animated triangle"),
                source: wgpu::ShaderSource::Wgsl(include_str!("triangle.wgsl").into()),
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("probe triangle pipeline"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(config.format.into())],
                }),
                multiview_mask: None,
                cache: None,
            });
            let uniform = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("16-byte animation uniform"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("animation"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                }],
            });
            (pipeline, uniform, bind_group)
        };
        #[cfg(all(
            feature = "rerbr-3d",
            not(any(feature = "harwood-block", feature = "harwood-neighbors"))
        ))]
        let scene = scene3d::Scene3d::new(&device, config.format);
        #[cfg(any(feature = "harwood-block", feature = "harwood-neighbors"))]
        let harwood = harwood::Scene::new(
            device
                .features()
                .contains(wgpu::Features::TEXTURE_COMPRESSION_BC),
        );
        Ok(Self {
            #[cfg(all(feature = "physics-probe", not(feature = "physics-drive")))]
            physics: physics_probe::Scene::new(),
            surface,
            window,
            instance,
            device,
            queue,
            config,
            #[cfg(not(any(
                feature = "rerbr-3d",
                feature = "harwood-block",
                feature = "harwood-neighbors"
            )))]
            pipeline,
            #[cfg(not(any(
                feature = "rerbr-3d",
                feature = "harwood-block",
                feature = "harwood-neighbors"
            )))]
            uniform,
            #[cfg(not(any(
                feature = "rerbr-3d",
                feature = "harwood-block",
                feature = "harwood-neighbors"
            )))]
            bind_group,
            #[cfg(all(
                feature = "rerbr-3d",
                not(any(feature = "harwood-block", feature = "harwood-neighbors"))
            ))]
            scene,
            #[cfg(any(feature = "harwood-block", feature = "harwood-neighbors"))]
            harwood,
            size,
            reconfigure: true,
            #[cfg(not(any(
                feature = "rerbr-3d",
                feature = "harwood-block",
                feature = "harwood-neighbors"
            )))]
            started: Instant::now(),
            frames: 0,
            alternate_background: false,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        #[cfg(feature = "harwood-benchmark")]
        if size != self.size {
            self.harwood.interrupt_benchmark("RESOLUTION CHANGED");
        }
        self.size = size;
        self.config.width = size.width.max(1);
        self.config.height = size.height.max(1);
        // Configure only when drawing an active, nonzero surface.
        self.reconfigure = true;
    }

    fn draw(&mut self) {
        if self.size.width == 0 || self.size.height == 0 {
            return;
        }
        if self.reconfigure {
            self.surface.configure(&self.device, &self.config);
            self.reconfigure = false;
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.reconfigure = true;
                frame
            }
            wgpu::CurrentSurfaceTexture::Timeout => return,
            wgpu::CurrentSurfaceTexture::Occluded => {
                #[cfg(feature = "harwood-benchmark")]
                self.harwood.interrupt_benchmark("SURFACE OCCLUDED");
                return;
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.resize(self.window.inner_size());
                return;
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                #[cfg(feature = "harwood-benchmark")]
                self.harwood.interrupt_benchmark("SURFACE LOST");
                eprintln!("surface lost; recreating it");
                self.surface = self
                    .instance
                    .create_surface(self.window.clone())
                    .expect("recreate Metal surface");
                self.resize(self.window.inner_size());
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                #[cfg(feature = "harwood-benchmark")]
                {
                    self.harwood.interrupt_benchmark("SURFACE VALIDATION ERROR");
                    eprintln!("harwood-benchmark: surface validation failed");
                    return;
                }
                #[cfg(not(feature = "harwood-benchmark"))]
                panic!("surface acquisition validation error")
            }
        };
        #[cfg(feature = "harwood-benchmark")]
        let cpu_started = Instant::now();
        #[cfg(not(any(
            feature = "rerbr-3d",
            feature = "harwood-block",
            feature = "harwood-neighbors"
        )))]
        {
            let seconds = self.started.elapsed().as_secs_f32();
            let angle = seconds * 0.7;
            let values = [
                angle.cos(),
                angle.sin(),
                self.config.width as f32 / self.config.height as f32,
                0.85 + 0.15 * seconds.sin(),
            ];
            let mut bytes = [0u8; 16];
            for (out, value) in bytes.chunks_exact_mut(4).zip(values) {
                out.copy_from_slice(&value.to_ne_bytes());
            }
            self.queue.write_buffer(&self.uniform, 0, &bytes);
        }
        let view = frame.texture.create_view(&Default::default());
        let background = if self.alternate_background {
            wgpu::Color {
                r: 0.10,
                g: 0.025,
                b: 0.04,
                a: 1.0,
            }
        } else {
            wgpu::Color {
                r: 0.015,
                g: 0.035,
                b: 0.08,
                a: 1.0,
            }
        };
        let mut encoder = self.device.create_command_encoder(&Default::default());
        #[cfg(any(feature = "harwood-block", feature = "harwood-neighbors"))]
        self.harwood.draw(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            (self.config.width, self.config.height),
            self.config.format,
            background,
        );
        #[cfg(all(
            feature = "rerbr-3d",
            not(any(feature = "harwood-block", feature = "harwood-neighbors"))
        ))]
        self.scene.draw(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            (self.config.width, self.config.height),
            background,
        );
        #[cfg(not(any(
            feature = "rerbr-3d",
            feature = "harwood-block",
            feature = "harwood-neighbors"
        )))]
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("probe frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(background),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        #[cfg(all(feature = "physics-probe", not(feature = "physics-drive")))]
        self.physics.draw(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            (self.config.width, self.config.height),
            self.config.format,
        );
        let commands = encoder.finish();
        #[cfg(feature = "harwood-benchmark")]
        let cpu_preparation = cpu_started.elapsed();
        self.queue.submit([commands]);
        self.window.pre_present_notify();
        self.queue.present(frame);
        #[cfg(any(feature = "harwood-block", feature = "harwood-neighbors"))]
        self.harwood.finish_frame();
        #[cfg(feature = "harwood-benchmark")]
        self.harwood.benchmark_submitted(cpu_preparation);
        self.frames += 1;
        if self.frames == 1 || self.frames.is_multiple_of(600) {
            eprintln!(
                "presented frame {} (Metal, {}x{})",
                self.frames, self.config.width, self.config.height
            );
        }
    }
}
