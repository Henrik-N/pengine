use crate::texture;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_window::{WindowResized, Windows};
use penguin_util::pollster;

pub struct DepthTexture(pub texture::Texture);

pub struct GraphicsContextPlugin;

impl Plugin for GraphicsContextPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(init_graphics_context.system())
            .add_system(on_window_resize.system());
    }
}

pub fn init_graphics_context(mut cmd: Commands, windows: Res<Windows>) {
    let window = windows
        .get_primary()
        .expect("Failed to get window. You're probably missing WinitPlugin.");
    let instance = wgpu::Instance::new(wgpu::Backends::VULKAN);

    let surface = unsafe {
        let raw_window_handle = window.raw_window_handle().get_handle();
        instance.create_surface(&raw_window_handle)
    };

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .expect("no supported gpu");

    let (device, queue) = pollster::block_on(adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features:
                //wgpu::Features::default(), // wgpu::Features::BUFFER_BINDING_ARRAY,
                //wgpu::Features::default(), // wgpu::Features::BUFFER_BINDING_ARRAY,
                // wgpu::Features::POLYGON_MODE_LINE |
                // allow non-zero value for first_instance field in draw calls
                //wgpu::Features::INDIRECT_FIRST_INSTANCE |
                //wgpu::Features::TEXTURE_BINDING_ARRAY |
                // wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY,
                wgpu::Features::all() ^ wgpu::Features::TEXTURE_COMPRESSION_ETC2 ^ wgpu::Features::TEXTURE_COMPRESSION_ASTC_LDR ^ wgpu::Features::VERTEX_ATTRIBUTE_64BIT,
                limits: wgpu::Limits::default(),
            },
            None,
        ))
        .expect("failed to init device, missing required features?");

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface.get_preferred_format(&adapter).unwrap(),
        width: window.physical_width(),
        height: window.physical_height(),
        present_mode: wgpu::PresentMode::Mailbox,
    };
    surface.configure(&device, &config);

    let depth_texture = DepthTexture(texture::Texture::create_depth_texture(&device, &config));

    cmd.insert_resource(instance);
    cmd.insert_resource(surface);
    cmd.insert_resource(adapter);
    cmd.insert_resource(device);
    cmd.insert_resource(queue);
    cmd.insert_resource(config);
    cmd.insert_resource(depth_texture);
}

fn on_window_resize(
    device: Res<wgpu::Device>,
    surface: Res<wgpu::Surface>,
    mut config: ResMut<wgpu::SurfaceConfiguration>,
    mut events: EventReader<WindowResized>,
) {
    if let Some(e) = events.iter().last() {
        config.width = e.width as _;
        config.height = e.height as _;

        surface.configure(&device, &config);
    }
}
