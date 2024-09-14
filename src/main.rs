use std::sync::Arc;
use std::time::Duration;

use display_info::DisplayInfo;
use wgpu::util::DeviceExt;
use wgpu::{Instance, Surface};
use winit::application::ApplicationHandler;
use winit::event::{KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId, WindowLevel};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}
impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [1.0, 1.0, 0.0],
        color: [1.0, 0.0, 0.0],
    },
    Vertex {
        position: [-1.0, 1.0, 0.0],
        color: [0.0, 1.0, 0.0],
    },
    Vertex {
        position: [-1.0, -1.0, 0.0],
        color: [0.0, 0.0, 1.0],
    },
    Vertex {
        position: [1.0, -1.0, 0.0],
        color: [0.4, 0.4, 0.1],
    },
];

const INDICES: &[u16] = &[0, 1, 2, 2, 3, 0];

#[repr(C)]
#[derive(Debug, Copy, Clone)]
// bytemuck::Pod, bytemuck::Zeroable)]
struct TimerUniform {
    t: f32,
}
#[repr(C)]
struct Timer {
    start: std::time::Instant,
    elapsed: f64,
    last: f64,
    acc: f64,
    timer_uniform: TimerUniform,
    timer_buffer: wgpu::Buffer,
    timer_bind_group: wgpu::BindGroup,
    timer_bind_group_layout: wgpu::BindGroupLayout,
}
impl Timer {
    fn new(device: &wgpu::Device) -> Self {
        let mut timer_uniform = TimerUniform { t: 0.2 };
        let timer_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Timer Buffer"),
            contents: &timer_uniform.t.to_le_bytes(),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let timer_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_for_timer_uniform"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },

                    count: None,
                }],
            });

        let timer_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &timer_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: timer_buffer.as_entire_binding(),
            }],
        });

        let start = std::time::Instant::now();

        Timer {
            start,
            elapsed: 0.0,
            last: 0.0,
            acc: 0.0f64,
            timer_uniform,
            timer_buffer,
            timer_bind_group,
            timer_bind_group_layout,
        }
    }
}

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    // an instance of WGPU API
    instance: Option<Instance>,
    // surface for drawing
    surface: Option<Surface<'static>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,

    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    timer: Option<Timer>,

    render_pipeline: Option<wgpu::RenderPipeline>,
}

struct Pipeline {
    render_pipeline: wgpu::RenderPipeline,
}

const WIDTH: u32 = 256;
const HEIGHT: u32 = 128;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut x = 0;
        let mut y = 0;
        let display_infos = DisplayInfo::all().unwrap();
        for display_info in display_infos {
            if display_info.is_primary {
                x = display_info.width - WIDTH;
                y = display_info.height - HEIGHT;
                break;
            }
        }

        self.window = Some(Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_decorations(false)
                        .with_inner_size(winit::dpi::LogicalSize::new(WIDTH, HEIGHT))
                        .with_position(winit::dpi::LogicalPosition::new(x, y))
                        .with_transparent(true)
                        .with_window_level(WindowLevel::AlwaysOnTop),
                )
                .unwrap(),
        ));

        self.instance = Some(Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::empty(),
            ..Default::default()
        }));
        self.surface = Some(
            self.instance
                .as_ref()
                .unwrap()
                .create_surface(self.window.clone().unwrap())
                .unwrap(),
        );
        let adapter = pollster::block_on(self.instance.as_ref().unwrap().request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: self.surface.as_ref(),
                force_fallback_adapter: false,
            },
        ))
        .unwrap();
        let device_queue = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("device-descriptor"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
            None,
        ))
        .unwrap();

        self.device = Some(device_queue.0);
        self.queue = Some(device_queue.1);

        let texture_format = wgpu::TextureFormat::Bgra8UnormSrgb;

        let size = self.window.as_ref().unwrap().inner_size();
        self.surface.as_ref().unwrap().configure(
            &self.device.as_ref().unwrap(),
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                // not really sure what the TextureFormat is
                format: texture_format,
                width: size.width,
                height: size.height,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 1,
                alpha_mode: wgpu::CompositeAlphaMode::PostMultiplied,
                // alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
            },
        );

        /////// brush stuff
        let brush = wgpu_text::BrushBuilder::using_font_bytes(font).unwrap();

        //// uniform buffer
        self.timer = Some(Timer::new(self.device.as_ref().unwrap()));

        ///// shader time
        let shader =
            self.device
                .as_ref()
                .unwrap()
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
                });
        let render_pipeline_layout =
            self.device
                .as_ref()
                .unwrap()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &[&self.timer.as_ref().unwrap().timer_bind_group_layout],
                    push_constant_ranges: &[],
                });

        // vertex buffer
        self.vertex_buffer = Some(self.device.as_ref().unwrap().create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
        // index buffer
        self.index_buffer = Some(self.device.as_ref().unwrap().create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(INDICES),
                usage: wgpu::BufferUsages::INDEX,
            },
        ));
        let num_indices = INDICES.len() as u32;

        // render pipelinne
        self.render_pipeline = Some(self.device.as_ref().unwrap().create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: texture_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: wgpu::PolygonMode::Fill,
                    // Requires Features::DEPTH_CLIP_CONTROL
                    unclipped_depth: false,
                    // Requires Features::CONSERVATIVE_RASTERIZATION
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            },
        ));

        // initial redraw request
        self.window.as_ref().unwrap().request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: winit::event::ElementState::Pressed,
                        logical_key: Key::Named(NamedKey::Escape),
                        ..
                    },
                ..
            } => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.update();
                let output = self
                    .surface
                    .as_ref()
                    .unwrap()
                    .get_current_texture()
                    .unwrap();

                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = self.device.as_ref().unwrap().create_command_encoder(
                    &wgpu::CommandEncoderDescriptor {
                        label: Some("render encoder"),
                    },
                );

                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    render_pass.set_pipeline(&self.render_pipeline.as_ref().unwrap());
                    render_pass.set_bind_group(
                        0,
                        &self.timer.as_ref().unwrap().timer_bind_group,
                        &[],
                    );
                    render_pass
                        .set_vertex_buffer(0, self.vertex_buffer.as_ref().unwrap().slice(..));
                    render_pass.set_index_buffer(
                        self.index_buffer.as_ref().unwrap().slice(..),
                        wgpu::IndexFormat::Uint16,
                    ); // 1.
                    render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1); // 2.
                }

                // submit will accept anything that implements IntoIter
                self.queue
                    .as_ref()
                    .unwrap()
                    .submit(std::iter::once(encoder.finish()));
                output.present();
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}
impl App {
    fn update(&mut self) {
        match self.timer.as_mut() {
            Some(timer) => {
                let target_fps = 1.0 / 60.0 as f64;
                timer.elapsed = timer.start.elapsed().as_secs_f64();
                timer.acc += timer.elapsed - timer.last;
                timer.last = timer.elapsed;
                // framerate stuff goes here?
                timer.timer_uniform.t = timer.elapsed as f32;
                self.queue.as_ref().unwrap().write_buffer(
                    &timer.timer_buffer,
                    0,
                    &timer.timer_uniform.t.to_le_bytes(),
                );
            }
            None => {}
        };
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();

    // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
    // dispatched any events. This is ideal for games and similar applications.
    event_loop.set_control_flow(ControlFlow::Poll);

    // ControlFlow::Wait pauses the event loop if no events are available to process.
    // This is ideal for non-game applications that only update in response to user
    // input, and uses significantly less power/CPU time than ControlFlow::Poll.
    // event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    let _ = event_loop.run_app(&mut app);
}
