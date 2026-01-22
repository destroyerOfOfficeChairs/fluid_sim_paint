struct BrushUniforms {
    // We pass the "Grid Coordinates" of the mouse directly
    mouse_pos: vec2<f32>, 
    radius: f32,
    strength: f32,
};

@group(0) @binding(0) var<uniform> brush: BrushUniforms;
@group(0) @binding(1) var input_texture: texture_2d<f32>;       // Read from here
@group(0) @binding(2) var output_texture: texture_storage_2d<rgba32float, write>; // Write to here

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let coords = vec2<i32>(id.xy);
    let dims = vec2<i32>(textureDimensions(output_texture));

    // Bounds check
    if (coords.x >= dims.x || coords.y >= dims.y) {
        return;
    }

    // 1. Read the existing paint (Don't erase what's there!)
    let old_color = textureLoad(input_texture, coords, 0);

    // 2. Calculate Distance to Mouse
    let pixel_pos = vec2<f32>(f32(coords.x), f32(coords.y));
    let dist = distance(pixel_pos, brush.mouse_pos);

    // 3. Apply Brush
    var new_alpha = old_color.a;
    
    if (dist < brush.radius) {
        // Simple hard circle for now
        new_alpha += brush.strength; 
    }
    
    // Clamp to max 1.0 density
    new_alpha = min(new_alpha, 1.0);

    // 4. Write Output (Preserve RGB, update Alpha)
    textureStore(output_texture, coords, vec4<f32>(old_color.rgb, new_alpha));
}
