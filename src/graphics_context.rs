use crate::{events, texture};

/// Graphics API handles and window/surface size data.
pub struct GraphicsContext {
    /// Platform-specific surface that rendered images are presented to.
    pub surface: wgpu::Surface,
    /// Physical device, usually a dedicated gpu.
    pub adapter: wgpu::Adapter,
    /// Logical device, a connection to physical device.
    pub device: wgpu::Device,
    /// Commands queue on the device
    pub queue: wgpu::Queue,
    /// Configuration for the surface.
    pub config: wgpu::SurfaceConfiguration,
    /// Window size excluding the window's borders and title bar.
    pub size: winit::dpi::PhysicalSize<u32>,
    /// Window scale factor.
    pub scale_factor: f64,
    /// The depth texture.
    pub depth_texture: texture::Texture,
}
impl GraphicsContext {
    pub async fn new(window: &winit::window::Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::Backends::VULKAN);
        let surface = unsafe { instance.create_surface(window) };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("no supported gpu");

        let (device, queue) = adapter
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
            )
            .await
            .expect("failed to init device, missing required features?");

        assert_ne!(size.width, 0);
        assert_ne!(size.height, 0);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_preferred_format(&adapter).unwrap(),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Mailbox,
        };
        surface.configure(&device, &config);

        let scale_factor = window.scale_factor();

        let depth_texture = texture::Texture::create_depth_texture(&device, &config);

        Self {
            surface,
            adapter,
            device,
            queue,
            config,
            size,
            scale_factor,
            depth_texture,
        }
    }

    fn on_resize(&mut self, size: winit::dpi::PhysicalSize<u32>, scale_factor: Option<f64>) {
        assert_ne!(size.width, 0);
        assert_ne!(size.height, 0);

        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);

        if let Some(scale_factor) = scale_factor {
            self.scale_factor = scale_factor;
        }

        self.depth_texture = texture::Texture::create_depth_texture(&self.device, &self.config);
    }

    pub fn on_event(&mut self, event: &events::PenguinEvent) -> bool {
        use events::{event::WindowResizeEvent, PenguinEvent};

        match event {
            PenguinEvent::Window(WindowResizeEvent { size, scale_factor }) => {
                self.on_resize(*size, *scale_factor);
                false
            }
            _ => false,
        }
    }
}
