///! This module contains structs that stores the data and handles to GPU data that is used to render a scene.
pub mod compute_pipeline;
mod mesh_pass;

use crate::render_scene::mesh_pass::{IndirectBatch, PassObject};
use crate::{mesh, RenderInstance, VertexArrayBuffer};
use macaw as m;
use penguin_util as util;
use penguin_util::handle::HandleMap;
use penguin_util::{
    handle::Handle,
    raw_gpu_types::{DrawIndexedIndirect, DrawIndirectCount},
    GpuBufferDeviceExt,
};
use std::{mem, slice};
use util::GpuBuffer;

/// Describes a render object entry to add to the render scene.
pub struct RenderObjectDescriptor {
    /// The mesh ID is corresponding to a mesh asset's index in the mesh_assets array on RenderScene
    /// creation.
    pub mesh_id: usize,
    /// The initial transform of this object.
    pub transform: m::Mat4,
    /// The render bounds of this object (for culling).
    pub render_bounds: mesh::RenderBounds,
    /// Weather this mesh object should be drawn in the forward rendering mesh pass.
    pub draw_forward_pass: bool,
    // other mesh pass..
    // other mesh pass..
}

/// Data for an object in the scene.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct RenderObject {
    pub mesh: Handle<mesh::Mesh>,
    // material: usize,
    pub transform: m::Mat4,
    // pub render_bounds: mesh::RenderBounds,
    draw_command_index: u32, // todo Should actually just be in PassObject
}
unsafe impl bytemuck::Pod for RenderObject {}
unsafe impl bytemuck::Zeroable for RenderObject {}

pub const MAX_DRAW_COMMANDS: usize = 100;

/// Stores the data, and handles to GPU data, that is used to render a scene.
/// All mesh passes will keep the same object data for culling and object transform.
pub struct RenderScene {
    //
    // ----- Mesh data ----------------------
    //
    /// Vertex array buffer containing all of the meshes vertices and indices.
    pub vertex_array_buffer: VertexArrayBuffer,
    /// Representation of each mesh in the vertex array buffer.
    meshes: Vec<mesh::Mesh>,
    // --------------------------------------
    //
    //
    // ---- Object data --------------------------------
    //
    /// Data for each render object that doesn't change per mesh pass, such as the transform and
    /// render bounds for culling.
    pub render_objects: HandleMap<RenderObject>,
    /// The render objects array in GPU-memory.
    pub render_objects_buffer: GpuBuffer<RenderObject>,
    /// Render objects that need to be reuploaded to the GPU.
    render_objects_to_update: Vec<Handle<RenderObject>>,
    //
    pub instance_buffer: GpuBuffer<RenderInstance>,
    // --------------------------------------
    //
    //
    // ----- Draw buffers -------------------
    //
    /// Buffer containing all the batched draw calls, with instance count set to 0.
    pub draw_commands_buffer: GpuBuffer<DrawIndexedIndirect>,
    /// Buffer of final draw commands needed for the frame (set by the compute shader).
    pub out_draw_commands_buffer: GpuBuffer<DrawIndexedIndirect>,
    /// Buffer with draw_count set to 0, used to reset draw_count_buffer.
    pub clear_draw_count_buffer: GpuBuffer<DrawIndirectCount>,
    /// Buffer containing the number of draw commands to issue this frame (filled by the compute shader).
    pub draw_count_buffer: GpuBuffer<DrawIndirectCount>,
    // ---------------------------------------
    //
    //
    pub max_draw_count: u32,
    //
    pub instance_index_to_render_object_map: GpuBuffer<u32>,
    //
    pub clear_compute_shader_local_data_buffer: GpuBuffer<DrawOutputInfo>,
    pub compute_shader_local_data_buffer: GpuBuffer<DrawOutputInfo>,

    /// Mesh pass for forward rendering.
    forward_pass: mesh_pass::MeshPass,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
/// Data local to the compute shader helping to determine where an invoked draw command should
/// be placed in the output draw commands buffer.
pub struct DrawOutputInfo {
    /// Weather the draw command at this index has been given an output draw command slot.
    /// A value of 0 indicates false.
    has_output_slot: u32,
    /// The slot the draw command at this index has been assigned to in the output draw commands
    /// buffer. A value of u32::MAX (4294967295_u32) indicates that the output slot is unset.
    output_slot: u32,
}
impl std::default::Default for DrawOutputInfo {
    fn default() -> Self {
        Self {
            has_output_slot: 0,
            output_slot: u32::MAX,
        }
    }
}

impl RenderScene {
    /// Creates a new render scene with the specified mesh assets.
    pub fn new(device: &wgpu::Device, mesh_assets: &[&str]) -> Self {
        // mesh data buffers --------------
        let (vertex_array_buffer, meshes) =
            mesh::VertexArrayBuffer::build_from_mesh_assets(&device, mesh_assets);

        // draw indirect buffers ---------------
        //
        let (draw_commands_buffer, out_draw_commands_buffer) =
            create_draw_indirect_buffers(&device, MAX_DRAW_COMMANDS);

        // draw count buffers -----------------
        //
        let (clear_draw_count_buffer, draw_count_buffer) = create_draw_count_buffers(device);

        // render object buffer -------------------
        //
        let render_objects_buffer = create_render_objects_buffer(device, MAX_DRAW_COMMANDS);

        // instance buffers -------------------
        //
        let instance_buffer = create_instance_buffer(device, MAX_DRAW_COMMANDS);

        // ----------------
        let instance_index_to_render_object_map =
            device.create_buffer_init_t::<u32>(&wgpu::util::BufferInitDescriptor {
                label: Some("final draw command indices"),
                contents: bytemuck::cast_slice(
                    &(0..MAX_DRAW_COMMANDS).map(|_| 0_u32).collect::<Vec<_>>(),
                ),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });

        let (clear_compute_shader_local_data_buffer, compute_shader_local_data_buffer) =
            create_compute_shader_local_data_buffers(device, MAX_DRAW_COMMANDS);

        Self {
            vertex_array_buffer,
            draw_commands_buffer,
            out_draw_commands_buffer,
            clear_draw_count_buffer,
            draw_count_buffer,
            meshes,
            render_objects: HandleMap::new(),
            render_objects_buffer,
            render_objects_to_update: Vec::new(),
            forward_pass: mesh_pass::MeshPass::new(),
            max_draw_count: 0,
            instance_buffer,
            instance_index_to_render_object_map,
            clear_compute_shader_local_data_buffer,
            compute_shader_local_data_buffer,
        }
    }

    /// Adds a RenderObject to the scene and adds it to the listed mesh passes.
    pub fn register_object(&mut self, desc: &RenderObjectDescriptor) -> Handle<RenderObject> {
        let mesh_handle = if self.meshes.get(desc.mesh_id).is_some() {
            Handle::<mesh::Mesh>::from(desc.mesh_id)
        } else {
            panic!("no mesh with id {} in the render scene", desc.mesh_id)
        };

        let render_object: Handle<RenderObject> = self.render_objects.push(RenderObject {
            mesh: mesh_handle,
            transform: desc.transform,
            draw_command_index: 0,
        });

        if desc.draw_forward_pass {
            self.forward_pass.unbatched_objects.push(render_object);
        }

        // this render object's data will need to be updated in GPU memory.
        self.render_objects_to_update.push(render_object);

        render_object
    }

    pub fn update_transform_model_matrix(
        &mut self,
        render_object: Handle<RenderObject>,
        transform: m::Mat4,
    ) {
        // todo RwLock?
        self.render_objects[render_object].transform = transform;

        self.render_objects_to_update.push(render_object);
    }

    /// Update GPU memory with any newly submitted render object data.
    pub fn update(&mut self, queue: &wgpu::Queue) {
        while let Some(render_object) = self.render_objects_to_update.pop() {
            let offset = mem::size_of::<RenderObject>() * render_object.id as usize;
            let render_object_data = self.render_objects[render_object];

            queue.write_buffer(
                &self.render_objects_buffer,
                offset as _,
                bytemuck::cast_slice(slice::from_ref(&render_object_data)),
            );
        }
    }

    pub fn build_batches(&mut self, queue: &wgpu::Queue) {
        if self.forward_pass.update_batches(&self.render_objects) {
            println!("building batches..");

            // create a draw call for each unique mesh + material combo
            let indirect_commands = self
                .forward_pass
                .indirect_batches
                .iter()
                .map(|batch: &IndirectBatch| {
                    let mesh = self.meshes[batch.mesh_h.id as usize];
                    println!("mesh: {:?}, max instance count: {}", mesh, batch.count);

                    let first_instance = batch.first as _;
                    let instance_count = 0; // set in compute shader
                    mesh.create_draw_command(first_instance, instance_count)
                })
                .collect::<Vec<_>>();

            // assign draw commands to render objects
            self.forward_pass
                .objects
                .inner
                .iter()
                .for_each(|pass_object: &PassObject| {
                    let render_object = pass_object.original_render_object;

                    self.render_objects[render_object].draw_command_index =
                        pass_object.draw_command_id;

                    self.render_objects_to_update.push(render_object);
                });

            queue.write_buffer(
                &self.draw_commands_buffer,
                0,
                bytemuck::cast_slice(&indirect_commands),
            );

            // update max draw count
            self.max_draw_count = indirect_commands.len() as _;
        }
    }
}

fn create_draw_indirect_buffers(
    device: &wgpu::Device,
    max_draw_commands: usize,
) -> (
    GpuBuffer<DrawIndexedIndirect>,
    GpuBuffer<DrawIndexedIndirect>,
) {
    let size =
        (std::mem::size_of::<DrawIndexedIndirect>() * max_draw_commands) as wgpu::BufferAddress;

    let usage =
        wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;

    let draw_indirect_buffer =
        device.create_buffer_t::<DrawIndexedIndirect>(&wgpu::BufferDescriptor {
            label: Some("draw indirect buffer"),
            size,
            usage,
            mapped_at_creation: false,
        });

    let out_draw_commands_buffer =
        device.create_buffer_t::<DrawIndexedIndirect>(&wgpu::BufferDescriptor {
            label: Some("draw indirect buffer"),
            size: (std::mem::size_of::<DrawIndexedIndirect>() * MAX_DRAW_COMMANDS) as _,
            usage: wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

    (draw_indirect_buffer, out_draw_commands_buffer)
}

fn create_draw_count_buffers(
    device: &wgpu::Device,
) -> (GpuBuffer<DrawIndirectCount>, GpuBuffer<DrawIndirectCount>) {
    let contents = bytemuck::cast_slice(slice::from_ref(&DrawIndirectCount { count: 0 }));

    let usage =
        wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;

    let clear_draw_count_buffer =
        device.create_buffer_init_t::<DrawIndirectCount>(&wgpu::util::BufferInitDescriptor {
            label: Some("draw indirect count buffer"),
            contents,
            usage: usage | wgpu::BufferUsages::COPY_SRC,
        });
    let draw_count_buffer =
        device.create_buffer_init_t::<DrawIndirectCount>(&wgpu::util::BufferInitDescriptor {
            label: Some("draw indirect count buffer"),
            contents,
            usage,
        });

    (clear_draw_count_buffer, draw_count_buffer)
}

fn create_render_objects_buffer(
    device: &wgpu::Device,
    max_render_objects: usize,
) -> GpuBuffer<RenderObject> {
    device.create_buffer_t::<RenderObject>(&wgpu::BufferDescriptor {
        label: Some("render objects buffer"),
        size: (mem::size_of::<RenderObject>() * max_render_objects) as _,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_instance_buffer(
    device: &wgpu::Device,
    max_instances: usize,
) -> GpuBuffer<RenderInstance> {
    let instances = (0..max_instances)
        .map(|_| RenderInstance {
            render_object_id: Handle::from(0),
            // model: m::Mat4::IDENTITY,
        })
        .collect::<Vec<_>>();

    let instance_buffer =
        device.create_buffer_init_t::<RenderInstance>(&wgpu::util::BufferInitDescriptor {
            label: Some("instance buffer"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::VERTEX,
        });

    instance_buffer
}

fn create_compute_shader_local_data_buffers(
    device: &wgpu::Device,
    max_draw_commands: usize,
) -> (GpuBuffer<DrawOutputInfo>, GpuBuffer<DrawOutputInfo>) {
    let contents = (0..max_draw_commands)
        .map(|_| DrawOutputInfo::default())
        .collect::<Vec<_>>();

    let clear_compute_shader_local_data_buffer =
        device.create_buffer_init_t::<DrawOutputInfo>(&wgpu::util::BufferInitDescriptor {
            label: Some("compute shader local data buffer"),
            contents: bytemuck::cast_slice(&contents),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });

    let compute_shader_local_data_buffer =
        device.create_buffer_init_t::<DrawOutputInfo>(&wgpu::util::BufferInitDescriptor {
            label: Some("compute shader local data buffer"),
            contents: bytemuck::cast_slice(&contents),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

    (
        clear_compute_shader_local_data_buffer,
        compute_shader_local_data_buffer,
    )
}
