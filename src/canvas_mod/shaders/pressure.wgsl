struct PressureUniforms {
    width: f32,
    height: f32,
    dt: f32, // Not strictly needed for pure projection, but good to have
};

@group(0) @binding(0) var<uniform> params: PressureUniforms;

// --- DIVERGENCE PIPELINE ---
@group(0) @binding(1) var velocity_in: texture_2d<f32>;
@group(0) @binding(2) var divergence_out: texture_storage_2d<r32float, write>;

@compute @workgroup_size(16, 16)
fn divergence_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let coords = vec2<i32>(id.xy);
    let dims = vec2<i32>(textureDimensions(divergence_out));
    if (coords.x >= dims.x || coords.y >= dims.y) { return; }

    // Sample neighbors (Left, Right, Top, Bottom)
    let w = textureLoad(velocity_in, coords + vec2<i32>(-1, 0), 0).xy; // Left
    let e = textureLoad(velocity_in, coords + vec2<i32>( 1, 0), 0).xy; // Right
    let s = textureLoad(velocity_in, coords + vec2<i32>( 0,-1), 0).xy; // Bottom
    let n = textureLoad(velocity_in, coords + vec2<i32>( 0, 1), 0).xy; // Top

    // Calculate Divergence
    // How much velocity is entering vs leaving this cell?
    let div = 0.5 * (e.x - w.x + n.y - s.y);

    textureStore(divergence_out, coords, vec4<f32>(div, 0.0, 0.0, 0.0));
}

// --- JACOBI (PRESSURE) PIPELINE ---
@group(0) @binding(1) var pressure_in: texture_2d<f32>;
@group(0) @binding(2) var divergence_in: texture_2d<f32>;
@group(0) @binding(3) var pressure_out: texture_storage_2d<r32float, write>;

@compute @workgroup_size(16, 16)
fn jacobi_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let coords = vec2<i32>(id.xy);
    let dims = vec2<i32>(textureDimensions(pressure_out));
    if (coords.x >= dims.x || coords.y >= dims.y) { return; }

    // Read neighbors pressure
    let pL = textureLoad(pressure_in, coords + vec2<i32>(-1, 0), 0).x;
    let pR = textureLoad(pressure_in, coords + vec2<i32>( 1, 0), 0).x;
    let pB = textureLoad(pressure_in, coords + vec2<i32>( 0,-1), 0).x;
    let pT = textureLoad(pressure_in, coords + vec2<i32>( 0, 1), 0).x;

    // Read divergence (the "b" value in the equation)
    let bC = textureLoad(divergence_in, coords, 0).x;

    // The Jacobi formula for Poisson equation
    // Alpha = -1 (smoothness), Beta = 4 (neighbors)
    let pNew = (pL + pR + pB + pT - bC) * 0.25;

    textureStore(pressure_out, coords, vec4<f32>(pNew, 0.0, 0.0, 0.0));
}

// --- SUBTRACT GRADIENT PIPELINE ---
@group(0) @binding(1) var pressure_final: texture_2d<f32>;
@group(0) @binding(2) var velocity_old: texture_2d<f32>;
@group(0) @binding(3) var velocity_new: texture_storage_2d<rg32float, write>;

@compute @workgroup_size(16, 16)
fn subtract_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let coords = vec2<i32>(id.xy);
    let dims = vec2<i32>(textureDimensions(velocity_new));
    if (coords.x >= dims.x || coords.y >= dims.y) { return; }

    // Read neighbor pressures
    let pL = textureLoad(pressure_final, coords + vec2<i32>(-1, 0), 0).x;
    let pR = textureLoad(pressure_final, coords + vec2<i32>( 1, 0), 0).x;
    let pB = textureLoad(pressure_final, coords + vec2<i32>( 0,-1), 0).x;
    let pT = textureLoad(pressure_final, coords + vec2<i32>( 0, 1), 0).x;

    // Calculate Gradient (Slope of pressure)
    // 0.5 is because we span 2 cells (Left to Right)
    let grad = vec2<f32>(pR - pL, pT - pB) * 0.5;

    // Read old velocity
    let old_v = textureLoad(velocity_old, coords, 0).xy;

    // Subtract gradient to get mass-conserving velocity
    let new_v = old_v - grad;

    textureStore(velocity_new, coords, vec4<f32>(new_v, 0.0, 0.0));
}
