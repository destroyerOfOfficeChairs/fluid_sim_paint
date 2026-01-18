@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = textureDimensions(output_texture);
    if (id.x >= dims.x || id.y >= dims.y) {
        return;
    }

    let old_color = textureLoad(input_texture, vec2<i32>(i32(id.x), i32(id.y)), 0);

    let decay = 0.005; 
    let new_rgb = max(old_color.rgb * 0.99 - decay, vec3(0.0));

    let new_color = vec4<f32>(new_rgb, 1.0);

    textureStore(output_texture, vec2<i32>(i32(id.x), i32(id.y)), new_color);
}
