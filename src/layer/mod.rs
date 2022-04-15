mod application_layer;
mod base_render_scene_layer;
mod pipelines_layer;
mod scene_layer;
mod editor_layer;

pub use application_layer::ApplicationLayer;
pub use base_render_scene_layer::BaseRenderSceneLayer;
pub use pipelines_layer::PipelinesLayer;
pub use scene_layer::SceneLayer;

use crate::{
    camera, components, editor, mesh, render_scene, texture, RenderInstance,
    RenderObjectDescriptor, Vertex,
};
use crate::{GraphicsContext, RendererState};
use atomic_refcell::AtomicRef;
use editor::EditorComponentStorage;
use legion::{component, maybe_changed, IntoQuery, Resources};
use macaw as m;
use penguin_util::handle::Handle;
use penguin_util::{GpuBuffer, GpuBufferDeviceExt};
use std::slice;

use crate::render_scene::MAX_DRAW_COMMANDS;
use components::*;
use legion::system;
use legion::systems::Step;
use penguin_util::raw_gpu_types::DrawIndirectCount;
use render_scene::RenderObject;
use wgpu::CommandEncoder;

mod leg {
    pub use legion::storage::*;
    pub use legion::systems::*;
    pub use legion::world::*;
    pub use legion::*;
}

pub trait Layer {
    fn init(self, cmd: &mut leg::CommandBuffer, resources: &mut leg::Resources);
    fn startup_steps() -> Option<Vec<leg::Step>>;
    fn run_steps() -> Option<Vec<leg::Step>>;
}

use resources::*;
mod resources {
    use super::*;
    use crate::layer::base_render_scene_layer::RenderObjects;
    use crate::render_scene;

    impl EditorComponentStorage {
        pub fn init_register() -> Self {
            use components::*;
            let mut s = editor::EditorComponentStorage::default();
            s.register_component_editor::<Name>();
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

    pub struct Entities {
        pub entities: Vec<legion::Entity>,
    }

    impl Entities {
        pub fn init(
            cmd: &mut leg::CommandBuffer,
            context: &GraphicsContext,
            resources: &legion::systems::Resources,
        ) -> Self {
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

            let mesh_assets = ["cube.obj", "cone.obj"];

            let mut render_objects = resources.get_mut::<RenderObjects>().unwrap();

            // register render objects
            //
            let mut render_obj_desc = render_scene::RenderObjectDescriptor {
                mesh_handle: Handle::from(0),
                transform: m::Mat4::IDENTITY,
                render_bounds: mesh::RenderBounds {
                    origin: m::Vec3::ZERO,
                    radius: 3.0,
                },
                draw_forward_pass: true,
            };

            let cube_object = render_objects.register_object(&render_obj_desc);
            let cube_object2 = render_objects.register_object(&render_obj_desc);

            render_obj_desc.mesh_handle = Handle::from(1);
            let cone_object = render_objects.register_object(&render_obj_desc);
            let cone_object2 = render_objects.register_object(&render_obj_desc);
            let test_object = render_objects.register_object(&render_obj_desc);

            render_objects.should_rebuild_batches = true;
            // render_objects.build_batches(&context.queue);

            // construct entities
            let entities = vec![
                base_entity(cmd, "Cube 0", cube_object, Transf::TRS),
                base_entity(cmd, "Cube 1", cube_object2, Transf::TR),
                base_entity(cmd, "Cone 0", cone_object, Transf::T),
                base_entity(cmd, "Cone 1", cone_object2, Transf::TRS),
                base_entity(cmd, "Test 0", test_object, Transf::TRS),
            ];

            Self { entities }
        }
    }
}

// fn create_uniform_buffer(
//     device: &wgpu::Device,
//     uniform_data: &camera::CameraUniformData,
// ) -> GpuBuffer<camera::CameraUniformData> {
//     device.create_buffer_init_t::<camera::CameraUniformData>(&wgpu::util::BufferInitDescriptor {
//         label: Some("camera uniform buffer"),
//         contents: bytemuck::cast_slice(slice::from_ref(uniform_data)),
//         usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
//     })
// }
