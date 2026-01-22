struct ViewUniforms {
    scale: f32,
    pan: vec2<f32>, 
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@group(0) @binding(2) var<uniform> view: ViewUniforms;

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // We multiply the position by the scale from the UI.
    // (We will implement Pan later, so we ignore it for now)
    out.clip_position = vec4<f32>(model.position * view.scale, 1.0);
    
    out.tex_coords = model.tex_coords;
    return out;
}

// Fragment Shader
@group(0) @binding(0) var density_texture: texture_2d<f32>;
@group(0) @binding(1) var density_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 1. Sample the Simulation Texture
    let fluid_data = textureSample(density_texture, density_sampler, in.tex_coords);
    
    // RGB = Color (Black for now), A = Density
    let density = fluid_data.a;

    // 2. Define Colors
    let paper_color = vec3<f32>(1.0, 1.0, 1.0); // Pure WHITE Paper
    let ink_color   = vec3<f32>(0.0, 0.0, 0.0); // Pure BLACK Ink

    // 3. Blend Ink onto Paper 
    // mix(paper, ink, density) means:
    // If density is 0.0, show Paper.
    // If density is 1.0, show Ink.
    let final_color = mix(paper_color, ink_color, density);

    return vec4<f32>(final_color, 1.0);
}
