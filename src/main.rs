use std::sync::Arc;

use display_info::DisplayInfo;
use wgpu::{Instance, Surface};
use winit::application::ApplicationHandler;
use winit::event::{KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    // an instance of WGPU API
    instance: Option<Instance>,
    // surface for drawing
    surface: Option<Surface<'static>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
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
                        // .with_decorations(false)
                        .with_inner_size(winit::dpi::LogicalSize::new(WIDTH, HEIGHT))
                        .with_position(winit::dpi::LogicalPosition::new(x, y))
                        .with_transparent(true),
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

        let size = self.window.as_ref().unwrap().inner_size();
        self.surface.as_ref().unwrap().configure(
            &self.device.as_ref().unwrap(),
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                // not really sure what the TextureFormat is
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                width: size.width,
                height: size.height,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 1,
                alpha_mode: wgpu::CompositeAlphaMode::PostMultiplied,
                // alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
            },
        );

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
                println!("Redraw requested");
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
                    println!("render pass");
                    let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 1.0,
                                    g: 0.8,
                                    b: 1.0,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                }

                // submit will accept anything that implements IntoIter
                self.queue
                    .as_ref()
                    .unwrap()
                    .submit(std::iter::once(encoder.finish()));
                output.present();
                // println!("{:#?}", &output.texture);

                // self.instance.g

                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                // Draw.

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                // self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
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
