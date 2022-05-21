use cgmath::Rotation3;
use light::LightUniform;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

mod camera;
mod instance;
mod light;
mod model;
mod context;
mod renderer;
mod resources;
mod texture;

use model::Vertex;

const NUM_INSTANCES_PER_ROW: u32 = 10;

pub struct State {
    context: context::Context,
    render_pipeline: wgpu::RenderPipeline,
    obj_model: model::Model,
    camera: camera::Camera,
    projection: camera::Projection,
    camera_controller: camera::CameraController,
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
    #[allow(dead_code)]
    debug_material: model::Material,
    use_debug: bool,
    mouse_pressed: bool,
}

impl State {
    async fn new(window: &Window) -> Self {
        let context = context::Context::new(window).await;

        let texture_bind_group_layout = texture::Texture::create_bind_group_layout(&context.device);

        let camera = camera::Camera::new((0.0, 5.0, 10.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));
        let projection =
            camera::Projection::new(context.config.width, context.config.height, cgmath::Deg(45.0), 0.1, 100.0);
        let camera_controller = camera::CameraController::new(4.0, 0.4);

        let mut camera_uniform = camera::CameraUniform::new();
        camera_uniform.update_view_proj(&camera, &projection);

        let camera_buffer = camera::Camera::create_buffer_init(&context.device, camera_uniform);

        const SPACE_BETWEEN: f32 = 3.0;
        let instances = instance::Instance::instance_vec(NUM_INSTANCES_PER_ROW, SPACE_BETWEEN);

        let instance_data = instances
            .iter()
            .map(instance::Instance::to_raw)
            .collect::<Vec<_>>();

        let instance_buffer = instance::create_buffer_init(&context.device, instance_data);

        let camera_bind_group_layout = camera::Camera::camera_bind_group_layout(&context.device);

        let camera_bind_group =
            camera::Camera::create_bind_group(&context.device, &camera_bind_group_layout, &camera_buffer);

        log::warn!("Load model");
        let obj_model =
            resources::load_model("cube.obj", &context.device, &context.queue, &texture_bind_group_layout)
                .await
                .unwrap();

        let light_uniform = light::LightUniform::new();

        let light_buffer = light::create_buffer_init(&context.device, light_uniform);

        let light_bind_group_layout = light::create_bind_group_layout(&context.device);

        let light_bind_group =
            light::create_bind_group(&context.device, &light_bind_group_layout, &light_buffer);

        let depth_texture =
            texture::Texture::create_depth_texture(&context.device, &context.config, "depth_texture");

        let shader = wgpu::ShaderModuleDescriptor {
            label: Some("Normal Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader_a.wgsl").into()),
        };

        let render_pipeline_layout = renderer::RenderPipeline::create_pipeline_layout(
            &context.device,
            &[
                &texture_bind_group_layout,
                &camera_bind_group_layout,
                &light_bind_group_layout,
            ],
        );

        let render_pipeline = renderer::RenderPipeline::new(
            &context.device,
            &render_pipeline_layout,
            context.config.format,
            Some(texture::Texture::DEPTH_FORMAT),
            &[model::ModelVertex::desc(), instance::InstanceRaw::desc()],
            shader,
        )
        .render_pipeline;

        let light_layout = renderer::RenderPipeline::create_pipeline_layout(
            &context.device,
            &[&camera_bind_group_layout, &light_bind_group_layout],
        );

        let light_shader = wgpu::ShaderModuleDescriptor {
            label: Some("Light Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.wgsl").into()),
        };

        let light_render_pipeline = renderer::RenderPipeline::new(
            &context.device,
            &light_layout,
            context.config.format,
            Some(texture::Texture::DEPTH_FORMAT),
            &[model::ModelVertex::desc()],
            light_shader,
        )
        .render_pipeline;

        let debug_material = {
            let diffuse_bytes = include_bytes!("../res/cobble-diffuse.png");
            let normal_bytes = include_bytes!("../res/cobble-normal.png");

            let diffuse_texture = texture::Texture::from_bytes(
                &context.device,
                &context.queue,
                diffuse_bytes,
                "res/alt-diffuse.png",
                false,
            )
            .unwrap();
            let normal_texture = texture::Texture::from_bytes(
                &context.device,
                &context.queue,
                normal_bytes,
                "res/alt-normal.png",
                true,
            )
            .unwrap();

            model::Material::new(
                &context.device,
                "alt-material",
                diffuse_texture,
                normal_texture,
                &texture_bind_group_layout,
            )
        };

        Self {
            context,
            render_pipeline,
            obj_model,
            camera,
            projection,
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
            debug_material,
            use_debug: false,
            mouse_pressed: false,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.projection.resize(new_size.width, new_size.height);
            self.context.size = new_size;
            self.context.config.width = new_size.width;
            self.context.config.height = new_size.height;
            self.context.surface.configure(&self.context.device, &self.context.config);
            self.depth_texture =
                texture::Texture::create_depth_texture(&self.context.device, &self.context.config, "depth_texture");
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state,
                        virtual_keycode: Some(VirtualKeyCode::Space),
                        ..
                    },
                ..
            } => {
                self.use_debug = *state == ElementState::Pressed;
                true
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(key),
                        state,
                        ..
                    },
                ..
            } => self.camera_controller.process_keyboard(*key, *state),
            WindowEvent::MouseWheel { delta, .. } => {
                self.camera_controller.process_scroll(delta);
                true
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                self.mouse_pressed = *state == ElementState::Pressed;
                true
            }
            _ => false,
        }
    }

    fn update(&mut self, dt: std::time::Duration) {
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.camera_uniform
            .update_view_proj(&self.camera, &self.projection);
        self.context.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        let old_position: cgmath::Vector3<_> = self.light_uniform.position.into();
        self.light_uniform.position =
            (cgmath::Quaternion::from_axis_angle((0.0, 1.0, 0.0).into(), cgmath::Deg(1.0))
                * old_position)
                .into();
        self.context.queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::cast_slice(&[self.light_uniform]),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        renderer::render(self)
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
    let mut last_render_time = instant::Instant::now();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::MainEventsCleared => window.request_redraw(),

            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion{ delta, },
                .. // We're not using device_id currently
            } => if state.mouse_pressed {
                state.camera_controller.process_mouse(delta.0, delta.1)
            }

            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() && !state.input(event) => {
                match event {
                    #[cfg(not(target_arch="wasm32"))]
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

            Event::RedrawRequested(window_id) if window_id == window.id() => {
                let now = instant::Instant::now();
                let dt = now - last_render_time;
                last_render_time = now;
                state.update(dt);
                match state.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if it's lost or outdated
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => state.resize(state.context.size),
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
