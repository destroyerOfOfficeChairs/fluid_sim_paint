// Texture A (Read Only)
@group(0) @binding(0) var input_texture: texture_2d<f32>;

// Texture B (Write Only)
// Note: rgba8unorm is a standard format for storage textures
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    // 1. Get dimensions
    let dims = textureDimensions(output_texture);
    
    // 2. Safety check: Don't write outside the texture
    if (id.x >= dims.x || id.y >= dims.y) {
        return;
    }

    // 3. Read the old color
    // We use '0' for the mipmap level
    let old_color = textureLoad(input_texture, vec2<i32>(i32(id.x), i32(id.y)), 0);

    // 4. Fade logic: Multiply by 0.99
    // We keep Alpha at 1.0 so we can always see the result
    let new_color = vec4<f32>(old_color.rgb * 0.99, 1.0);

    // 5. Write back
    textureStore(output_texture, vec2<i32>(i32(id.x), i32(id.y)), new_color);
}
