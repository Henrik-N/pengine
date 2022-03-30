use wgpu::BufferDescriptor;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

/// Typed wgpu::Buffer for more readable code.
pub struct GpuBuffer<T> {
    pub inner: wgpu::Buffer,
    _marker: std::marker::PhantomData<T>
}

impl<T> From<wgpu::Buffer> for GpuBuffer<T> {
    fn from(buffer: wgpu::Buffer) -> Self {
        Self {
            inner: buffer,
            _marker: std::marker::PhantomData,
        }
    }
}
impl<T> std::ops::Deref for GpuBuffer<T> {
    type Target = wgpu::Buffer;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Extention methods for wgpu::Device.
pub trait GpuBufferDeviceExt {
    fn create_buffer_t<T>(&self, desc: &wgpu::BufferDescriptor<'_>) -> GpuBuffer<T>;
    fn create_buffer_init_t<T>(&self, desc: &wgpu::util::BufferInitDescriptor<'_>) -> GpuBuffer<T>;
}

impl GpuBufferDeviceExt for wgpu::Device {
    /// Creates a typed wgpu::Buffer.
    fn create_buffer_t<T>(&self, desc: &BufferDescriptor<'_>) -> GpuBuffer<T> {
        GpuBuffer::<T>::from(self.create_buffer(desc))
    }

    /// Creates and initializes a typed wgpu::Buffer.
    fn create_buffer_init_t<T>(&self, desc: &BufferInitDescriptor<'_>) -> GpuBuffer<T> {
        GpuBuffer::<T>::from(self.create_buffer_init(desc))
    }
}
