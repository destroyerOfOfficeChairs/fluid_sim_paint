pub struct Texture {
    #[allow(unused)]
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub fn create_storage_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        label: Option<&str>,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format, // Use the passed argument
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
        }
    }
}

pub fn create_sim_textures(
    device: &wgpu::Device,
    sim_width: u32,
    sim_height: u32,
) -> (
    Texture,
    Texture,
    Texture,
    Texture,
    Texture,
    Texture,
    Texture,
    Texture,
) {
    let closure = |name: &str, texture_type: wgpu::TextureFormat| {
        Texture::create_storage_texture(device, sim_width, sim_height, texture_type, Some(name))
    };
    let density_a = closure("Density A", wgpu::TextureFormat::Rgba32Float);
    let density_b = closure("Density B", wgpu::TextureFormat::Rgba32Float);
    let velocity_a = closure("Velocity A", wgpu::TextureFormat::Rg32Float);
    let velocity_b = closure("Velocity B", wgpu::TextureFormat::Rg32Float);
    let pressure_a = closure("Pressure A", wgpu::TextureFormat::R32Float);
    let pressure_b = closure("Pressure B", wgpu::TextureFormat::R32Float);
    let divergence = closure("Pressure A", wgpu::TextureFormat::R32Float);
    let temp_density = closure("Temp Density", wgpu::TextureFormat::Rgba32Float);
    (
        density_a,
        density_b,
        velocity_a,
        velocity_b,
        pressure_a,
        pressure_b,
        divergence,
        temp_density,
    )
}
