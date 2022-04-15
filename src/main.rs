mod bind_groups;
mod camera;
mod components;
mod editor;
mod events;
mod graphics_context;
mod input;
mod layer;
mod mesh;
mod render_scene;
mod texture;
mod time;

use graphics_context::GraphicsContext;

/// The maximum amount of draw calls expected. Decides the size of the draw commands buffer
/// (and will in the future simply indicate the maximum expected draw count).
const MAX_DRAW_COMMANDS: usize = 100;

use crate::events::PenguinEvent;

use crate::{
    mesh::{Vertex, VertexArrayBuffer},
    render_scene::{DrawOutputInfo, RenderObjectDescriptor},
};

use legion::{maybe_changed, IntoQuery, Resources};
use macaw as m;
use penguin_util::{
    handle::Handle, raw_gpu_types::DrawIndirectCount, GpuBuffer, GpuBufferDeviceExt,
};

use crate::bind_groups::DeviceExt;
use crate::layer::Layer;
use legion::systems::CommandBuffer;
use std::mem::transmute;
use std::{iter, mem, slice};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
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
pub struct Compute {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group: wgpu::BindGroup,
}

/// Data related to a render pass.
pub struct Render {
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_shader_bind_group: wgpu::BindGroup,
    pub fragment_shader_bind_group: wgpu::BindGroup,
}

struct LegionECSData {
    world: legion::World,
    resources: legion::Resources,
    #[allow(unused)]
    entities: Vec<legion::Entity>,
}

struct Yeet {
    renderer_layer: legion::systems::Schedule,
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
    camera: camera::MainCamera,
    /// Uniform buffer.
    uniform_buffer: GpuBuffer<camera::CameraUniformData>,
    /// The currently loaded RenderScene.
    scene: render_scene::RenderScene,
    /// ECS data.
    ecs: LegionECSData,
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

#[derive(Default)]
struct RenderObjectStorage {
    render_objects: Vec<render_scene::RenderObject>,
}
penguin_util::impl_deref!(
    mut RenderObjectStorage,
    render_objects,
    Vec<render_scene::RenderObject>
);

impl RendererState {
    fn new(context: &GraphicsContext) -> Self {
        let mut l_world = legion::World::default();

        let mut l_resources = legion::Resources::default();
        l_resources.insert(RenderObjectStorage::default());

        // editor
        let components_ui_storage = {
            use components::*;
            let mut s = editor::EditorComponentStorage::default();
            s.register_component_editor::<Name>();
            s.register_component_editor::<Translation>();
            s.register_component_editor::<Rotation>();
            s.register_component_editor::<Scale>();
            s
        };
        l_resources.insert(components_ui_storage);

        let mut cmd = legion::systems::CommandBuffer::new(&l_world);

        let Textures {
            bind_group_layout: texture_bind_group_layout,
            cube_texture,
            cube_texture_bind_group,
        } = Self::init_textures(&context.device, &context.queue);

        // ------------

        let (scene, entities) = {
            // helpers ----------

            enum Transf {
                T,
                TR,
                TRS,
            }

            fn base_entity(
                cmd: &mut CommandBuffer,
                name: &str,
                render_obj: Handle<render_scene::RenderObject>,
                transf: Transf,
            ) -> legion::Entity {
                let name = components::Name::from(name);

                match transf {
                    Transf::T => cmd.push((name, render_obj, components::Translation::default())),
                    Transf::TR => cmd.push((
                        name,
                        render_obj,
                        components::Translation::default(),
                        components::Rotation::default(),
                    )),
                    Transf::TRS => cmd.push((
                        name,
                        render_obj,
                        components::Translation::default(),
                        components::Rotation::default(),
                        components::Scale::default(),
                    )),
                }
            }

            // --------
            let mesh_assets = ["cube.obj", "cone.obj"];

            let mut scene = render_scene::RenderScene::new(&context.device, &mesh_assets);

            // register render objects
            //
            let mut render_obj_desc = RenderObjectDescriptor {
                // mesh_id: 0,
                mesh_handle: Handle::from(0),
                transform: m::Mat4::IDENTITY,
                render_bounds: mesh::RenderBounds {
                    origin: m::Vec3::ZERO,
                    radius: 3.0,
                },
                draw_forward_pass: true,
            };

            let cube_object = scene.register_object(&render_obj_desc);
            let cube_object2 = scene.register_object(&render_obj_desc);

            render_obj_desc.mesh_handle = Handle::from(1);
            let cone_object = scene.register_object(&render_obj_desc);
            let cone_object2 = scene.register_object(&render_obj_desc);
            let test_object = scene.register_object(&render_obj_desc);

            scene.build_batches(&context.queue);

            // construct entities
            let entities = vec![
                base_entity(&mut cmd, "Cube 0", cube_object, Transf::TRS),
                base_entity(&mut cmd, "Cube 1", cube_object2, Transf::TR),
                base_entity(&mut cmd, "Cone 0", cone_object, Transf::T),
                base_entity(&mut cmd, "Cone 1", cone_object2, Transf::TRS),
                base_entity(&mut cmd, "Test 0", test_object, Transf::TRS),
            ];

            (scene, entities)
        };

        cmd.flush(&mut l_world, &mut l_resources);

        let camera = camera::MainCamera::init(&context.config);

        let uniform_buffer = context
            .device
            .create_buffer_init_t::<camera::CameraUniformData>(&wgpu::util::BufferInitDescriptor {
                label: Some("camera uniform buffer"),
                contents: bytemuck::cast_slice(slice::from_ref(&camera.uniform_data)),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        const VERTEX: wgpu::ShaderStages = wgpu::ShaderStages::VERTEX;
        const READ: bool = true;
        const READ_WRITE: bool = false;

        let vertex_shader_bind_group_layout = bind_groups::BindGroupLayoutBuilder::<3>::builder()
            .uniform_buffer(0, VERTEX) // camera uniform
            .storage_buffer(1, VERTEX, READ) // render objects
            .storage_buffer(2, VERTEX, READ) // instance_index to render_object map
            .build(&context.device, Some("vertex bind group layout"));

        let camera_bind_group = bind_groups::BindGroupBuilder::<3>::builder()
            .buffer(0, &uniform_buffer)
            .buffer(1, &scene.render_objects_buffer)
            .buffer(2, &scene.instance_index_to_render_object_map)
            .build(
                &context.device,
                Some("vertex bind group"),
                &vertex_shader_bind_group_layout,
            );

        let render_pipeline = {
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

            render_pipeline
        };

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

        const COMPUTE: wgpu::ShaderStages = wgpu::ShaderStages::COMPUTE;

        let compute_bind_group_layout = bind_groups::BindGroupLayoutBuilder::<7>::builder()
            .uniform_buffer(0, COMPUTE)
            .storage_buffer(1, COMPUTE, READ)
            .storage_buffer(2, COMPUTE, READ)
            .storage_buffer(3, COMPUTE, READ_WRITE)
            .storage_buffer(4, COMPUTE, READ_WRITE)
            .storage_buffer(5, COMPUTE, READ_WRITE)
            .storage_buffer(6, COMPUTE, READ_WRITE)
            .build(&context.device, Some("compute bind group layout"));

        let compute_bind_group = bind_groups::BindGroupBuilder::<7>::builder()
            .buffer(0, &uniform_buffer)
            .buffer(1, &scene.draw_commands_buffer)
            .buffer(2, &scene.render_objects_buffer)
            .buffer(3, &scene.compute_shader_local_data_buffer)
            .buffer(4, &scene.draw_count_buffer)
            .buffer(5, &scene.out_draw_commands_buffer)
            .buffer(6, &scene.instance_index_to_render_object_map)
            .build(
                &context.device,
                Some("compute bind group"),
                &compute_bind_group_layout,
            );

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
            ecs: LegionECSData {
                world: l_world,
                resources: l_resources,
                entities,
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

        // legion ecs ------------------------
        let _world = &mut self.ecs.world;

        let mut translation_query =
            <(&mut components::Translation, &mut components::Rotation)>::query();
        for (mut translation, _rotation) in translation_query.iter_mut(&mut self.ecs.world) {
            translation.0 = m::vec3(4.1, 4. + y, 0.);
        }

        use components::*;
        use legion::component;

        {
            type TransQuery = (
                &'static Handle<render_scene::RenderObject>,
                &'static Translation,
            );

            let mut translation_query = <TransQuery>::query().filter(
                !component::<components::Rotation>()
                    & !component::<Scale>()
                    & maybe_changed::<Translation>(),
            );

            for (render_obj, translation) in translation_query.iter(&self.ecs.world) {
                self.scene.update_transform_model_matrix(
                    *render_obj,
                    m::Mat4::from_translation(translation.0),
                );
            }
        }

        {
            type TransRotQuery = (
                &'static Handle<render_scene::RenderObject>,
                &'static Translation,
                &'static Rotation,
            );

            let mut query = <TransRotQuery>::query().filter(
                !component::<Scale>()
                    & (maybe_changed::<Translation>() | maybe_changed::<Rotation>()),
            );

            for (render_obj, trans, rot) in query.iter(&self.ecs.world) {
                //let rot = m::Quat::from_euler(m::EulerRot::XYZ, rot.x, rot.y, rot.z);
                self.scene.update_transform_model_matrix(
                    *render_obj,
                    m::Mat4::from_rotation_translation(rot.0, trans.0),
                );
            }
        }

        {
            type TransRotScaleQuery = (
                &'static Handle<render_scene::RenderObject>,
                &'static components::Translation,
                &'static components::Rotation,
                &'static components::Scale,
            );

            let mut query = <TransRotScaleQuery>::query().filter(
                maybe_changed::<components::Translation>()
                    | maybe_changed::<Rotation>()
                    | maybe_changed::<Scale>(),
            );

            for (render_obj, trans, rot, scale) in query.iter(&self.ecs.world) {
                self.scene.update_transform_model_matrix(
                    *render_obj,
                    m::Mat4::from_scale_rotation_translation(scale.0, rot.0, trans.0),
                );
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

fn main() {
    // main_without_layers();
    main_with_layers();
}

/// Entry point.
fn main_without_layers() {
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
                #[allow(unused)]
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

                    let ui_storage = state
                        .ecs
                        .resources
                        .get::<editor::EditorComponentStorage>()
                        .expect("ui storage");

                    editor.update(
                        &context,
                        &window,
                        &mut editor::FrameData {
                            clock: &clock,
                            l_world: &mut state.ecs.world,
                            ui_storage: &ui_storage,
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

/// Entry point.
fn main_with_layers() {
    env_logger::init();
    let event_loop = EventLoop::with_user_event();
    let window = WindowBuilder::new()
        .with_title("Penguin engine")
        .build(&event_loop)
        .unwrap();

    let _event_sender = events::PenguinEventSender::init(event_loop.create_proxy());

    let mut world = legion::World::default();
    let mut resources = legion::Resources::default();

    let mut cmd = legion::systems::CommandBuffer::new(&world);

    // layers -------
    layer::ApplicationLayer.init(&mut cmd, &mut resources);
    layer::SceneLayer.init(&mut cmd, &mut resources);
    cmd.flush(&mut world, &mut resources);

    layer::BaseRenderSceneLayer {
        window: &window,
        mesh_assets: &["cube.obj", "cone.obj"],
    }
    .init(&mut cmd, &mut resources);

    layer::PipelinesLayer.init(&mut cmd, &mut resources);

    cmd.flush(&mut world, &mut resources);

    let mut startup_steps = Vec::new();
    startup_steps.extend(layer::BaseRenderSceneLayer::startup_steps().unwrap());
    let mut startup_schedule = legion::systems::Schedule::from(startup_steps);
    startup_schedule.execute(&mut world, &mut resources);

    // steps ---------
    let mut steps = Vec::new();
    steps.extend(layer::ApplicationLayer::run_steps().unwrap());
    steps.extend(layer::SceneLayer::run_steps().unwrap());

    steps.extend(layer::BaseRenderSceneLayer::run_steps().unwrap());
    steps.extend(layer::PipelinesLayer::run_steps().unwrap());

    let mut schedule = legion::systems::Schedule::from(steps);

    event_loop.run(move |event, _, control_flow| {
        use winit::event::Event;

        match event {
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                schedule.execute(&mut world, &mut resources);
            }
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
                        let mut context = resources.get_mut::<GraphicsContext>().unwrap();
                        context.on_resize(*physical_size, None);
                    }
                    WindowEvent::ScaleFactorChanged {
                        scale_factor,
                        new_inner_size,
                    } => {
                        let mut context = resources.get_mut::<GraphicsContext>().unwrap();
                        context.on_resize(**new_inner_size, Some(*scale_factor as _));
                    }
                    _ => {}
                }
            }

            _ => {}
        }
    });

    // ..
}
