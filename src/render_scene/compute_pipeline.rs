use crate::{camera, render_scene};
use std::mem;

pub const BIND_GROUP_LAYOUT_DESC: wgpu::BindGroupLayoutDescriptor =
    wgpu::BindGroupLayoutDescriptor {
        label: Some("compute bind group layout"),
        entries: &[
            // camera uniform
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        mem::size_of::<camera::CameraUniform>() as _,
                    ),
                },
                count: None,
            },
            // draw commands buffer (draw indirect buffer)
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // render objects buffer
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // compute local data buffer
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // draw count buffer
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // out draw commands
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // instance index to render object map
            wgpu::BindGroupLayoutEntry {
                binding: 6,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    };

pub fn bind_group_entries<'a>(
    uniform_buffer: &'a wgpu::Buffer,
    scene: &'a render_scene::RenderScene,
) -> [wgpu::BindGroupEntry<'a>; 7] {
    [
        // camera
        wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        },
        // draw commands
        wgpu::BindGroupEntry {
            binding: 1,
            resource: scene.draw_commands_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 2,
            resource: scene.render_objects_buffer.as_entire_binding(),
        },
        // compute shader local data
        wgpu::BindGroupEntry {
            binding: 3,
            resource: scene.compute_shader_local_data_buffer.as_entire_binding(),
        },
        // draw count
        wgpu::BindGroupEntry {
            binding: 4,
            resource: scene.draw_count_buffer.as_entire_binding(),
        },
        // out draw commands
        wgpu::BindGroupEntry {
            binding: 5,
            resource: scene.out_draw_commands_buffer.as_entire_binding(),
        },
        // instance index to render object map
        wgpu::BindGroupEntry {
            binding: 6,
            resource: scene
                .instance_index_to_render_object_map
                .as_entire_binding(),
        },
    ]
}
