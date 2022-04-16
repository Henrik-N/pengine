use crate::render_scene::mesh_pass;
use anyhow::*;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::system::Commands;
use macaw as m;
use penguin_util::raw_gpu_types::DrawIndexedIndirect;
use std::mem;
use std::ops::Range;
use wgpu::util::DeviceExt;

#[repr(C, align(4))]
#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub position: m::Vec3,
    pub normal: m::Vec3,
    pub uv: m::Vec2,
}
unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

// Ranges in an uploaded vertex/index buffer that represent a mesh
#[derive(Debug)]
pub struct MeshDefinition {
    pub first_vertex: u32,
    pub vertex_count: u32,
    pub first_index: u32,
    pub index_count: u32,
}

/// Uploaded meshes vertices and indices data
#[derive(Debug)]
pub struct VertexArrayBuffer {
    pub buffer: wgpu::Buffer,
    vertices_byte_range: u64,
}

/// Mesh data loaded into memory (CPU-side memory / RAM).
#[derive(Debug)]
pub struct MeshAsset {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x2,
    ];

    pub fn buffer_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as _,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

impl MeshDefinition {
    /// Creates a draw command using this mesh.
    pub fn create_draw_command(
        &self,
        first_instance: u32,
        instance_count: u32,
    ) -> DrawIndexedIndirect {
        DrawIndexedIndirect {
            index_count: self.index_count,
            instance_count,
            first_index: self.first_index,
            base_vertex: self.first_vertex,
            first_instance,
        }
    }
}

impl VertexArrayBuffer {
    /// Returns the slice of the vertex array buffer that contains the vertices.
    pub fn vertices_slice(&self) -> wgpu::BufferSlice {
        self.buffer.slice(..self.vertices_byte_range as u64)
    }

    /// Returns the slice of the vertex array buffer that contains the indices.
    pub fn indices_slice(&self) -> wgpu::BufferSlice {
        self.buffer.slice(self.vertices_byte_range as u64..)
    }

    /// Takes a list of mesh asset names and uploads their vertices and indices into a single,
    /// continuous, gpu buffer. Returns a handle to the allocated buffer and an array of meshes.
    ///
    /// The location of each mesh in the returned array corresponds to the location of the mesh
    /// asset name in the input mesh_asset_names array.
    pub fn build_from_mesh_assets(
        device: &wgpu::Device,
        mesh_asset_names: &[&str],
    ) -> (Self, Vec<MeshDefinition>) {
        let assets_dir = std::path::Path::new(env!("OUT_DIR")).join("assets/meshes");

        let mut next_first_vertex = 0;
        let mut next_first_index = 0;

        let mut meshes = Vec::with_capacity(mesh_asset_names.len());

        log::trace!("starts loading meshes...");

        let (vertices, indices): (Vec<Vec<Vertex>>, Vec<Vec<u32>>) = mesh_asset_names
            .iter()
            .map(|mesh_name| {
                let MeshAsset { vertices, indices } =
                    MeshAsset::load_obj(assets_dir.join(mesh_name))
                        .expect(&format!("failed to load {}", mesh_name));

                let mesh = MeshDefinition {
                    first_vertex: next_first_vertex,
                    vertex_count: vertices.len() as _,
                    first_index: next_first_index,
                    index_count: indices.len() as _,
                };
                log::trace!("loaded mesh {}: {:?}", mesh_name, mesh);
                meshes.push(mesh);

                next_first_vertex += vertices.len() as u32;
                next_first_index += indices.len() as u32;

                (vertices, indices)
            })
            .unzip();

        println!("\n");

        let vertices = vertices.into_iter().flatten().collect::<Vec<_>>();
        let indices = indices.into_iter().flatten().collect::<Vec<_>>();

        let vertices_bytes: &[u8] = bytemuck::cast_slice(&vertices);
        let indices_bytes: &[u8] = bytemuck::cast_slice(&indices);
        let vertices_byte_range = vertices_bytes.len();

        log::trace!("mesh loading complete");

        let vertex_array_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex index buffer"),
            contents: &[vertices_bytes, indices_bytes].concat(),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
        });

        (
            Self {
                buffer: vertex_array_buffer,
                vertices_byte_range: vertices_byte_range as u64,
            },
            meshes,
        )
    }
}

impl MeshAsset {
    /// Loads an obj file's vertices and indices into memory.
    pub fn load_obj<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let (shapes, _materials) = tobj::load_obj(
            path.as_ref(),
            &tobj::LoadOptions {
                single_index: true,
                triangulate: true,
                ignore_points: false,
                ignore_lines: false,
            },
        )?;

        let mut vertices: Vec<Vec<Vertex>> = Vec::new();
        let mut indices: Vec<Vec<u32>> = Vec::new();

        let mut next_vertex_index_begin = 0;

        for shape in shapes.iter() {
            let shape_verts = (0..shape.mesh.positions.len() / 3)
                .map(|vertex_index| Vertex {
                    position: m::Vec3::from_slice(
                        &shape.mesh.positions[vertex_index * 3..=vertex_index * 3 + 2],
                    ),
                    normal: m::Vec3::from_slice(
                        &shape.mesh.normals[vertex_index * 3..=vertex_index * 3 + 2],
                    ),
                    uv: m::Vec2::from_slice(
                        &shape.mesh.texcoords[vertex_index * 2..=vertex_index * 2 + 1],
                    ),
                })
                .collect::<Vec<_>>();

            let shape_inds = shape
                .mesh
                .indices
                .iter()
                .map(|index| next_vertex_index_begin + index)
                .collect::<Vec<_>>();

            next_vertex_index_begin += shape.mesh.positions.len() as u32;

            vertices.push(shape_verts);
            indices.push(shape_inds);
        }

        let vertices = vertices.into_iter().flatten().collect::<Vec<Vertex>>();
        let indices = indices.into_iter().flatten().collect::<Vec<u32>>();

        Ok(Self { vertices, indices })
    }
}

// end of meshes -------------------------------

// render scene ---------------
pub struct MeshAssetsToLoad {
    pub mesh_asset_names: &'static [&'static str],
}

pub struct RenderScene;

impl Plugin for RenderScene {
    fn build(&self, app: &mut App) {
        app.insert_resource(MeshAssetsToLoad {
            mesh_asset_names: &["cube.obj", "cone.obj"],
        });
    }
}

/// Batches mesh/material combos together into IndirectBatches that can be used to create draw commands
struct DrawBatcher(mesh_pass::LegacyMeshPass);

/// Meshpass: information needed to render a pass of meshes for a part of the renderer.
/// A mesh pass holds it's own buffer of batched draw commands, but it shares a common vertex array
/// buffer with other mesh passes.
struct ForwardMeshPass {
    batcher: DrawBatcher,
    compute_bind_group: wgpu::BindGroup,
    compute_pipeline: wgpu::ComputePipeline,
    vertex_bind_group: wgpu::BindGroup,
    fragment_bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
}
