use arrayvec::ArrayVec;
use downcast_rs::Downcast;
use std::mem::MaybeUninit;

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

pub trait DeviceExt {
    fn bind_group_layout_short<'a>(
        &self,
        entries: &'a [wgpu::BindGroupLayoutEntry],
    ) -> wgpu::BindGroupLayout;
}
impl DeviceExt for wgpu::Device {
    fn bind_group_layout_short<'a>(
        &self,
        entries: &'a [wgpu::BindGroupLayoutEntry],
    ) -> wgpu::BindGroupLayout {
        self.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries,
        })
    }
}

/// Marker trait to implement bind_group_layout_entry function for storage buffers

pub enum StorageType {
    Uniform,
    Storage,
}

pub struct BindGroupLayoutBuilder<const COUNT: usize> {
    data: ArrayVec<wgpu::BindGroupLayoutEntry, COUNT>,
}
impl<const COUNT: usize> BindGroupLayoutBuilder<COUNT> {
    pub fn builder() -> Self {
        Self {
            data: arrayvec::ArrayVec::new(),
        }
    }

    pub fn uniform_buffer(mut self, binding: u32, visibility: wgpu::ShaderStages) -> Self {
        self.data.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
        self
    }

    pub fn storage_buffer(
        mut self,
        binding: u32,
        visibility: wgpu::ShaderStages,
        read_only: bool,
    ) -> Self {
        self.data.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
        self
    }

    const TEXTURE_BINDING_TYPE: wgpu::BindingType = wgpu::BindingType::Texture {
        multisampled: false,
        view_dimension: wgpu::TextureViewDimension::D2,
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
    };

    pub fn texture_2d(mut self, binding: u32, visibility: wgpu::ShaderStages) -> Self {
        self.data.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: Self::TEXTURE_BINDING_TYPE,
            count: None,
        });
        self
    }

    pub fn sampler(mut self, binding: u32, visibility: wgpu::ShaderStages) -> Self {
        self.data.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        self
    }

    pub fn build(self, device: &wgpu::Device, label: Option<&str>) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label,
            entries: &self.data,
        })
    }
}

pub struct BindGroupBuilder<'a, const COUNT: usize> {
    data: ArrayVec<wgpu::BindGroupEntry<'a>, COUNT>,
}
impl<'a, const COUNT: usize> BindGroupBuilder<'a, COUNT> {
    pub fn builder() -> Self {
        Self {
            data: arrayvec::ArrayVec::new(),
        }
    }

    fn insert(mut self, binding: u32, resource: wgpu::BindingResource<'a>) -> Self {
        self.data.push(wgpu::BindGroupEntry { binding, resource });
        self
    }

    pub fn buffer(mut self, binding: u32, buffer: &'a wgpu::Buffer) -> Self {
        self.insert(binding, buffer.as_entire_binding())
    }

    pub fn texture_view(mut self, binding: u32, texture_view: &'a wgpu::TextureView) -> Self {
        self.insert(binding, wgpu::BindingResource::TextureView(texture_view))
    }

    pub fn sampler(mut self, binding: u32, sampler: &'a wgpu::Sampler) -> Self {
        self.insert(binding, wgpu::BindingResource::Sampler(sampler))
    }

    pub fn build(
        self,
        device: &wgpu::Device,
        label: Option<&str>,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout,
            entries: &self.data,
        })
    }
}

pub fn uniform_buffer_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
pub fn storage_buffer_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    read_only: bool,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

pub fn buffer_bind_group_entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

// pub fn texture_2d_layout_entry(
//     binding: u32,
//     visibility: wgpu::ShaderStages,
// ) -> wgpu::BindGroupLayoutEntry {
//     wgpu::BindGroupLayoutEntry {
//         binding,
//         visibility,
//         ty: TEXTURE_BINDING_TYPE,
//         count: None,
//     }
// }
//
// pub fn sampler_layout_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
//     wgpu::BindGroupLayoutEntry {
//         binding,
//         visibility,
//         ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
//         count: None,
//     }
// }

// pub trait StorageBufferTrait {
//     fn bind_group_layout_entry(binding: u32, visibility: wgpu::ShaderStages, read_only: bool) -> wgpu::BindGroupLayoutEntry {
//         wgpu::BindGroupLayoutEntry {
//             binding,
//             visibility,
//             ty: wgpu::BindingType::Buffer {
//                 ty: wgpu::BufferBindingType::Storage { read_only },
//                 has_dynamic_offset: false,
//                 min_binding_size: None,
//             },
//             count: None,
//         }
//     }
// }
// pub trait UniformBufferTrait {
//     fn bind_group_layout_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
//         wgpu::BindGroupLayoutEntry {
//             binding,
//             visibility,
//             ty: wgpu::BindingType::Buffer {
//                 ty: wgpu::BufferBindingType::Uniform,
//                 has_dynamic_offset: false,
//                 min_binding_size: None,
//             },
//             count: None,
//         }
//     }
// }
//
