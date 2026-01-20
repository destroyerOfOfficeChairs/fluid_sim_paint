struct SimParams {
    time: f32,
    dt: f32,
    pad1: f32,
    pad2: f32,
};

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> params: SimParams;
@group(0) @binding(3) var input_velocity: texture_2d<f32>;      // Read Old Velocity
@group(0) @binding(4) var s_linear: sampler;
@group(0) @binding(5) var output_velocity: texture_storage_2d<rg32float, write>; // Write New Velocity

fn hash(pos: vec2<u32>) -> f32 {
    var p = vec2<f32>(pos);
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    // 1. DEFINE COORDS FIRST (Fixing your error)
    let coords = vec2<i32>(id.xy);
    let dims = vec2<f32>(textureDimensions(output_texture));

    // Bounds Check
    if (u32(coords.x) >= u32(dims.x) || u32(coords.y) >= u32(dims.y)) {
        return;
    }

    // 2. NOW we can use coords safely
    let velocity = textureLoad(input_velocity, coords, 0).xy;

    // 3. Calculate the "Advection Source" (Where did the fluid come from?)
    let pos_current = vec2<f32>(coords);
    let pos_old = pos_current - (velocity * params.dt);
    let uv = pos_old / dims;

    // 4. Advect DENSITY (The Color)
    var new_color = textureSampleLevel(input_texture, s_linear, uv, 0.0).rgb;

    // 5. Advect VELOCITY (The Wind moves itself!)
    var new_velocity = textureSampleLevel(input_velocity, s_linear, uv, 0.0).xy;

    // 6. Apply Damping (Friction)
    new_velocity = new_velocity * 0.99;

    // 7. Inject Noise/Forces
    let breathe = (sin(params.time) + 1.0) * 0.5;
    let noise = hash(id.xy);
    let center = dims * 0.5;
    let dist = distance(pos_current, center);
    
    if (dist < 20.0) {
        new_color += vec3<f32>(noise * breathe);
        // Inject a little velocity to keep it moving
        new_velocity += vec2<f32>(0.05, 0.02) * breathe;
    }

    // 8. Write Output
    new_color *= 0.99; // Fade color
    
    textureStore(output_texture, coords, vec4<f32>(new_color, 1.0));
    textureStore(output_velocity, coords, vec4<f32>(new_velocity, 0.0, 0.0));
}
