use cgmath::Rotation3;
use light::LightUniform;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

mod camera;
mod camera_controller;
mod instance;
mod light;
mod model;
mod render_pipeline;
mod resources;
mod texture;

use model::{DrawModel, Vertex};

const NUM_INSTANCES_PER_ROW: u32 = 10;

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    obj_model: model::Model,
    camera: camera::Camera,
    camera_controller: camera_controller::CameraController,
    camera_uniform: camera::CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    instances: Vec<instance::Instance>,
    #[allow(dead_code)]
    instance_buffer: wgpu::Buffer,
    depth_texture: texture::Texture,
    light_uniform: LightUniform,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    light_render_pipeline: wgpu::RenderPipeline,
}

impl State {
    async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        log::warn!("WGPU setup");
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        log::warn!("device and queue");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                },
                // Some(&std::path::Path::new("trace")), // Trace path
                None, // Trace path
            )
            .await
            .unwrap();

        log::warn!("Surface");
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_preferred_format(&adapter).unwrap(),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };

        surface.configure(&device, &config);

        let texture_bind_group_layout = texture::Texture::create_bind_group_layout(&device);

        let camera = camera::Camera::new(&config);
        let camera_controller = camera_controller::CameraController::new(0.2);

        let mut camera_uniform = camera::CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer = camera::Camera::create_buffer_init(&device, camera_uniform);

        const SPACE_BETWEEN: f32 = 3.0;
        let instances = instance::Instance::instance_vec(NUM_INSTANCES_PER_ROW, SPACE_BETWEEN);

        let instance_data = instances
            .iter()
            .map(instance::Instance::to_raw)
            .collect::<Vec<_>>();

        let instance_buffer = instance::create_buffer_init(&device, instance_data);

        let camera_bind_group_layout = camera::Camera::camera_bind_group_layout(&device);

        let camera_bind_group =
            camera::Camera::create_bind_group(&device, &camera_bind_group_layout, &camera_buffer);

        log::warn!("Load model");
        let obj_model =
            resources::load_model("cube.obj", &device, &queue, &texture_bind_group_layout)
                .await
                .unwrap();

        let light_uniform = light::LightUniform::new();

        let light_buffer = light::create_buffer_init(&device, light_uniform);

        let light_bind_group_layout = light::create_bind_group_layout(&device);

        let light_bind_group =
            light::create_bind_group(&device, &light_bind_group_layout, &light_buffer);

        let depth_texture =
            texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        let shader = wgpu::ShaderModuleDescriptor {
            label: Some("Normal Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader_a.wgsl").into()),
        };

        let render_pipeline_layout = render_pipeline::RenderPipeline::create_pipeline_layout(
            &device,
            &[
                &texture_bind_group_layout,
                &camera_bind_group_layout,
                &light_bind_group_layout,
            ],
        );

        let render_pipeline = render_pipeline::RenderPipeline::new(
            &device,
            &render_pipeline_layout,
            config.format,
            Some(texture::Texture::DEPTH_FORMAT),
            &[model::ModelVertex::desc(), instance::InstanceRaw::desc()],
            shader,
        )
        .render_pipeline;

        let light_layout = render_pipeline::RenderPipeline::create_pipeline_layout(
            &device,
            &[&camera_bind_group_layout, &light_bind_group_layout],
        );

        let light_shader = wgpu::ShaderModuleDescriptor {
            label: Some("Light Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.wgsl").into()),
        };

        let light_render_pipeline = render_pipeline::RenderPipeline::new(
            &device,
            &light_layout,
            config.format,
            Some(texture::Texture::DEPTH_FORMAT),
            &[model::ModelVertex::desc()],
            light_shader,
        )
        .render_pipeline;

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            obj_model,
            camera,
            camera_controller,
            camera_buffer,
            camera_bind_group,
            camera_uniform,
            instances,
            instance_buffer,
            depth_texture,
            light_bind_group,
            light_buffer,
            light_uniform,
            light_render_pipeline,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.camera.aspect = self.config.width as f32 / self.config.height as f32;
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture =
                texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
        }
    }
    fn input(&mut self, event: &WindowEvent) -> bool {
        self.camera_controller.process_events(event)
    }

    fn update(&mut self) {
        self.camera_controller.update_camera(&mut self.camera);
        self.camera_uniform.update_view_proj(&self.camera);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        let old_position: cgmath::Vector3<_> = self.light_uniform.position.into();
        self.light_uniform.position =
            (cgmath::Quaternion::from_axis_angle((0.0, 1.0, 0.0).into(), cgmath::Deg(1.0))
                * old_position)
                .into();
        self.queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::cast_slice(&[self.light_uniform]),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

            use crate::model::DrawLight;
            render_pass.set_pipeline(&self.light_render_pipeline);
            render_pass.draw_light_model(
                &self.obj_model,
                &self.camera_bind_group,
                &self.light_bind_group,
            );

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.draw_model_instanced(
                &self.obj_model,
                0..self.instances.len() as u32,
                &self.camera_bind_group,
                &self.light_bind_group,
            );
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub async fn run() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let title = env!("CARGO_PKG_NAME");
    let window = winit::window::WindowBuilder::new()
        .with_title(title)
        .build(&event_loop)
        .unwrap();

    // State::new uses async code, so we're going to wait for it to finish
    let mut state = State::new(&window).await;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::MainEventsCleared => window.request_redraw(),
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                if !state.input(event) {
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    state: ElementState::Pressed,
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    ..
                                },
                            ..
                        } => *control_flow = ControlFlow::Exit,
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            state.resize(**new_inner_size);
                        }
                        _ => {}
                    }
                }
            }
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if it's lost or outdated
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        state.resize(state.size)
                    }
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // We're ignoring timeouts
                    Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                }
            }
            _ => {}
        }
    });
}
