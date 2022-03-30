///! Raw types to be submitted to the API.

/// Struct to be submitted to wgpu to execute indexed draw indirect commands.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawIndexedIndirect {
    /// Number of vertices to draw.
    pub index_count: u32,
    /// Number of instances to draw
    pub instance_count: u32,
    /// The base index within the index buffer.
    pub first_index: u32,
    /// The value added to the vertex index before indexing into the vertex buffer.
    pub base_vertex: u32,
    /// The instance id of the first instance to draw.
    pub first_instance: u32,
}

/// Struct to be submitted to wgpu to specify the number of draws when submitting the command
/// 'multi_draw_indirect_count'.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawIndirectCount {
    /// The number of times the corresponding draw command should be executed.
    pub count: u32,
}

/// Struct to be submitted to wgpu to execute draw indirect commands.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawIndirect {
    /// Number of vertices to draw
    pub vertex_count: u32,
    /// Number of instances to draw
    pub instance_count: u32,
    /// The index of the first vertex to draw
    pub first_vertex: u32,
    /// The instance id of the first instance to be drawn.
    pub first_instance: u32,
}
