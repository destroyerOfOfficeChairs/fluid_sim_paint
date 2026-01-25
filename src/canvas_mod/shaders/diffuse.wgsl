struct DiffuseUniforms {
    width: f32,
    height: f32,
    alpha: f32,        // determined by viscosity and dt
    one_over_beta: f32 // 1 / (4 + alpha)
};

@group(0) @binding(0) var<uniform> params: DiffuseUniforms;
@group(0) @binding(1) var x_in: texture_2d<f32>;          // The texture we are diffusing (Density or Velocity)
@group(0) @binding(2) var b_in: texture_2d<f32>;          // The original state (b in the Ax=b equation)
@group(0) @binding(3) var x_out: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let coords = vec2<i32>(id.xy);
    let dims = vec2<i32>(textureDimensions(x_out));
    if (coords.x >= dims.x || coords.y >= dims.y) { return; }

    // Read Neighbors (with clamping/boundary logic)
    // ... (Use your standard boundary lookup logic here) ...
    
    let C = textureLoad(x_in, coords, 0);
    let L = textureLoad(x_in, coords + vec2<i32>(-1, 0), 0);
    let R = textureLoad(x_in, coords + vec2<i32>( 1, 0), 0);
    let B = textureLoad(x_in, coords + vec2<i32>( 0,-1), 0);
    let T = textureLoad(x_in, coords + vec2<i32>( 0, 1), 0);

    let bC = textureLoad(b_in, coords, 0);

    // The Generalized Jacobi Diffusion Formula
    let newVal = (L + R + B + T + (bC * params.alpha)) * params.one_over_beta;

    textureStore(x_out, coords, newVal);
}
