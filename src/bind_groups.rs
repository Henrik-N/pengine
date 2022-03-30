pub mod layout_entry {
    pub mod texture {
        const TEXTURE_BINDING_TYPE: wgpu::BindingType = wgpu::BindingType::Texture {
            multisampled: false,
            view_dimension: wgpu::TextureViewDimension::D2,
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
        };

        pub fn texture_2d(
            binding: u32,
            visibility: wgpu::ShaderStages,
        ) -> wgpu::BindGroupLayoutEntry {
            wgpu::BindGroupLayoutEntry {
                binding,
                visibility,
                ty: TEXTURE_BINDING_TYPE,
                count: None,
            }
        }

        pub fn sampler(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
            wgpu::BindGroupLayoutEntry {
                binding,
                visibility,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            }
        }

        #[allow(unused)]
        pub fn texture_2d_array(
            binding: u32,
            visibility: wgpu::ShaderStages,
            array_len: u32,
        ) -> wgpu::BindGroupLayoutEntry {
            wgpu::BindGroupLayoutEntry {
                binding,
                visibility,
                ty: TEXTURE_BINDING_TYPE,
                count: std::num::NonZeroU32::new(array_len),
            }
        }
    }
}
