use crate::camera::{CameraUniformData, MainCamera};
use crate::layer::application_layer::Time;
use crate::{
    camera, mesh, texture, DrawOutputInfo, GraphicsContext, Layer, RenderInstance, Vertex,
    VertexArrayBuffer, MAX_DRAW_COMMANDS,
};
use legion::systems::{CommandBuffer, Step};
use legion::{Resources, Schedule};
use std::marker::PhantomData;
use std::{iter, mem, slice};

use crate::bind_groups;
use crate::bind_groups::{
    buffer_bind_group_entry, storage_buffer_layout_entry, uniform_buffer_layout_entry, DeviceExt,
};
use crate::components::Translation;
use crate::layer::base_render_scene_layer::{
    ComputeShaderDataBuffers, DrawCommandBuffers, DrawCountBuffers,
    InstanceIndexToRenderObjectMapBuffer, MaxDrawCount, RenderInstanceBuffer, RenderObjects,
    RenderObjectsBuffer,
};
use crate::render_scene::RenderObject;
use legion::system;
use penguin_util::handle::Handle;
use penguin_util::raw_gpu_types::{DrawIndexedIndirect, DrawIndirectCount};
use penguin_util::{GpuBuffer, GpuBufferDeviceExt};
use wgpu::{BindGroup, BindGroupLayoutEntry, ShaderStages};

// todo texture arrays

/// Data related to a compute pass.
struct Compute {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group: wgpu::BindGroup,
}

/// Data related to a render pass.
struct Render {
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_shader_bind_group: wgpu::BindGroup,
    pub fragment_shader_bind_group: wgpu::BindGroup,
}

pub struct PipelinesLayer;
impl Layer for PipelinesLayer {
    fn init(self, cmd: &mut CommandBuffer, r: &mut Resources) {
        log::warn!("TEST!");

        let context = r.get::<GraphicsContext>().unwrap();
        let device = &context.device;
        let queue = &context.queue;
        let config = &context.config;



        // --------
        let main_camera = MainCamera::init(config); // todo: Maybe remake into an entity
        let uniform_buffer = UniformBuffer::init(device, &main_camera.uniform_data);

        // -------
        const READ: bool = true;
        const READ_WRITE: bool = false;
        const VERTEX: wgpu::ShaderStages = wgpu::ShaderStages::VERTEX;
        const FRAGMENT: wgpu::ShaderStages = wgpu::ShaderStages::FRAGMENT;
        const COMPUTE: wgpu::ShaderStages = wgpu::ShaderStages::COMPUTE;
        // -------

        let (vertex_group, fragment_group, render_pipeline_layout) = {
            // vertex -----------
            let (vertex_bind_group_layout, vertex_bind_group) = {
                let vertex_bind_group_layout = bind_groups::BindGroupLayoutBuilder::<3>::builder()
                    .uniform_buffer(0, VERTEX) // camera uniform
                    .storage_buffer(1, VERTEX, READ) // render objects
                    .storage_buffer(2, VERTEX, READ) // instance_index to render_object map
                    .build(device, Some("vertex bind group layout"));

                let render_objects = r.get::<RenderObjectsBuffer>().unwrap();
                let instance_map =
                    r.get::<InstanceIndexToRenderObjectMapBuffer>().unwrap();

                let vertex_bind_group = bind_groups::BindGroupBuilder::<3>::builder()
                    .buffer(0, &uniform_buffer.buffer)
                    .buffer(1, &render_objects.buffer)
                    .buffer(2, &instance_map.buffer)
                    .build(device, Some("vertex bind group"), &vertex_bind_group_layout);

                (vertex_bind_group_layout, vertex_bind_group)
            };

            // fragment ------------
            let (fragment_bind_group_layout, fragment_bind_group) = {
                let cube_texture =
                    texture::Texture::from_asset(device, queue, "cube-diffuse.jpg").unwrap();

                let fragment_bind_group_layout =
                    bind_groups::BindGroupLayoutBuilder::<2>::builder()
                        .texture_2d(0, FRAGMENT)
                        .sampler(1, FRAGMENT)
                        .build(device, Some("fragment bind group layout"));

                let fragment_bind_group = bind_groups::BindGroupBuilder::<2>::builder()
                    .texture_view(0, &cube_texture.view)
                    .sampler(1, &cube_texture.sampler)
                    .build(
                        device,
                        Some("fragment bind group"),
                        &fragment_bind_group_layout,
                    );

                (fragment_bind_group_layout, fragment_bind_group)
            };

            // render pipeline layout -----------
            let render_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("render pipeline layout"),
                    bind_group_layouts: &[
                        &vertex_bind_group_layout,   // group 0
                        &fragment_bind_group_layout, // group 1
                    ],
                    push_constant_ranges: &[],
                });

            (
                vertex_bind_group,
                fragment_bind_group,
                render_pipeline_layout,
            )
        };

        // compute
        let (compute_group, compute_pipeline_layout) = {
            let compute_bind_group_layout = bind_groups::BindGroupLayoutBuilder::<7>::builder()
                .uniform_buffer(0, COMPUTE)
                .storage_buffer(1, COMPUTE, READ)
                .storage_buffer(2, COMPUTE, READ)
                .storage_buffer(3, COMPUTE, READ_WRITE)
                .storage_buffer(4, COMPUTE, READ_WRITE)
                .storage_buffer(5, COMPUTE, READ_WRITE)
                .storage_buffer(6, COMPUTE, READ_WRITE)
                .build(device, Some("compute bind group layout"));

            let draw_commands = r.get::<DrawCommandBuffers>().unwrap();
            let render_objects = r.get::<RenderObjectsBuffer>().unwrap();
            let shader_local = r.get::<ComputeShaderDataBuffers>().unwrap();
            let draw_count = r.get::<DrawCountBuffers>().unwrap();
            let instance_map = r.get::<InstanceIndexToRenderObjectMapBuffer>().unwrap();

            let compute_bind_group = bind_groups::BindGroupBuilder::<7>::builder()
                .buffer(0, &uniform_buffer.buffer)
                .buffer(1, &draw_commands.clear_buffer)
                .buffer(2, &render_objects.buffer)
                .buffer(3, &shader_local.buffer)
                .buffer(4, &draw_count.buffer)
                .buffer(5, &draw_commands.out_buffer)
                .buffer(6, &instance_map.buffer)
                .build(
                    device,
                    Some("compute bind group"),
                    &compute_bind_group_layout,
                );

            let compute_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("compute pipeline layout"),
                    bind_group_layouts: &[&compute_bind_group_layout],
                    push_constant_ranges: &[],
                });

            (compute_bind_group, compute_pipeline_layout)
        };

        let render_pipeline = {
            // --------
            let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/vert_frag.wgsl").into()),
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            })
        };

        let compute_pipeline = {
            let compute_shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("compute shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/compute.wgsl").into()),
            });

            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("compute pipeline"),
                layout: Some(&compute_pipeline_layout),
                module: &compute_shader,
                entry_point: "cs_main",
            })
        };

        drop(context);

        r.insert(main_camera);
        r.insert(uniform_buffer);
        r.insert(Render {
            pipeline: render_pipeline,
            vertex_shader_bind_group: vertex_group,
            fragment_shader_bind_group: fragment_group,
        });
        r.insert(Compute {
            pipeline: compute_pipeline,
            bind_group: compute_group,
        });
    }

    fn startup_steps() -> Option<Vec<Step>> {
        None
    }

    fn run_steps() -> Option<Vec<Step>> {
        Some(
            uniform_buffer::steps().into_iter()
                .chain(
                    Schedule::builder()
                        .add_system(compute_commands_system())
                        .add_system(render_commands_system())
                        .build()
                        .into_vec(),
                ).collect::<Vec<_>>()
        )
    }
}

use uniform_buffer::*;
mod uniform_buffer {
    use super::*;
    use macaw as m;

    pub struct UniformBuffer {
        pub buffer: GpuBuffer<CameraUniformData>,
    }

    pub fn steps() -> Vec<Step> {
        Schedule::builder()
            .add_system(update_main_camera_system())
            .add_system(enqueue_uniform_buffer_write_system())
            .build()
            .into_vec()
    }

    #[system]
    fn update_main_camera(
        #[resource] main_camera: &mut MainCamera,
        #[resource] dt: &Time,
    ) {
        main_camera.update(dt.delta_time());
    }

    #[system]
    fn enqueue_uniform_buffer_write(
        #[resource] context: &GraphicsContext,
        #[resource] uniform_buffer: &UniformBuffer,
        #[resource] editor_camera: &MainCamera,
    ) {
        let queue = &context.queue;

        queue.write_buffer(
            &uniform_buffer.buffer,
            0,
            bytemuck::cast_slice(slice::from_ref(&editor_camera.uniform_data)),
        );
    }

    impl UniformBuffer {
        pub fn init(device: &wgpu::Device, camera_uniform_data: &CameraUniformData) -> Self {
            let buffer = device.create_buffer_init_t::<camera::CameraUniformData>(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("camera uniform buffer"),
                    contents: bytemuck::cast_slice(slice::from_ref(camera_uniform_data)),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                },
            );

            Self { buffer }
        }
    }
}

#[system]
fn compute_commands(
    #[resource] context: &GraphicsContext,
    #[resource] compute_local: &ComputeShaderDataBuffers,
    #[resource] draw_counts: &DrawCountBuffers,
    #[resource] compute: &Compute,
    #[resource] render_objs: &RenderObjects,
) {
    let device = &context.device;
    let queue = &context.queue;

    let mut cmd = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("compute commands encoder"),
    });

    cmd.push_debug_group("compute pass");
    {
        compute_local.reset(&mut cmd);
        draw_counts.reset(&mut cmd);

        let mut compute_pass = cmd.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("compute pass"),
        });
        compute_pass.set_pipeline(&compute.pipeline);
        compute_pass.set_bind_group(0, &compute.bind_group, &[]);
        compute_pass.dispatch(render_objs.render_objects.inner.len() as _, 1, 1);
    }
    cmd.pop_debug_group();

    queue.submit(iter::once(cmd.finish()));
}


#[system]
fn render_commands(
    #[resource] context: &GraphicsContext,
    #[resource] render: &Render,
    #[resource] vertex_array_buffer: &VertexArrayBuffer,
    #[resource] instances: &RenderInstanceBuffer,
    #[resource] draw_commands: &DrawCommandBuffers,
    #[resource] draw_counts: &DrawCountBuffers,
    #[resource] max_draw_count: &MaxDrawCount,
) {
    /// Access the output view texture to submit render commands.
    fn render_func<OutputTextureFunc: FnOnce(&wgpu::TextureView)>(
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

    let device = &context.device;
    let queue = &context.queue;

    // todo: Respond to result, reconfigure surface if needed.
    let _render_result = render_func(&context, |output| {
        let mut cmd = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("compute commands encoder"),
        });

        cmd.push_debug_group("render pass");
        {
            let mut render_pass = cmd.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &output,
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
                    view: &context.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            // set render pipeline
            render_pass.set_pipeline(&render.pipeline);

            // set bind groups
            render_pass.set_bind_group(0, &render.vertex_shader_bind_group, &[]);
            render_pass.set_bind_group(1, &render.fragment_shader_bind_group, &[]);

            // set vertex/index buffer
            render_pass.set_vertex_buffer(0, vertex_array_buffer.vertices_slice());
            render_pass.set_index_buffer(
                vertex_array_buffer.indices_slice(),
                wgpu::IndexFormat::Uint32,
            );
            // set instance buffer
            render_pass.set_vertex_buffer(1, instances.buffer.slice(..));

            // draw
            render_pass.multi_draw_indexed_indirect_count(
                &draw_commands.out_buffer,
                0,
                &draw_counts.buffer,
                0,
                max_draw_count.0,
            );
        }
        cmd.pop_debug_group();

        queue.submit(iter::once(cmd.finish()));
    });
}
