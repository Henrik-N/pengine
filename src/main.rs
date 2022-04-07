mod bind_groups;
mod camera;
mod components;
mod editor;
mod events;
mod graphics_context;
mod input;
mod mesh;
mod render_scene;
mod texture;
mod time;

use graphics_context::GraphicsContext;

/// The maximum amount of draw calls expected. Decides the size of the draw commands buffer
/// (and will in the future simply indicate the maximum expected draw count).
const MAX_DRAW_COMMANDS: usize = 100;

use crate::events::PenguinEvent;
use crate::input::InputEvent;
use crate::render_scene::RenderObject;
use crate::{
    mesh::{Vertex, VertexArrayBuffer},
    render_scene::{DrawOutputInfo, RenderObjectDescriptor},
};
use egui::Widget;
use image::imageops::colorops::contrast_in_place;
use macaw as m;
use penguin_util::{
    handle::Handle, raw_gpu_types::DrawIndirectCount, GpuBuffer, GpuBufferDeviceExt,
};
use std::any::Any;
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

struct ECSData {
    world: hecs::World,
    entities: Vec<hecs::Entity>,
    cube_entity: hecs::Entity,
}

/// State with data necessary to render.
pub struct RendererState {
    /// Compute pass data.
    compute: Compute,
    /// Render pass data.
    render: Render,
    // A texture.
    _cube_texture: texture::Texture,
    /// Editor camera data.
    camera: camera::EditorCamera,
    /// Uniform buffer.
    uniform_buffer: GpuBuffer<camera::CameraUniform>,
    /// The currently loaded RenderScene.
    scene: render_scene::RenderScene,
    /// CPU storage for handles to RenderObjects in the RenderScene.
    scene_objects: SceneObjects,
    /// ECS data.
    ecs: ECSData,
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
    fn new(context: &GraphicsContext) -> Self {
        let Textures {
            bind_group_layout: texture_bind_group_layout,
            cube_texture,
            cube_texture_bind_group,
        } = Self::init_textures(&context.device, &context.queue);

        // ------------

        let mut scene = render_scene::RenderScene::new(&context.device, &["cube.obj", "cone.obj"]);

        let mut world = hecs::World::new();

        let cube_object = scene.register_object(&RenderObjectDescriptor {
            mesh_id: 0,
            transform: m::Mat4::IDENTITY,
            render_bounds: mesh::RenderBounds {
                origin: m::Vec3::ZERO,
                radius: 3.0,
            },
            draw_forward_pass: true,
        });

        let cube_entity = world.spawn(
            hecs::EntityBuilder::new()
                .add(cube_object)
                .add(components::Transform::default())
                .add(components::EntityName::from("Cube 0"))
                .build(),
        );

        let cube_object2 = scene.register_object(&RenderObjectDescriptor {
            mesh_id: 0,
            transform: m::Mat4::IDENTITY,
            render_bounds: mesh::RenderBounds {
                origin: m::Vec3::ZERO,
                radius: 3.0,
            },
            draw_forward_pass: true,
        });

        let cube_entity2 = world.spawn(
            hecs::EntityBuilder::new()
                .add(cube_object2)
                .add(components::Transform::default())
                .add(components::EntityName::from("Cube 1"))
                .build(),
        );

        let cone_object = scene.register_object(&RenderObjectDescriptor {
            mesh_id: 1,
            transform: m::Mat4::IDENTITY,
            render_bounds: mesh::RenderBounds {
                origin: m::Vec3::ZERO,
                radius: 3.0,
            },
            draw_forward_pass: true,
        });

        let cone_entity = world.spawn(
            hecs::EntityBuilder::new()
                .add(cone_object)
                .add(components::Transform::default())
                .add(components::EntityName::from("Cone entity"))
                .build(),
        );

        let cone_object2 = scene.register_object(&RenderObjectDescriptor {
            mesh_id: 1,
            transform: m::Mat4::IDENTITY,
            render_bounds: mesh::RenderBounds {
                origin: m::Vec3::ZERO,
                radius: 3.0,
            },
            draw_forward_pass: true,
        });

        let cone_entity2 = world.spawn(
            hecs::EntityBuilder::new()
                .add(cone_object2)
                .add(components::Transform::default())
                .add(components::EntityName::from("Cone entity2"))
                .build(),
        );
        scene.build_batches(&context.queue);

        let camera = camera::EditorCamera::init(&context.config);

        let uniform_buffer = context
            .device
            .create_buffer_init_t::<camera::CameraUniform>(&wgpu::util::BufferInitDescriptor {
                label: Some("camera uniform buffer"),
                contents: bytemuck::cast_slice(slice::from_ref(&camera.uniform_data)),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let vertex_shader_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let camera_bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
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

        let shader = context
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/vert_frag.wgsl").into()),
            });

        let render_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("render pipeline layout"),
                    bind_group_layouts: &[
                        &vertex_shader_bind_group_layout, // group 0
                        &texture_bind_group_layout,       // group 1
                    ],
                    push_constant_ranges: &[],
                });

        let render_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                            format: context.config.format,
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

        let compute_shader = context
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("compute shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/compute.wgsl").into()),
            });

        let compute_bind_group_layout = context
            .device
            .create_bind_group_layout(&render_scene::compute_pipeline::BIND_GROUP_LAYOUT_DESC);

        let compute_bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("compute bind group"),
                layout: &compute_bind_group_layout,
                entries: &render_scene::compute_pipeline::bind_group_entries(
                    &uniform_buffer,
                    &scene,
                ),
            });

        let compute_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("compute pipeline layout"),
                    bind_group_layouts: &[&compute_bind_group_layout],
                    push_constant_ranges: &[],
                });

        let compute_pipeline =
            context
                .device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
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
            compute,
            render,
            _cube_texture: cube_texture,
            camera,
            uniform_buffer,
            scene,
            scene_objects: SceneObjects {
                cube_object,
                cube_object2,
                cone_object,
                cone_object2,
            },
            ecs: ECSData {
                world,
                entities: vec![cube_entity, cube_entity2, cone_entity, cone_entity2],
                cube_entity,
            },
        }
    }

    fn on_event(&mut self, event: &events::PenguinEvent) -> bool {
        if self.camera.controller.on_event(&event) {
            return true;
        }

        match event {
            events::PenguinEvent::Window(events::event::WindowResizeEvent { size, .. }) => {
                self.camera.projection.resize((size.width, size.height));

                false
            }
            _ => false,
        }
    }

    /// Called each frame.
    fn update_camera_and_scene(&mut self, context: &GraphicsContext, dt: std::time::Duration) {
        // update camera data
        self.camera.update(dt);

        // schedule uniform buffer write
        context.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(slice::from_ref(&self.camera.uniform_data)),
        );

        let (_x, y) = unsafe {
            TIME_STATE += dt.as_secs_f32() * 2.;
            (f32::cos(TIME_STATE), f32::sin(TIME_STATE))
        };

        // cube 2
        let mut transform = self
            .ecs
            .world
            .query_one_mut::<&mut components::Transform>(self.ecs.entities[1])
            .unwrap();

        transform.translation = m::vec3(1., 4., y);
        transform.is_dirty = true;

        for (entity, (render_object, mut transform)) in self
            .ecs
            .world
            .query_mut::<(&Handle<RenderObject>, &mut components::Transform)>()
        {
            // cone 0
            if entity == self.ecs.entities[2] {
                transform.translation = m::vec3(3.1, 4. + y, 0.);
                transform.is_dirty = true;
            }

            // cone 1
            if entity == self.ecs.entities[3] {
                transform.translation = m::vec3(3.1, -y, 0.);
                transform.rotation = m::vec3(0., 180.0, 0.);
                transform.is_dirty = true;
            }

            if transform.is_dirty {
                self.scene
                    .update_transform_model_matrix(*render_object, transform.model_matrix());

                transform.is_dirty = false;
            }
        }

        // update scene
        self.scene.update(&context.queue);
    }

    /// Access the output view texture to submit render commands.
    fn render<OutputTextureFunc: FnOnce(&wgpu::TextureView)>(
        &self,
        context: &GraphicsContext,
        f: OutputTextureFunc,
    ) -> Result<(), wgpu::SurfaceError> {
        let output_texture = context.surface.get_current_texture()?;
        let output_texture_view = output_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        f(&output_texture_view);

        output_texture.present();

        Ok(())
    }

    /// Compute commands.
    fn compute_commands(
        &self,
        device: &wgpu::Device,
        encoder: Option<wgpu::CommandEncoder>,
    ) -> wgpu::CommandEncoder {
        let mut cmd = match encoder {
            Some(encoder) => encoder,
            None => device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("compute commands encoder"),
            }),
        };

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

        cmd
    }

    fn render_commands(
        &self,
        device: &wgpu::Device,
        output_texture_view: &wgpu::TextureView,
        depth_texture_view: &wgpu::TextureView,
        encoder: Option<wgpu::CommandEncoder>,
    ) -> wgpu::CommandEncoder {
        let mut cmd = match encoder {
            Some(encoder) => encoder,
            None => device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render commands encoder"),
            }),
        };

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
                    view: &depth_texture_view,
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

        cmd
    }
}

pub trait PenguinEventListener<T> {
    fn on_penguin_event(&mut self, e: &T) -> bool;
}

/// Entry point.
fn main() {
    env_logger::init();
    let event_loop = EventLoop::with_user_event();
    let window = WindowBuilder::new()
        .with_title("Penguin engine")
        .build(&event_loop)
        .unwrap();

    let mut context = penguin_util::pollster::block_on(GraphicsContext::new(&window));

    // base render layer --------
    let mut state = RendererState::new(&context);

    // egui -------
    let mut editor = editor::EditorState::new(&context);

    // clock for calculating delta time -----
    let mut clock = time::Clock::start();
    let event_sender = events::PenguinEventSender::init(event_loop.create_proxy());

    event_loop.run(move |event, _, control_flow| {
        // pass winit events to editor layer
        editor.handle_platform_event(&event);

        match event {
            winit::event::Event::UserEvent(penguin_event) => {
                let mut event_consumed = false;

                event_consumed = context.on_event(&penguin_event);

                if !event_consumed {
                    event_consumed = editor.on_event(&penguin_event);
                }

                if !event_consumed {
                    event_consumed = state.on_event(&penguin_event);
                }
            }
            //
            winit::event::Event::DeviceEvent { ref event, .. } => {
                match event {
                    winit::event::DeviceEvent::Key(KeyboardInput {
                        scancode: _,
                        state,
                        virtual_keycode: Some(keycode),
                        ..
                    }) => {
                        let key = input::Key::from_virtual_keycode(*keycode);

                        if let Some(key) = key {
                            event_sender.send_event(events::PenguinEvent::Input(
                                input::InputEvent::Key(input::KeyEvent {
                                    key,
                                    state: input::KeyState::from(*state),
                                }),
                            ));
                        }
                    }
                    winit::event::DeviceEvent::Button {
                        button: 1, // left mouse button
                        state,
                    } => {
                        event_sender.send_event(events::PenguinEvent::Input(
                            input::InputEvent::Key(input::KeyEvent {
                                key: input::Key::LMouseButton,
                                state: input::KeyState::from(*state),
                            }),
                        ));
                    }
                    winit::event::DeviceEvent::MouseMotion { delta } => {
                        event_sender.send_event(events::PenguinEvent::Input(
                            input::InputEvent::MouseMotion(*delta),
                        ));
                    }
                    _ => {}
                }
            }
            //
            winit::event::Event::WindowEvent {
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
                    WindowEvent::Resized(physical_size) => event_sender.send_event(
                        events::PenguinEvent::Window(events::event::WindowResizeEvent {
                            size: *physical_size,
                            scale_factor: None,
                        }),
                    ),
                    WindowEvent::ScaleFactorChanged {
                        scale_factor,
                        new_inner_size,
                    } => event_sender.send_event(events::PenguinEvent::Window(
                        events::event::WindowResizeEvent {
                            size: **new_inner_size,
                            scale_factor: Some(*scale_factor),
                        },
                    )),
                    _ => {}
                }
            }
            //
            winit::event::Event::MainEventsCleared => {
                window.request_redraw();
            }
            //
            winit::event::Event::RedrawRequested(window_id) if window_id == window.id() => {
                let dt = clock.tick();

                // update
                {
                    state.update_camera_and_scene(&context, dt);

                    editor.update(
                        &context,
                        &window,
                        &editor::FrameData {
                            clock: &clock,
                            world: &state.ecs.world,
                        },
                    );
                }

                // compute commands
                {
                    let cmd = state.compute_commands(&context.device, None);

                    context.queue.submit(iter::once(cmd.finish()));
                }

                // render commands
                {
                    // get frame surface texture to render to
                    let render_result = state.render(&context, |output| {
                        let cmd = state.render_commands(
                            &context.device,
                            output,
                            &context.depth_texture.view,
                            None,
                        );

                        let cmd = editor.render_commands(&context.device, output, Some(cmd));

                        context.queue.submit(iter::once(cmd.finish()));
                    });

                    match render_result {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => {
                            println!("Surface lost. Reconfiguring");

                            // reconfigure
                            event_sender.send_event(PenguinEvent::Window(
                                events::event::WindowResizeEvent {
                                    size: context.size,
                                    scale_factor: Some(context.scale_factor),
                                },
                            ));
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            eprintln!("Out of memory. Exiting");
                            *control_flow = ControlFlow::Exit;
                        }
                        Err(e) => eprintln!("Surface error: {:?}", e),
                    };
                }
            }
            //
            _ => {}
        }
    });
}
