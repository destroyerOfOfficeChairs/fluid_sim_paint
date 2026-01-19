use crate::canvas::texture::Texture;

pub fn create_ping_pong_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue, // <--- Bring this back
    sim_width: u32,
    sim_height: u32,
) -> (Texture, Texture, Texture, Texture) {
    // Return 4 textures now!

    // 1. DENSITY (Color) - Rgba8Unorm
    let texture_a = Texture::create_storage_texture(
        device,
        sim_width,
        sim_height,
        wgpu::TextureFormat::Rgba8Unorm,
        Some("Density A"),
    );
    let texture_b = Texture::create_storage_texture(
        device,
        sim_width,
        sim_height,
        wgpu::TextureFormat::Rgba8Unorm,
        Some("Density B"),
    );

    // 2. VELOCITY (Movement) - Rg16Float (Float allows negatives!)
    let velocity_a = Texture::create_storage_texture(
        device,
        sim_width,
        sim_height,
        wgpu::TextureFormat::Rg32Float,
        Some("Velocity A"),
    );
    let velocity_b = Texture::create_storage_texture(
        device,
        sim_width,
        sim_height,
        wgpu::TextureFormat::Rg32Float,
        Some("Velocity B"),
    );

    // 3. INITIALIZE WIND
    let vel_x: f32 = 1.0;
    let vel_y: f32 = 0.5;

    let mut velocity_data = Vec::with_capacity((sim_width * sim_height * 8) as usize); // 2 floats * 4 bytes
    for _ in 0..(sim_width * sim_height) {
        velocity_data.extend_from_slice(&vel_x.to_ne_bytes());
        velocity_data.extend_from_slice(&vel_y.to_ne_bytes());
    }

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &velocity_a.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &velocity_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(sim_width * 8), // 2 floats * 4 bytes = 8 bytes per pixel
            rows_per_image: Some(sim_height),
        },
        wgpu::Extent3d {
            width: sim_width,
            height: sim_height,
            depth_or_array_layers: 1,
        },
    );

    (texture_a, texture_b, velocity_a, velocity_b)
}
