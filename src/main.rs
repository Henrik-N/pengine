mod camera;
mod texture;

use macaw as ma;
use std::os::linux::raw::stat;
use std::{iter, mem, slice};
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}
impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x2,
    ];

    fn buffer_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[rustfmt::skip]
const VERTICES: &[Vertex] = &[
    Vertex { position: [-0.0868241, 0.49240386, 0.0], tex_coords: [0.4131759, 1.0 - 0.99240386] },
    Vertex { position: [-0.49513406, 0.06958647, 0.0], tex_coords: [0.0048659444, 1.0 - 0.56958647] },
    Vertex { position: [-0.21918549, -0.44939706, 0.0], tex_coords: [0.28081453, 1.0 - 0.05060294] },
    Vertex { position: [0.35966998, -0.3473291, 0.0], tex_coords: [0.85967, 1.0 - 0.1526709] },
    Vertex { position: [0.44147372, 0.2347359, 0.0], tex_coords: [0.9414737, 1.0 - 0.7347359] },
];

const INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4];

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    // pipeline
    render_pipeline: wgpu::RenderPipeline,
    // mesh
    vertex_array_buffer: wgpu::Buffer,
    vertex_array_buffer_indices_offset: usize, // in bytes
    indices_count: usize,
    // texture
    diffuse_bind_group: wgpu::BindGroup,
    diffuse_texture: texture::Texture,
    // camera
    camera: camera::Camera,
    projection: camera::PerspectiveProjection,
    camera_controller: camera::CameraController,
    camera_uniform: camera::CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    //
    mouse_pressed: bool,
}
impl State {
    async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::Backends::VULKAN);
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("no supported gpu");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::default(), // wgpu::Features::BUFFER_BINDING_ARRAY,
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .expect("failed to init device, missing required features?");

        assert_ne!(size.width, 0);
        assert_ne!(size.height, 0);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_preferred_format(&adapter).unwrap(),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo, // todo check supported present modes
        };

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
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

        let diffuse_texture = texture::Texture::from_bytes(
            &device,
            &queue,
            include_bytes!("tree.png"),
            Some("diffuse texture"),
        )
        .unwrap();

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("diffuse bind group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
        });

        let camera = camera::Camera::new(
            (0.0, 5.0, 10.0).into(),
            f32::to_radians(-90.),
            f32::to_radians(-20.),
        );
        let camera_controller = camera::CameraController::new(4.0, 50.0);

        let projection = camera::PerspectiveProjection {
            fov_y: f32::to_radians(45.0),
            aspect: config.width as f32 / config.height as f32,
            z_near: 0.1,
            z_far: 100.0,
        };
        let mut camera_uniform = camera::CameraUniform::new();
        camera_uniform.update_view_proj(&camera, &projection);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera uniform buffer"),
            contents: bytemuck::cast_slice(slice::from_ref(&camera_uniform)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera bind group"),
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

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // ----------

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render pipeline layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::buffer_layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,                         // all
                alpha_to_coverage_enabled: false, // related to anti-aliasing
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            multiview: None, // related to rendering to array textures
        });

        let vert_bytes: &[u8] = bytemuck::cast_slice(VERTICES);
        let ind_bytes: &[u8] = bytemuck::cast_slice(INDICES);
        let vertex_array_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex array buffer"),
            contents: &[vert_bytes, ind_bytes].concat(),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
        });
        let vertex_array_buffer_indices_offset = vert_bytes.len();
        let indices_count = INDICES.len();

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_array_buffer,
            vertex_array_buffer_indices_offset,
            indices_count,
            diffuse_bind_group,
            diffuse_texture,
            camera,
            projection,
            camera_controller,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            mouse_pressed: false,
        }
    }

    fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        assert_ne!(size.width, 0);
        assert_ne!(size.height, 0);

        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);

        self.projection.resize(size.into());
    }

    fn input(&mut self, event: &DeviceEvent) -> bool {
        match event {
            //
            DeviceEvent::Key(KeyboardInput {
                virtual_keycode: Some(key),
                state,
                ..
            }) => self.camera_controller.process_key_events(*key, *state),
            //
            DeviceEvent::Button {
                button: 1, // left mouse button
                state,
            } => {
                self.mouse_pressed = *state == ElementState::Pressed;
                true
            }
            //
            DeviceEvent::MouseMotion { delta } => {
                if self.mouse_pressed {
                    self.camera_controller
                        .process_mouse_delta_events(delta.0, delta.1);
                }
                true
            }
            //
            _ => false,
        }
    }

    fn update(&mut self, dt: std::time::Duration) {
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.camera_uniform
            .update_view_proj(&self.camera, &self.projection);

        // todo use a staging buffer instead of writing directly
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(slice::from_ref(&self.camera_uniform)),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // get frame surface texture to render to
        let output_texture = self.surface.get_current_texture()?;
        let output_texture_view = output_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut cmd = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render commands encoder"),
            });

        {
            let mut render_pass = cmd.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &output_texture_view,
                    // the texture that will receive the resolved output (used for multisampling)
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        // store rendered results to output texture
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);

            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.diffuse_bind_group, &[]);

            render_pass.set_vertex_buffer(
                0,
                self.vertex_array_buffer
                    .slice(0..(self.vertex_array_buffer_indices_offset as u64)),
            );
            render_pass.set_index_buffer(
                self.vertex_array_buffer
                    .slice((self.vertex_array_buffer_indices_offset as u64)..),
                wgpu::IndexFormat::Uint16,
            );

            render_pass.draw_indexed(0..(self.indices_count as u32), 0, 0..1);
        }

        self.queue.submit(iter::once(cmd.finish()));
        output_texture.present();

        Ok(())
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Penguin engine")
        .build(&event_loop)
        .unwrap();

    let mut state = pollster::block_on(State::new(&window));

    let mut last_render_time = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| match event {
        //
        Event::DeviceEvent { ref event, .. } => {
            state.input(event);
        }
        //
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => {
            match event {
                //
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
                //
                WindowEvent::Resized(physical_size) => {
                    state.resize(*physical_size);
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    state.resize(**new_inner_size);
                }
                _ => {}
            }
        }
        //
        Event::MainEventsCleared => {
            window.request_redraw();
        }
        //
        Event::RedrawRequested(window_id) if window_id == window.id() => {
            let now = std::time::Instant::now();
            let dt = now - last_render_time;
            last_render_time = now;
            state.update(dt);

            match state.render() {
                Ok(_) => {}
                Err(wgpu::SurfaceError::Lost) => {
                    println!("Device lost. Reconfiguring");
                    state.resize(state.size);
                }
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    eprintln!("Out of memory. Exiting");
                    *control_flow = ControlFlow::Exit;
                }
                Err(e) => eprintln!("Surface error: {:?}", e),
            }
        }
        //
        _ => {}
    });
}
