struct ViewUniforms {
    screen_size: vec2<f32>,
    canvas_size: vec2<f32>,
    pan: vec2<f32>,
    zoom: f32,
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

    // Calculate the Ratio (Target Pixels / Screen Pixels)
    // Example: 1920 / 3840 = 0.5 (Take up half the screen)
    let size_ratio = view.canvas_size / view.screen_size;

    // Apply Zoom
    let final_scale = size_ratio * view.zoom;

    // Apply to Vertex Position
    // We only scale X and Y. We add Pan here too for the future.
    let pos_xy = (model.position.xy * final_scale) + view.pan;

    out.clip_position = vec4<f32>(pos_xy, 0.0, 1.0);
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
    
    // RGB = The actual color in the fluid
    let ink_color = fluid_data.rgb;
    // A = How much ink is there
    let density = fluid_data.a;

    let paper_color = vec3<f32>(1.0, 1.0, 1.0); // White paper

    // Blend Ink onto Paper 
    let final_color = mix(paper_color, ink_color, density);

    return vec4<f32>(final_color, 1.0);
}
