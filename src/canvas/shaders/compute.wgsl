struct SimParams {
    time: f32,
    dt: f32,
    pad1: f32,
    pad2: f32,
};

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> params: SimParams;
@group(0) @binding(3) var velocity_texture: texture_2d<f32>; // The Wind
@group(0) @binding(4) var s_linear: sampler;                 // Smooth sampler

fn hash(pos: vec2<u32>) -> f32 {
    var p = vec2<f32>(pos);
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = vec2<f32>(textureDimensions(output_texture));
    let coords = vec2<i32>(id.xy);

    if (u32(coords.x) >= u32(dims.x) || u32(coords.y) >= u32(dims.y)) {
        return;
    }

    // 1. Get Velocity at this pixel
    // We use textureLoad because we want the exact velocity at this grid cell
    let velocity = textureLoad(velocity_texture, coords, 0).xy;

    // 2. Advection Logic (Trace Backwards)
    // "Where was the particle that is currently here?"
    // pos_old = pos_current - velocity * dt
    let pos_current = vec2<f32>(coords);
    let pos_old = pos_current - (velocity * params.dt);

    // 3. Sample Density at the old position
    // We must normalize to UV coordinates (0.0 to 1.0) for textureSampleLevel
    let uv = pos_old / dims;
    
    // Use Level 0 (highest detail). We need a sampler for smooth movement!
    var new_color = textureSampleLevel(input_texture, s_linear, uv, 0.0).rgb;

    // 4. Inject Noise (The "Source")
    // Keep the breathing noise from before so we have something to advect!
    let breathe = (sin(params.time) + 1.0) * 0.5;
    let noise = hash(id.xy);
    
    // Only add noise in the center to look like a "smoke emitter"
    let center = dims * 0.5;
    let dist = distance(pos_current, center);
    if (dist < 20.0) {
        new_color += vec3<f32>(noise * breathe);
    }

    // 5. Fade out (Damping)
    new_color *= 0.99;

    textureStore(output_texture, coords, vec4<f32>(new_color, 1.0));
}
