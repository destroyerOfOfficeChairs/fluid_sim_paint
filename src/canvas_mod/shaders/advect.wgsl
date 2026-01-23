struct AdvectionUniforms {
    dt: f32,             // Time step (e.g., 0.016)
    width: f32,          // Sim width
    height: f32,         // Sim height
    dissipation: f32,    // Decay (0.99 = slow fade, 1.0 = forever)
};

@group(0) @binding(0) var<uniform> params: AdvectionUniforms;
@group(0) @binding(1) var velocity_in: texture_2d<f32>;
@group(0) @binding(2) var density_in: texture_2d<f32>;
@group(0) @binding(3) var velocity_out: texture_storage_2d<rg32float, write>;
@group(0) @binding(4) var density_out: texture_storage_2d<rgba32float, write>;
@group(0) @binding(5) var tex_sampler: sampler;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let coords = vec2<i32>(id.xy);
    let dims = vec2<i32>(textureDimensions(velocity_out));

    if (coords.x >= dims.x || coords.y >= dims.y) {
        return;
    }

    let dt = params.dt;
    
    // 1. Where are we in World Space?
    let pos = vec2<f32>(f32(coords.x), f32(coords.y));
    
    // 2. Read the Velocity at this point
    // We normailze UVs because sample_texture requires 0.0-1.0 range
    let uv = (pos + 0.5) / vec2<f32>(params.width, params.height);
    let velocity = textureSampleLevel(velocity_in, tex_sampler, uv, 0.0).xy;

    // 3. Trace Backwards
    // "Where did the stuff at this pixel come from?"
    // Result = Pos - (Velocity * Time)
    let back_pos = pos - (velocity * dt);

    // 4. Sample the Old Frame at that "Backwards" position
    let back_uv = (back_pos + 0.5) / vec2<f32>(params.width, params.height);
    
    let advected_density = textureSampleLevel(density_in, tex_sampler, back_uv, 0.0);
    let advected_velocity = textureSampleLevel(velocity_in, tex_sampler, back_uv, 0.0);

    // 5. Apply Dissipation (Fade out slightly)
    let new_density = advected_density * params.dissipation;
    // Velocity also decays, otherwise water spins forever
    let new_velocity = advected_velocity.xy * params.dissipation; 

    // 6. Write Result
    textureStore(density_out, coords, new_density);
    textureStore(velocity_out, coords, vec4<f32>(new_velocity, 0.0, 0.0));
}
