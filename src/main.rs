mod bind_groups;
mod camera;
mod mesh;
mod render_scene;
mod texture;

/// The maximum amount of draw calls expected. Decides the size of the draw commands buffer
/// (and will in the future simply indicate the maximum expected draw count).
const MAX_DRAW_COMMANDS: usize = 100;

use crate::{
    mesh::{Vertex, VertexArrayBuffer},
    render_scene::{DrawOutputInfo, RenderObjectDescriptor},
};
use macaw as m;
use penguin_util::{
    handle::Handle, raw_gpu_types::DrawIndirectCount, GpuBuffer, GpuBufferDeviceExt,
};
use std::{iter, mem, slice};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct RenderInstance {
    pub render_object_id: Handle<render_scene::RenderObject>,
}
unsafe impl bytemuck::Pod for RenderInstance {}
unsafe impl bytemuck::Zeroable for RenderInstance {}

impl RenderInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![
        5 => Uint32,
    ];

    fn buffer_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as _,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

/// Temporary variable that increases with a value each frame.
static mut TIME_STATE: f32 = 0.0_f32;

/// Data related to a compute pass.
struct Compute {
    pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
}

/// Data related to a render pass.
struct Render {
    pipeline: wgpu::RenderPipeline,
    vertex_shader_bind_group: wgpu::BindGroup,
    fragment_shader_bind_group: wgpu::BindGroup,
}

/// Objects registered in the scene (temporary storage).
struct SceneObjects {
    cube_object: Handle<render_scene::RenderObject>,
    cube_object2: Handle<render_scene::RenderObject>,
    cone_object: Handle<render_scene::RenderObject>,
    cone_object2: Handle<render_scene::RenderObject>,
}

/// State with data necessary to render.
struct RendererState {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// Window size excluding the window's borders and title bar.
    size: winit::dpi::PhysicalSize<u32>,
    /// Compute pass data.
    compute: Compute,
    /// Render pass data.
    render: Render,
    // A texture.
    _cube_texture: texture::Texture,
    /// The depth texture.
    depth_texture: texture::Texture,
    /// Editor camera data.
    camera: camera::EditorCamera,
    /// Uniform buffer.
    uniform_buffer: GpuBuffer<camera::CameraUniform>,
    /// Indicates weather the mouse left mouse button is held down.
    mouse_pressed: bool,
    /// The currently loaded RenderScene.
    scene: render_scene::RenderScene,
    /// CPU storage for handles to RenderObjects in the RenderScene.
    scene_objects: SceneObjects,
}

/// Helper struct when creating texture-related data.
struct Textures {
    bind_group_layout: wgpu::BindGroupLayout,
    //
    cube_texture: texture::Texture,
    cube_texture_bind_group: wgpu::BindGroup,
}
impl RendererState {
    fn init_textures(device: &wgpu::Device, queue: &wgpu::Queue) -> Textures {
        let cube_texture = texture::Texture::from_asset(device, queue, "cube-diffuse.jpg").unwrap();

        use bind_groups::layout_entry;

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture bind group layout"),
                entries: &[
                    layout_entry::texture::texture_2d(0, wgpu::ShaderStages::FRAGMENT),
                    layout_entry::texture::sampler(1, wgpu::ShaderStages::FRAGMENT),
                ],
            });

        let cube_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cube diffuse bind group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&cube_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&cube_texture.sampler),
                },
            ],
        });

        Textures {
            bind_group_layout: texture_bind_group_layout,
            cube_texture,
            cube_texture_bind_group,
        }
    }
}

impl RendererState {
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
                    features:
                    //wgpu::Features::default(), // wgpu::Features::BUFFER_BINDING_ARRAY,
                    //wgpu::Features::default(), // wgpu::Features::BUFFER_BINDING_ARRAY,
                    // wgpu::Features::POLYGON_MODE_LINE |
                    // allow non-zero value for first_instance field in draw calls
                    //wgpu::Features::INDIRECT_FIRST_INSTANCE |
                    //wgpu::Features::TEXTURE_BINDING_ARRAY |
                    // wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY,
                    wgpu::Features::all() ^ wgpu::Features::TEXTURE_COMPRESSION_ETC2 ^ wgpu::Features::TEXTURE_COMPRESSION_ASTC_LDR ^ wgpu::Features::VERTEX_ATTRIBUTE_64BIT,
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
            present_mode: wgpu::PresentMode::Mailbox,
        };

        let Textures {
            bind_group_layout: texture_bind_group_layout,
            cube_texture,
            cube_texture_bind_group,
        } = Self::init_textures(&device, &queue);

        let depth_texture = texture::Texture::create_depth_texture(&device, &config);

        // ------------

        let mut scene = render_scene::RenderScene::new(&device, &["cube.obj", "cone.obj"]);

        let cube_object = scene.register_object(&RenderObjectDescriptor {
            mesh_id: 0,
            transform: m::Mat4::IDENTITY,
            render_bounds: mesh::RenderBounds {
                origin: m::Vec3::ZERO,
                radius: 3.0,
            },
            draw_forward_pass: true,
        });

        let cube_object2 = scene.register_object(&RenderObjectDescriptor {
            mesh_id: 0,
            transform: m::Mat4::IDENTITY,
            render_bounds: mesh::RenderBounds {
                origin: m::Vec3::ZERO,
                radius: 3.0,
            },
            draw_forward_pass: true,
        });

        let cone_object = scene.register_object(&RenderObjectDescriptor {
            mesh_id: 1,
            transform: m::Mat4::IDENTITY,
            render_bounds: mesh::RenderBounds {
                origin: m::Vec3::ZERO,
                radius: 3.0,
            },
            draw_forward_pass: true,
        });

        let cone_object2 = scene.register_object(&RenderObjectDescriptor {
            mesh_id: 1,
            transform: m::Mat4::IDENTITY,
            render_bounds: mesh::RenderBounds {
                origin: m::Vec3::ZERO,
                radius: 3.0,
            },
            draw_forward_pass: true,
        });
        scene.build_batches(&queue);

        let camera = camera::EditorCamera::init(&config);

        let uniform_buffer = device.create_buffer_init_t::<camera::CameraUniform>(
            &wgpu::util::BufferInitDescriptor {
                label: Some("camera uniform buffer"),
                contents: bytemuck::cast_slice(slice::from_ref(&camera.uniform_data)),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            },
        );

        let vertex_shader_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera bind group"),
                entries: &[
                    // camera uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // render objects
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // instance_index to render_object map
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &vertex_shader_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: scene.render_objects_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: scene
                        .instance_index_to_render_object_map
                        .as_entire_binding(),
                },
            ],
        });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/vert_frag.wgsl").into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render pipeline layout"),
                bind_group_layouts: &[
                    &vertex_shader_bind_group_layout, // group 0
                    &texture_bind_group_layout,       // group 1
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    mesh::MeshVertex::buffer_layout(),
                    RenderInstance::buffer_layout(),
                ],
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
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

        let render = Render {
            pipeline: render_pipeline,
            vertex_shader_bind_group: camera_bind_group,
            fragment_shader_bind_group: cube_texture_bind_group,
        };

        let compute_shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("compute shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/compute.wgsl").into()),
        });

        let compute_bind_group_layout = device
            .create_bind_group_layout(&render_scene::compute_pipeline::BIND_GROUP_LAYOUT_DESC);

        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compute bind group"),
            layout: &compute_bind_group_layout,
            entries: &render_scene::compute_pipeline::bind_group_entries(&uniform_buffer, &scene),
        });

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("compute pipeline layout"),
                bind_group_layouts: &[&compute_bind_group_layout],
                push_constant_ranges: &[],
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("compute pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: "cs_main",
        });

        let compute = Compute {
            pipeline: compute_pipeline,
            bind_group: compute_bind_group,
        };

        Self {
            surface,
            device,
            queue,
            config,
            size,
            compute,
            render,
            _cube_texture: cube_texture,
            depth_texture,
            camera,
            uniform_buffer,
            mouse_pressed: false,
            scene,
            scene_objects: SceneObjects {
                cube_object,
                cube_object2,
                cone_object,
                cone_object2,
            },
        }
    }

    /// Called when the window gets resized.
    fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        assert_ne!(size.width, 0);
        assert_ne!(size.height, 0);

        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);

        self.depth_texture = texture::Texture::create_depth_texture(&self.device, &self.config);
        self.camera
            .projection
            .resize((self.config.width, self.config.height));
    }

    /// Called on input events.
    fn input(&mut self, event: &DeviceEvent) -> bool {
        match event {
            //
            DeviceEvent::Key(KeyboardInput {
                virtual_keycode: Some(key),
                state,
                ..
            }) => self.camera.controller.process_key_events(*key, *state),
            //
            DeviceEvent::Button {
                button: 1, // left mouse button
                state,
            } => {
                self.mouse_pressed = *state == ElementState::Pressed;
                true
            }
            DeviceEvent::MouseMotion { delta } => {
                if self.mouse_pressed {
                    self.camera
                        .controller
                        .process_mouse_delta_events(delta.0, delta.1);
                }
                true
            }
            //
            _ => false,
        }
    }

    /// Called each frame.
    fn update(&mut self, dt: std::time::Duration) {
        // update camera data
        self.camera.update(dt);

        // schedule uniform buffer write
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(slice::from_ref(&self.camera.uniform_data)),
        );

        let (x, y) = unsafe {
            TIME_STATE += dt.as_secs_f32() * 2.;
            (f32::cos(TIME_STATE), f32::sin(TIME_STATE))
        };

        self.scene.update_transform(
            self.scene_objects.cube_object,
            m::Mat4::from_translation(m::vec3(x, y, 0.)),
        );

        self.scene.update_transform(
            self.scene_objects.cube_object2,
            m::Mat4::from_translation(m::vec3(1., 4., y)),
        );

        self.scene.update_transform(
            self.scene_objects.cone_object,
            m::Mat4::from_translation(m::vec3(3.1, 4. + y, 0.)),
        );

        self.scene.update_transform(
            self.scene_objects.cone_object2,
            m::Mat4::from_rotation_translation(
                m::Quat::from_rotation_z(f32::to_radians(180.)),
                m::vec3(3.1, -y, 0.),
            ),
        );

        // update scene
        self.scene.update(&self.queue);
    }

    /// Submits compute commands.
    fn prepare(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        // compute commands
        let mut cmd = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("compute commands encoder"),
        });
        cmd.push_debug_group("compute pass");
        {
            // clear local compute commands buffer
            cmd.copy_buffer_to_buffer(
                &self.scene.clear_compute_shader_local_data_buffer,
                0,
                &self.scene.compute_shader_local_data_buffer,
                0,
                (MAX_DRAW_COMMANDS * std::mem::size_of::<DrawOutputInfo>()) as _,
            );

            // clear draw count buffer
            cmd.copy_buffer_to_buffer(
                &self.scene.clear_draw_count_buffer,
                0,
                &self.scene.draw_count_buffer,
                0,
                std::mem::size_of::<DrawIndirectCount>() as _,
            );

            let mut compute_pass = cmd.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compute pass"),
            });
            compute_pass.set_pipeline(&self.compute.pipeline);
            compute_pass.set_bind_group(0, &self.compute.bind_group, &[]);
            compute_pass.dispatch(self.scene.render_objects.inner.len() as _, 1, 1);
        }
        cmd.pop_debug_group();

        queue.submit(Some(cmd.finish()));
    }

    /// Submits render commands.
    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // submit compute commands
        self.prepare(&self.device, &self.queue);

        // get frame surface texture to render to
        let output_texture = self.surface.get_current_texture()?;
        let output_texture_view = output_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // render commands
        let mut cmd = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render commands encoder"),
            });

        cmd.push_debug_group("render pass");
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
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            // set render pipeline
            render_pass.set_pipeline(&self.render.pipeline);

            // set bind groups
            render_pass.set_bind_group(0, &self.render.vertex_shader_bind_group, &[]);
            render_pass.set_bind_group(1, &self.render.fragment_shader_bind_group, &[]);

            // set vertex/index buffer
            render_pass.set_vertex_buffer(0, self.scene.vertex_array_buffer.vertices_slice());
            render_pass.set_index_buffer(
                self.scene.vertex_array_buffer.indices_slice(),
                wgpu::IndexFormat::Uint32,
            );
            // set instance buffer
            render_pass.set_vertex_buffer(1, self.scene.instance_buffer.slice(..));

            // draw
            render_pass.multi_draw_indexed_indirect_count(
                &self.scene.out_draw_commands_buffer,
                0,
                &self.scene.draw_count_buffer,
                0,
                self.scene.max_draw_count as _,
            );
        }
        cmd.pop_debug_group();

        self.queue.submit(iter::once(cmd.finish()));
        output_texture.present();

        Ok(())
    }
}

/// Entry point.
fn main() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Penguin engine")
        .build(&event_loop)
        .unwrap();

    let mut state = pollster::block_on(RendererState::new(&window));

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
                    println!("Surface lost. Reconfiguring");
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
