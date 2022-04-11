use crate::{camera, components, editor, mesh, render_scene, texture, RenderInstance, Vertex};
use crate::{GraphicsContext, RendererState};
use atomic_refcell::AtomicRef;
use editor::EditorComponentStorage;
use legion::{component, maybe_changed, IntoQuery};
use macaw as m;
use penguin_util::handle::Handle;
use penguin_util::{GpuBuffer, GpuBufferDeviceExt};
use std::slice;

mod leg {
    pub use legion::storage::*;
    pub use legion::systems::*;
    pub use legion::world::*;
    pub use legion::*;
}

pub trait Layer {
    fn init(cmd: &mut leg::CommandBuffer, resources: &mut leg::Resources) -> Self;
    fn run_steps() -> Vec<leg::Step>;
}

/// State with data necessary to render.
// pub struct RendererState {
//     /// Compute pass data.
//     compute: Compute,
//     /// Render pass data.
//     render: Render,
//     // A texture.
//     _cube_texture: texture::Texture,
//     /// Editor camera data.
//     camera: camera::EditorCamera,
//     /// Uniform buffer.
//     uniform_buffer: GpuBuffer<camera::CameraUniform>,
//     /// The currently loaded RenderScene.
//     scene: render_scene::RenderScene,
//     /// ECS data.
//     ecs: LegionECSData,
// }
use resources::*;
mod resources {
    use super::*;
    use crate::render_scene;

    impl EditorComponentStorage {
        pub fn init_register() -> Self {
            use components::*;
            let mut s = editor::EditorComponentStorage::default();
            s.register_component_editor::<EntityName>();
            s.register_component_editor::<Translation>();
            s.register_component_editor::<Rotation>();
            s.register_component_editor::<Scale>();
            s
        }
    }

    pub struct Textures {
        pub bind_group_layout: wgpu::BindGroupLayout,
        //
        pub cube_texture: texture::Texture,
        // pub cube_texture_bind_group: wgpu::BindGroup,
    }
    impl Textures {
        pub fn init(device: &wgpu::Device, queue: &wgpu::Queue) -> (Textures, wgpu::BindGroup) {
            let cube_texture =
                texture::Texture::from_asset(device, queue, "cube-diffuse.jpg").unwrap();

            use crate::bind_groups::layout_entry;

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

            (
                Self {
                    bind_group_layout: texture_bind_group_layout,
                    cube_texture,
                },
                cube_texture_bind_group,
            )
        }
    }

    pub struct Scene {
        pub render_scene: render_scene::RenderScene,
        pub entities: Vec<legion::Entity>,
    }

    impl Scene {
        pub fn init(cmd: &mut leg::CommandBuffer, context: &GraphicsContext) -> Self {
            enum Transf {
                T,
                TR,
                TRS,
            }

            fn base_entity(
                cmd: &mut leg::CommandBuffer,
                name: &str,
                render_obj: Handle<render_scene::RenderObject>,
                transf: Transf,
            ) -> legion::Entity {
                let name = components::EntityName::from(name);

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

            let mesh_assets = ["cube.obj", "cone.obj"];

            let mut scene = render_scene::RenderScene::new(&context.device, &mesh_assets);

            // register render objects
            //
            let mut render_obj_desc = render_scene::RenderObjectDescriptor {
                mesh_id: 0,
                transform: m::Mat4::IDENTITY,
                render_bounds: mesh::RenderBounds {
                    origin: m::Vec3::ZERO,
                    radius: 3.0,
                },
                draw_forward_pass: true,
            };

            let cube_object = scene.register_object(&render_obj_desc);
            let cube_object2 = scene.register_object(&render_obj_desc);

            render_obj_desc.mesh_id = 1;
            let cone_object = scene.register_object(&render_obj_desc);
            let cone_object2 = scene.register_object(&render_obj_desc);
            let test_object = scene.register_object(&render_obj_desc);

            scene.build_batches(&context.queue);

            // construct entities
            let entities = vec![
                base_entity(cmd, "Cube 0", cube_object, Transf::TRS),
                base_entity(cmd, "Cube 1", cube_object2, Transf::TR),
                base_entity(cmd, "Cone 0", cone_object, Transf::T),
                base_entity(cmd, "Cone 1", cone_object2, Transf::TRS),
                base_entity(cmd, "Test 0", test_object, Transf::TRS),
            ];

            Self {
                render_scene: scene,
                entities,
            }
        }
    }
}

pub struct RendererLayer;

fn create_uniform_buffer(
    device: &wgpu::Device,
    uniform_data: &camera::CameraUniformData,
) -> GpuBuffer<camera::CameraUniformData> {
    device.create_buffer_init_t::<camera::CameraUniformData>(&wgpu::util::BufferInitDescriptor {
        label: Some("camera uniform buffer"),
        contents: bytemuck::cast_slice(slice::from_ref(uniform_data)),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

fn vertex_shader_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
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
    })
}

use crate::render_scene::compute_pipeline::bind_group_entries;
use util::*;

mod util {
    pub fn buffer_entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry {
        wgpu::BindGroupEntry {
            binding,
            resource: buffer.as_entire_binding(),
        }
    }
}

struct RenderPipelineDesc<'a> {
    shader_src: &'a str,
    bind_group_layouts: &'a [&'a wgpu::BindGroupLayout],
}

fn render_pipeline(context: &GraphicsContext, desc: &RenderPipelineDesc) -> wgpu::RenderPipeline {
    let shader = context
        .device
        .create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("render pipeline shader"),
            source: wgpu::ShaderSource::Wgsl(desc.shader_src.into()),
        });

    let render_pipeline_layout =
        context
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render pipeline layout"),
                bind_group_layouts: desc.bind_group_layouts,
                push_constant_ranges: &[],
            });

    let render_pipeline = context
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
}

struct ComputePipelineDesc<'a> {
    shader_src: &'a str,
    bind_group_layouts: &'a [&'a wgpu::BindGroupLayout],
}

fn compute_pipeline(device: &wgpu::Device, desc: &ComputePipelineDesc) -> wgpu::ComputePipeline {
    let compute_shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: Some("compute shader"),
        source: wgpu::ShaderSource::Wgsl(desc.shader_src.into()),
    });

    let compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("compute pipeline layout"),
        bind_group_layouts: desc.bind_group_layouts,
        push_constant_ranges: &[],
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("compute pipeline"),
        layout: Some(&compute_pipeline_layout),
        module: &compute_shader,
        entry_point: "cs_main",
    });

    compute_pipeline
}

impl Layer for RendererLayer {
    fn init(cmd: &mut leg::CommandBuffer, r: &mut leg::Resources) -> Self {
        let context: AtomicRef<GraphicsContext> = r
            .get::<GraphicsContext>()
            .expect("no graphics context resource");
        let device = &context.device;

        // init resources
        let editor_component_storage = EditorComponentStorage::init_register();
        let scene = Scene::init(cmd, &context);
        let camera = camera::EditorCamera::init(&context.config);
        let (textures, textures_bind_group) = Textures::init(device, &context.queue);

        let uniform_buffer = create_uniform_buffer(device, &camera.uniform_data);

        use crate::Render;

        let render: Render = {
            let vertex_shader_bind_group_layout = vertex_shader_bind_group_layout(device);

            let vertex_shader_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vertex shader bind group"),
                layout: &vertex_shader_bind_group_layout,
                entries: &[
                    buffer_entry(0, &uniform_buffer),
                    buffer_entry(1, &scene.render_scene.render_objects_buffer),
                    buffer_entry(2, &scene.render_scene.instance_index_to_render_object_map),
                ],
            });

            let render_pipeline = render_pipeline(
                &context,
                &RenderPipelineDesc {
                    shader_src: include_str!("shaders/vert_frag.wgsl"),
                    bind_group_layouts: &[
                        &vertex_shader_bind_group_layout, // group 0
                        &textures.bind_group_layout,
                    ],
                },
            );

            Render {
                pipeline: render_pipeline,
                vertex_shader_bind_group,
                fragment_shader_bind_group: textures_bind_group,
            }
        };

        use crate::Compute;

        let compute: Compute = {
            let compute_bind_group_layout = device
                .create_bind_group_layout(&render_scene::compute_pipeline::BIND_GROUP_LAYOUT_DESC);

            let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("compute bind group"),
                layout: &compute_bind_group_layout,
                entries: &render_scene::compute_pipeline::bind_group_entries(
                    &uniform_buffer,
                    &scene.render_scene,
                ),
            });

            let compute_pipeline = compute_pipeline(
                device,
                &ComputePipelineDesc {
                    shader_src: include_str!("shaders/compute.wgsl"),
                    bind_group_layouts: &[&compute_bind_group_layout],
                },
            );

            Compute {
                pipeline: compute_pipeline,
                bind_group: compute_bind_group,
            }
        };

        drop(context);

        // insert resources
        r.insert(editor_component_storage);
        r.insert(scene);
        r.insert(camera);
        r.insert(textures);
        r.insert(render);
        r.insert(compute);

        Self
        // todo!()
    }

    fn run_steps() -> Vec<Step> {
        legion::Schedule::builder()
            .add_system(update_translation_system())
            .add_system(update_translation_rotation_system())
            .add_system(update_translation_rotation_scale_system())
            .build()
            .into_vec()
    }
}

use components::*;
use legion::system;
use legion::systems::Step;
use render_scene::RenderObject;

// todo: Multithreaded access to scene for updating the render scene objects

#[system(for_each)]
#[filter(
maybe_changed::<Translation>()
& !component::<Rotation>()
& !component::<Scale>()
)]
fn update_translation(
    render_obj: &Handle<RenderObject>,
    translation: &Translation,
    #[resource] scene: &mut Scene,
) {
    scene
        .render_scene
        .update_transform_model_matrix(*render_obj, m::Mat4::from_translation(translation.0));
}

#[system(for_each)]
#[filter(
maybe_changed::<Translation>()
| maybe_changed::<Rotation>()
& !component::<Scale>()
)]
fn update_translation_rotation(
    render_obj: &Handle<RenderObject>,
    translation: &Translation,
    rotation: &Rotation,
    #[resource] scene: &mut Scene,
) {
    scene.render_scene.update_transform_model_matrix(
        *render_obj,
        m::Mat4::from_rotation_translation(rotation.0, translation.0),
    )
}

#[system(for_each)]
#[filter(
maybe_changed::<Translation>()
| maybe_changed::<Rotation>()
| maybe_changed::<Scale>()
)]
fn update_translation_rotation_scale(
    render_obj: &Handle<RenderObject>,
    translation: &Translation,
    rotation: &Rotation,
    scale: &Scale,
    #[resource] scene: &mut Scene,
) {
    scene.render_scene.update_transform_model_matrix(
        *render_obj,
        m::Mat4::from_scale_rotation_translation(scale.0, rotation.0, translation.0),
    );
}
