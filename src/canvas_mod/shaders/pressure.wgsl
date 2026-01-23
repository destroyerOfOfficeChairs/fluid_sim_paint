struct PressureUniforms {
    width: f32,
    height: f32,
    dt: f32,
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

    // DIVERGENCE BOUNDARY FIX:
    // If a neighbor is outside the screen, assume velocity is 0 (Solid Wall).
    // Otherwise, read the texture.
    
    let w = select(textureLoad(velocity_in, coords + vec2<i32>(-1, 0), 0).xy, vec2<f32>(0.0), coords.x == 0);
    let e = select(textureLoad(velocity_in, coords + vec2<i32>( 1, 0), 0).xy, vec2<f32>(0.0), coords.x == dims.x - 1);
    let s = select(textureLoad(velocity_in, coords + vec2<i32>( 0,-1), 0).xy, vec2<f32>(0.0), coords.y == 0);
    let n = select(textureLoad(velocity_in, coords + vec2<i32>( 0, 1), 0).xy, vec2<f32>(0.0), coords.y == dims.y - 1);

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

    // Get Center Divergence
    let bC = textureLoad(divergence_in, coords, 0).x;

    // Get Center Pressure (needed for boundary fallback)
    let pC = textureLoad(pressure_in, coords, 0).x;

    // PRESSURE BOUNDARY FIX (Pure Neumann):
    // If neighbor is a wall, use Center Pressure (pC) instead of neighbor pressure.
    // This tells the physics "The pressure difference at the wall is zero".
    
    let pL = select(textureLoad(pressure_in, coords + vec2<i32>(-1, 0), 0).x, pC, coords.x == 0);
    let pR = select(textureLoad(pressure_in, coords + vec2<i32>( 1, 0), 0).x, pC, coords.x == dims.x - 1);
    let pB = select(textureLoad(pressure_in, coords + vec2<i32>( 0,-1), 0).x, pC, coords.y == 0);
    let pT = select(textureLoad(pressure_in, coords + vec2<i32>( 0, 1), 0).x, pC, coords.y == dims.y - 1);

    // Jacobi formula
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

    let pC = textureLoad(pressure_final, coords, 0).x;

    // GRADIENT BOUNDARY FIX:
    // Same rule: If neighbor is wall, use Center Pressure.
    let pL = select(textureLoad(pressure_final, coords + vec2<i32>(-1, 0), 0).x, pC, coords.x == 0);
    let pR = select(textureLoad(pressure_final, coords + vec2<i32>( 1, 0), 0).x, pC, coords.x == dims.x - 1);
    let pB = select(textureLoad(pressure_final, coords + vec2<i32>( 0,-1), 0).x, pC, coords.y == 0);
    let pT = select(textureLoad(pressure_final, coords + vec2<i32>( 0, 1), 0).x, pC, coords.y == dims.y - 1);

    let grad = vec2<f32>(pR - pL, pT - pB) * 0.5;

    let old_v = textureLoad(velocity_old, coords, 0).xy;
    let new_v = old_v - grad;

    textureStore(velocity_new, coords, vec4<f32>(new_v, 0.0, 0.0));
}
