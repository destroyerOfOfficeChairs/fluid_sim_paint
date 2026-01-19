use crate::canvas::texture::Texture;

pub fn create_ping_pong_textures(
    device: &wgpu::Device,
    _queue: &wgpu::Queue, // <--- Rename to _queue since we don't use it anymore
    sim_width: u32,
    sim_height: u32,
) -> (Texture, Texture) {
    // 1. Create the textures (Start as black/empty)
    let texture_a =
        Texture::create_storage_texture(device, sim_width, sim_height, Some("Texture A"));
    let texture_b =
        Texture::create_storage_texture(device, sim_width, sim_height, Some("Texture B"));

    // We don't need to upload data anymore because the Compute Shader
    // calculates the color for every pixel on the very first frame.

    (texture_a, texture_b)
}
