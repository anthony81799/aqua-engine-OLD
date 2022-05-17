use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub position: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding: u32,
    pub color: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding2: u32,
}

impl LightUniform {
    pub fn new() -> Self {
        return LightUniform {
            position: [2.0, 2.0, 2.0],
            _padding: 0,
            color: [1.0, 1.0, 1.0],
            _padding2: 0,
        };
    }
}

pub fn create_buffer_init(device: &wgpu::Device, light_uniform: LightUniform) -> wgpu::Buffer {
    return device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Light VB"),
        contents: bytemuck::cast_slice(&[light_uniform]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
}

pub fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    return device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
        label: None,
    });
}

pub fn create_bind_group(
    device: &wgpu::Device,
    light_bind_group_layout: &wgpu::BindGroupLayout,
    light_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    return device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: light_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: light_buffer.as_entire_binding(),
        }],
        label: None,
    });
}
