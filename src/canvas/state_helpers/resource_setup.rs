use crate::canvas::texture::Texture;

pub fn create_ping_pong_textures(device: &wgpu::Device, queue: &wgpu::Queue) -> (Texture, Texture) {
    // We hardcode the simulation size here (or you could pass it in)
    let sim_width = 512;
    let sim_height = 512;

    // 1. Create the textures
    let texture_a =
        Texture::create_storage_texture(device, sim_width, sim_height, Some("Texture A"));
    let texture_b =
        Texture::create_storage_texture(device, sim_width, sim_height, Some("Texture B"));

    // 2. Generate Random Noise for Texture A
    // We fill Texture A so we have something to fade immediately
    let mut initial_data = Vec::with_capacity((sim_width * sim_height * 4) as usize);
    for _ in 0..(sim_width * sim_height) {
        let r: u8 = rand::random();
        let g: u8 = rand::random();
        let b: u8 = rand::random();
        initial_data.extend_from_slice(&[r, g, b, 255]);
    }

    // 3. Upload to GPU
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture_a.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &initial_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * sim_width),
            rows_per_image: Some(sim_height),
        },
        wgpu::Extent3d {
            width: sim_width,
            height: sim_height,
            depth_or_array_layers: 1,
        },
    );

    (texture_a, texture_b)
}
