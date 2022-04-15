mod enqueue_transform_updates;

use crate::layer::scene_layer;
use crate::{
    bind_groups, mesh, render_scene, GraphicsContext, Layer, RenderObjectDescriptor, Vertex,
    VertexArrayBuffer, MAX_DRAW_COMMANDS,
};
use legion::systems::{CommandBuffer, Step};
use legion::{Entity, Resources, Schedule};
use penguin_util::handle::{Handle, HandleMap};
use std::{mem, slice};

use crate::components::Translation;
use legion::system;
use wgpu::{BindGroupLayoutEntry, ShaderStages};

use crate::events::PenguinEventSender;
use crate::render_scene::mesh_pass;
use crate::render_scene::mesh_pass::{IndirectBatch, MeshPass, PassObject};
use crate::render_scene::RenderObject;
use crate::{events, DrawOutputInfo, RenderInstance};
use macaw as m;
use penguin_util::raw_gpu_types::{DrawIndexedIndirect, DrawIndirectCount};
use penguin_util::GpuBuffer;
use penguin_util::GpuBufferDeviceExt;

use crate::layer::application_layer::Time;
pub use resources::*;

mod resources {
    use super::*;
    use penguin_util::impl_deref;

    // cpu side
    // ------------------

    pub struct Meshes(pub Vec<mesh::Mesh>);
    impl_deref!(mut Meshes, Vec<mesh::Mesh>);

    pub struct RenderObjects {
        pub render_objects: HandleMap<RenderObject>,
        pub should_rebuild_batches: bool,
        pub render_objects_to_reupload: Vec<Handle<RenderObject>>,
        pub forward_pass: mesh_pass::MeshPass,
    }

    /// The max value for possible draw commands (max draw count read from the draw count buffer)
    pub struct MaxDrawCount(pub u32);
    impl_deref!(mut MaxDrawCount, u32);

    // gpu side
    // ----------------

    /// The render_objects array in RenderObjects, uploaded to GPU memory.
    pub struct RenderObjectsBuffer {
        pub buffer: GpuBuffer<RenderObject>,
    }
    // todo: Separate instances (model matrices) from the RenderObject buffer.

    /// Buffers for draw commands
    pub struct DrawCommandBuffers {
        /// Batched draw commands with instance count set to 0. Batches are built CPU-side and
        /// uploaded to this buffer.
        pub clear_buffer: GpuBuffer<DrawIndexedIndirect>,
        /// Buffer that the compute shader fills with draw commands, and instance counts.
        pub out_buffer: GpuBuffer<DrawIndexedIndirect>,
    }

    /// Buffer that maps each instance index in the DrawCommandBuffers::out_buffer to a render object.
    /// Filled in the compute shader.
    pub struct InstanceIndexToRenderObjectMapBuffer {
        pub buffer: GpuBuffer<u32>, // instance index u32 -> Handle<RenderObject>
    }

    /// Buffers containing the count of number of draw calls to draw.
    pub struct DrawCountBuffers {
        /// Buffer with the draw count set to 0. Used to reset the buffer.
        pub clear_buffer: GpuBuffer<DrawIndirectCount>,
        /// Buffer containing the draw count. Set by the compute shader.
        pub buffer: GpuBuffer<DrawIndirectCount>,
    }

    /// Data local to the compute shader
    pub struct ComputeShaderDataBuffers {
        pub clear_buffer: GpuBuffer<DrawOutputInfo>,
        pub buffer: GpuBuffer<DrawOutputInfo>,
        pub buffer_size: usize,
    }

    pub struct RenderInstanceBuffer {
        pub buffer: GpuBuffer<RenderInstance>,
    }
}

pub struct BaseRenderSceneLayer<'a> {
    pub window: &'a winit::window::Window,
    pub mesh_assets: &'a [&'a str],
}

impl Layer for BaseRenderSceneLayer<'_> {
    fn init(self, cmd: &mut CommandBuffer, r: &mut Resources) {
        // todo: Move context to another layer, it doesn't make sense here
        let context = penguin_util::pollster::block_on(GraphicsContext::new(&self.window));
        let device = &context.device;

        let draw_commands = DrawCommandBuffers::init(device, MAX_DRAW_COMMANDS);
        let draw_counts = DrawCountBuffers::init(device);

        let instances = RenderInstanceBuffer::init(device, MAX_DRAW_COMMANDS);
        let instances_to_render_objects = InstanceIndexToRenderObjectMapBuffer::init(device);
        let local_shader_storage = ComputeShaderDataBuffers::init(device, MAX_DRAW_COMMANDS);

        // -------
        let mesh_assets = r.get::<scene_layer::MeshAssets>().unwrap();
        let (vertex_array_buffer, meshes) =
            mesh::VertexArrayBuffer::build_from_mesh_assets(device, &mesh_assets);
        drop(mesh_assets);
        r.remove::<scene_layer::MeshAssets>();
        // -----

        let render_objects_buffer = RenderObjectsBuffer::init(device, MAX_DRAW_COMMANDS);
        let render_objects = RenderObjects::default();

        // base
        r.insert(context);
        r.insert(draw_commands);
        r.insert(draw_counts);
        r.insert(MaxDrawCount(0));
        r.insert(instances);
        r.insert(instances_to_render_objects);
        r.insert(local_shader_storage);

        // render objects
        r.insert(vertex_array_buffer);
        r.insert(Meshes(meshes));
        r.insert(render_objects_buffer);
        r.insert(render_objects);
    }

    fn startup_steps() -> Option<Vec<Step>> {
        Some(startup::steps())
    }

    fn run_steps() -> Option<Vec<Step>> {
        Some(
            enqueue_transform_updates::steps()
                .into_iter()
                .chain(
                    Schedule::builder()
                        .add_system(build_batches_system())
                        .add_system(reupload_updated_objects_system())
                        .build()
                        .into_vec(),
                )
                .collect::<Vec<_>>(),
        )
    }
}

mod startup {
    use super::*;
    use crate::components::{MeshComponent, Rotation};
    use legion::world::SubWorld;
    use legion::IntoQuery;

    pub fn steps() -> Vec<Step> {
        Schedule::builder()
            .add_system(register_render_objects_system())
            .build()
            .into_vec()
    }

    #[system(for_each)]
    fn register_render_objects(
        cmd: &mut legion::systems::CommandBuffer,
        entity: &Entity,
        mesh: &MeshComponent,
        #[resource] render_objects: &mut RenderObjects,
    ) {
        let render_obj_desc = RenderObjectDescriptor {
            mesh_handle: Handle::from(mesh.0),
            transform: m::Mat4::IDENTITY,
            render_bounds: mesh::RenderBounds {
                origin: m::Vec3::ZERO,
                radius: 3.0,
            },
            draw_forward_pass: true,
        };

        let render_obj_handle = render_objects.register_object(&render_obj_desc);

        println!("registering render object {} for entity: {:?} --------------------------------------------------", render_obj_handle.id, entity);

        cmd.add_component(*entity, render_obj_handle);
    }
}

/// Builds batches of draw commands and uploads them into the draw commands buffer
#[system]
fn build_batches(
    #[resource] context: &GraphicsContext,
    #[resource] render_objs: &mut RenderObjects,
    #[resource] draw_commands: &DrawCommandBuffers,
    #[resource] max_draw_count: &mut MaxDrawCount,
    #[resource] meshes: &Meshes,
) {
    let queue = &context.queue;

    if render_objs
        .forward_pass
        .update_batches(&render_objs.render_objects)
    {
        println!("building batches.. ------------------------------------- ");

        // create a draw call for each unique mesh + material combo
        let indirect_commands = render_objs
            .forward_pass
            .indirect_batches
            .iter()
            .map(|batch: &IndirectBatch| {
                let mesh = meshes[batch.mesh_h.id as usize];
                println!("mesh: {:?}, max instance count: {}", mesh, batch.count);

                let first_instance = batch.first as _;
                let instance_count = 0; // set in compute shader
                mesh.create_draw_command(first_instance, instance_count)
            })
            .collect::<Vec<_>>();

        // assign draw commands to render objects
        render_objs
            .forward_pass
            .objects
            .inner
            .iter()
            .for_each(|pass_object: &PassObject| {
                let render_object = pass_object.original_render_object;

                render_objs.render_objects[render_object].draw_command_index =
                    pass_object.draw_command_id;

                render_objs.render_objects_to_reupload.push(render_object);
            });

        queue.write_buffer(
            &draw_commands.clear_buffer,
            0,
            bytemuck::cast_slice(&indirect_commands),
        );

        // update max draw count
        max_draw_count.0 = indirect_commands.len() as _;

        println!(
            "indirect commands ------------------: {}",
            indirect_commands.len()
        );
    }
}

#[system]
fn reupload_updated_objects(
    #[resource] context: &GraphicsContext,
    #[resource] render_objects: &mut RenderObjects,
    #[resource] render_objects_buffer: &RenderObjectsBuffer,
) {
    let queue = &context.queue;

    while let Some(render_object_handle) = render_objects.render_objects_to_reupload.pop() {
        let offset = mem::size_of::<RenderObject>() * render_object_handle.id as usize;
        let render_object_data = render_objects.render_objects[render_object_handle];

        queue.write_buffer(
            &render_objects_buffer.buffer,
            offset as _,
            bytemuck::cast_slice(slice::from_ref(&render_object_data)),
        );
    }
}

impl Default for RenderObjects {
    fn default() -> Self {
        Self {
            render_objects: HandleMap::new(),
            should_rebuild_batches: true,
            render_objects_to_reupload: Vec::new(),
            forward_pass: mesh_pass::MeshPass::new(),
        }
    }
}

impl RenderObjects {
    pub fn register_object(&mut self, desc: &RenderObjectDescriptor) -> Handle<RenderObject> {
        let render_object: Handle<RenderObject> = self.render_objects.push(RenderObject {
            mesh: desc.mesh_handle,
            transform: desc.transform,
            draw_command_index: 0,
        });

        if desc.draw_forward_pass {
            self.forward_pass.unbatched_objects.push(render_object);
        }

        // this render object's data will need to be updated in GPU memory.
        self.render_objects_to_reupload.push(render_object);

        render_object
    }

    pub fn enqueue_model_matrix_update(
        &mut self,
        render_object: Handle<RenderObject>,
        model_matrix: m::Mat4,
    ) {
        self.render_objects[render_object].transform = model_matrix;
        self.render_objects_to_reupload.push(render_object);
    }
}

impl RenderObjectsBuffer {
    pub fn init(device: &wgpu::Device, max_render_objects: usize) -> Self {
        let buffer = device.create_buffer_t::<RenderObject>(&wgpu::BufferDescriptor {
            label: Some("render objects buffer"),
            size: (mem::size_of::<RenderObject>() * max_render_objects) as _,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self { buffer }
    }
}

impl DrawCommandBuffers {
    pub fn init(device: &wgpu::Device, max_draw_commands: usize) -> Self {
        let size =
            (std::mem::size_of::<DrawIndexedIndirect>() * max_draw_commands) as wgpu::BufferAddress;

        let usage = wgpu::BufferUsages::INDIRECT
            | wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST;

        let clear_buffer = device.create_buffer_t::<DrawIndexedIndirect>(&wgpu::BufferDescriptor {
            label: Some("draw indirect buffer"),
            size,
            usage,
            mapped_at_creation: false,
        });

        let buffer = device.create_buffer_t::<DrawIndexedIndirect>(&wgpu::BufferDescriptor {
            label: Some("draw indirect buffer"),
            size,
            usage: wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            clear_buffer,
            out_buffer: buffer,
        }
    }
}

impl DrawCountBuffers {
    pub fn init(device: &wgpu::Device) -> Self {
        let contents = bytemuck::cast_slice(slice::from_ref(&DrawIndirectCount { count: 0 }));

        let usage = wgpu::BufferUsages::INDIRECT
            | wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST;

        let clear_buffer =
            device.create_buffer_init_t::<DrawIndirectCount>(&wgpu::util::BufferInitDescriptor {
                label: Some("draw indirect count buffer"),
                contents,
                usage: usage | wgpu::BufferUsages::COPY_SRC,
            });

        let buffer =
            device.create_buffer_init_t::<DrawIndirectCount>(&wgpu::util::BufferInitDescriptor {
                label: Some("draw indirect count buffer"),
                contents,
                usage,
            });

        Self {
            clear_buffer,
            buffer,
        }
    }

    /// Resets the buffer's draw count to 0.
    pub fn reset(&self, cmd: &mut wgpu::CommandEncoder) {
        cmd.copy_buffer_to_buffer(
            &self.clear_buffer,
            0,
            &self.buffer,
            0,
            mem::size_of::<DrawIndirectCount>() as _,
        );
    }
}

impl ComputeShaderDataBuffers {
    pub fn init(device: &wgpu::Device, max_draw_commands: usize) -> Self {
        let contents = (0..max_draw_commands)
            .map(|_| DrawOutputInfo::default())
            .collect::<Vec<_>>();

        let clear_buffer =
            device.create_buffer_init_t::<DrawOutputInfo>(&wgpu::util::BufferInitDescriptor {
                label: Some("compute shader local data buffer"),
                contents: bytemuck::cast_slice(&contents),
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            });

        let buffer =
            device.create_buffer_init_t::<DrawOutputInfo>(&wgpu::util::BufferInitDescriptor {
                label: Some("compute shader local data buffer"),
                contents: bytemuck::cast_slice(&contents),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });

        Self {
            clear_buffer,
            buffer,
            buffer_size: std::mem::size_of::<DrawOutputInfo>() * max_draw_commands,
        }
    }

    pub fn reset(&self, cmd: &mut wgpu::CommandEncoder) {
        cmd.copy_buffer_to_buffer(
            &self.clear_buffer,
            0,
            &self.buffer,
            0,
            self.buffer_size as _,
        );
    }
}

impl RenderInstanceBuffer {
    pub fn init(device: &wgpu::Device, max_instances: usize) -> Self {
        let instances = (0..max_instances)
            .map(|_| RenderInstance {
                render_object_id: Handle::from(0),
            })
            .collect::<Vec<_>>();

        let buffer =
            device.create_buffer_init_t::<RenderInstance>(&wgpu::util::BufferInitDescriptor {
                label: Some("instance buffer"),
                contents: bytemuck::cast_slice(&instances),
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::VERTEX,
            });

        Self { buffer }
    }
}

impl InstanceIndexToRenderObjectMapBuffer {
    pub fn init(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer_init_t::<u32>(&wgpu::util::BufferInitDescriptor {
            label: Some("final draw command indices"),
            contents: bytemuck::cast_slice(
                &(0..MAX_DRAW_COMMANDS).map(|_| 0_u32).collect::<Vec<_>>(),
            ),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        Self { buffer }
    }
}
