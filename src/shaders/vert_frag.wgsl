// vertex ---------------------------------------------
// input -----------
struct VertexInput {
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] normal: vec3<f32>;
    [[location(2)]] uv: vec2<f32>;
};

struct InstanceInput {
//     [[location(5)]] model_mat_0: vec4<f32>;
//     [[location(6)]] model_mat_1: vec4<f32>;
//     [[location(7)]] model_mat_2: vec4<f32>;
//     [[location(8)]] model_mat_3: vec4<f32>;
    [[builtin(instance_index)]] index: u32;
};

struct CameraUniform {
    view_proj: mat4x4<f32>;
};

struct RenderObject {
    mesh_handle: u32;
    transform: mat4x4<f32>;
    draw_command_index: u32;
};

struct RenderObjectsStorage {
    data: array<RenderObject>;
};

struct InstanceIndexToRenderObjectMapStorage {
    data: array<u32>;
};

[[group(0), binding(0)]] var<uniform> camera: CameraUniform;
[[group(0), binding(1)]] var<storage, read> render_objects: RenderObjectsStorage;
[[group(0), binding(2)]] var<storage, read> instance_index_to_render_object_id: InstanceIndexToRenderObjectMapStorage;

// output ----
struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] uv: vec2<f32>;
};

// vertex main -----
[[stage(vertex)]]
fn vs_main(vert: VertexInput, inst: InstanceInput) -> VertexOutput {
    let render_object_id = instance_index_to_render_object_id.data[inst.index];
    let model_matrix = render_objects.data[render_object_id].transform;


    var out: VertexOutput;
    out.uv = vert.uv;
    out.clip_position = camera.view_proj * model_matrix * vec4<f32>(vert.position, 1.0);

    return out;
}

// fragment ---------------------------------------------------

// input ---------
[[group(1), binding(0)]] var t_diffuse: texture_2d<f32>;
[[group(1), binding(1)]] var s_diffuse: sampler;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.uv);
}
