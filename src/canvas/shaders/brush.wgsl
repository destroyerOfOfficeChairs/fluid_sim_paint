struct BrushUniforms {
    mouse_pos: vec2<f32>,      // Current Position
    last_mouse_pos: vec2<f32>, // Previous Position (NEW)
    radius: f32,
    strength: f32,
};

@group(0) @binding(0) var<uniform> brush: BrushUniforms;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var output_texture: texture_storage_2d<rgba32float, write>;

// Helper function: Distance squared to a line segment (p1 to p2)
fn dist_sq_to_segment(p: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>) -> f32 {
    let l2 = dot(p2 - p1, p2 - p1);
    if (l2 == 0.0) { return dot(p - p1, p - p1); }
    
    let t = clamp(dot(p - p1, p2 - p1) / l2, 0.0, 1.0);
    let projection = p1 + t * (p2 - p1);
    return dot(p - projection, p - projection);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let coords = vec2<i32>(id.xy);
    let dims = vec2<i32>(textureDimensions(output_texture));

    if (coords.x >= dims.x || coords.y >= dims.y) {
        return;
    }

    let pixel_pos = vec2<f32>(f32(coords.x), f32(coords.y));
    
    // 1. Calculate Distance to the "Capsule" (Line Segment)
    // We use squared distance to avoid expensive sqrt() until the end
    let d2 = dist_sq_to_segment(pixel_pos, brush.last_mouse_pos, brush.mouse_pos);
    
    let old_color = textureLoad(input_texture, coords, 0);
    var new_alpha = old_color.a;

    // 2. Compare with Radius Squared
    if (d2 < brush.radius * brush.radius) {
        new_alpha += brush.strength;
    }

    new_alpha = min(new_alpha, 1.0);
    
    textureStore(output_texture, coords, vec4<f32>(old_color.rgb, new_alpha));
}
