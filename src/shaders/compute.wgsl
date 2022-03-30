// 0
//
struct CameraUniform {
    view_proj: mat4x4<f32>;
};

// 1
//
struct DrawIndexedIndirect {
    index_count: u32;
    instance_count: atomic<u32>;
    first_index: u32;
    base_vertex: u32;
    first_instance: u32;
};
struct DrawCommandsStorage {
    data: array<DrawIndexedIndirect>;
};

// 2
//
struct DrawIndirectCount {
    count: atomic<u32>;
};
struct DrawIndirectCountStorage {
    data: array<DrawIndirectCount>;
};

// 3
//
struct DrawOutputInfo {
    has_output_slot: atomic<u32>;
    output_slot: atomic<u32>;
};
struct DrawOutputInfoStorage {
    data: array<DrawOutputInfo>;
};

// 4
//
struct RenderObject {
    mesh_handle: u32;
    transform: mat4x4<f32>;
    draw_command_index: u32;
    // todo: render_bounds: RenderBounds, (for culling)
};
struct RenderObjectsStorage {
    data: array<RenderObject>;
};

// 5, 6
//
struct AtomicU32Storage {
    data: array<atomic<u32> >; // todo: instance index -> render object map
};


// unused, but plan to use for culling
[[group(0), binding(0)]] var<uniform> camera: CameraUniform;

// IN
//
// prebuilt draw commands
[[group(0), binding(1)]] var<storage, read> draw_commands: DrawCommandsStorage;
// render objects (to get the draw call ids from)
[[group(0), binding(2)]] var<storage, read> render_objects: RenderObjectsStorage;


// LOCAL
//
// data local to the compute shader
[[group(0), binding(3)]] var<storage, read_write> output_info: DrawOutputInfoStorage;


// OUT
//
// draw count
[[group(0), binding(4)]] var<storage, read_write> draw_counts: DrawIndirectCountStorage;
// output draw commands, the draw commands that will be executed
[[group(0), binding(5)]] var<storage, read_write> out_draw_commands: DrawCommandsStorage;
// instances
[[group(0), binding(6)]] var<storage, read_write> instance_index_to_render_object_map: AtomicU32Storage;

fn isVisible(render_object: RenderObject) -> bool {
    // todo frustum culling
    // todo occlusion culling
    return true;
}

[[stage(compute), workgroup_size(1)]]
fn cs_main([[builtin(global_invocation_id)]] gid: vec3<u32>) {
    let render_object_id = gid.x;

    let render_object = render_objects.data[render_object_id];
    let draw_command_index = render_object.draw_command_index;

    if (isVisible(render_object)) {
        // check if this draw call is already in the output draw buffer
        let is_draw_invoked = atomicAdd(&output_info.data[draw_command_index].has_output_slot, 1u);

        var output_slot: u32;
        if (is_draw_invoked < 1u) {
            // this is the first time this draw command is invoked, give it a new draw command slot
            output_slot = atomicAdd(&draw_counts.data[0].count, 1u);

            // assign draw command to output draw command slot
            out_draw_commands.data[output_slot] = draw_commands.data[draw_command_index];

            // store the output slot
            atomicStore(&output_info.data[draw_command_index].output_slot, output_slot);

        } else {
            // this is not the first time this draw command is invoked

            let value_indicating_not_set = 4294967295u; // u32::MAX
            loop {
                // spin until the output slot is actually set on the first "thread" this draw command was invoked in
                output_slot = atomicLoad(&output_info.data[draw_command_index].output_slot);

                if (output_slot != value_indicating_not_set) {
                    break;
                }
            }
        }

        // get the index of the instance within the draw command
        let instance_slot = atomicAdd(&out_draw_commands.data[output_slot].instance_count, 1u);

        // get the index of the instance in the global instance array
        let instance_index = out_draw_commands.data[output_slot].first_instance + instance_slot;

        // map instance to be drawn to the render object (for use in the vertex shader)
        instance_index_to_render_object_map.data[instance_index] = render_object_id;
    }
}
