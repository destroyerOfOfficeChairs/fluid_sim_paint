struct SimParams {
    time: f32,
    pad1: f32,
    pad2: f32,
    pad3: f32,
};

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> params: SimParams; // <--- The Time Machine

// Pseudo-random hash function
fn hash(pos: vec2<u32>) -> f32 {
    var p = vec2<f32>(pos);
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = textureDimensions(output_texture);
    if (id.x >= dims.x || id.y >= dims.y) {
        return;
    }

    // 1. Calculate a "Breathing" factor (-1.0 to 1.0)
    let breathe = sin(params.time);

    // 2. Generate fresh noise for this pixel
    let noise = hash(id.xy);

    // 3. Mix between Black (0.0) and Noise (1.0) based on time
    // We map sine from (-1, 1) to (0, 1)
    let strength = (breathe + 1.0) * 0.5;
    
    let pixel_color = vec3<f32>(noise * strength);

    textureStore(output_texture, vec2<i32>(i32(id.x), i32(id.y)), vec4<f32>(pixel_color, 1.0));
}
