struct BrushUniforms {
    mouse_pos: vec2<f32>,
    last_mouse_pos: vec2<f32>,
    velocity_factor: f32,
    radius: f32,
    smudge: f32,
    // Shader automatically handles the padding to align the next vec4
    brush_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> brush: BrushUniforms;
@group(0) @binding(1) var density_in: texture_2d<f32>;
@group(0) @binding(2) var density_out: texture_storage_2d<rgba32float, write>;
@group(0) @binding(3) var velocity_in: texture_2d<f32>;
@group(0) @binding(4) var velocity_out: texture_storage_2d<rg32float, write>;

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
    let dims = vec2<i32>(textureDimensions(density_out));

    if (coords.x >= dims.x || coords.y >= dims.y) {
        return;
    }

    // 1. ALWAYS Read the Input (This is the Advected Result from Step 1)
    let in_density = textureLoad(density_in, coords, 0);
    let in_velocity = textureLoad(velocity_in, coords, 0);

    // 2. Prepare variables to hold the Final Value
    var final_density = in_density;
    var final_velocity = in_velocity;

    // 3. If inside Brush, Modify the values
    let pixel_pos = vec2<f32>(f32(coords.x), f32(coords.y));
    let d2 = dist_sq_to_segment(pixel_pos, brush.last_mouse_pos, brush.mouse_pos);

    if (d2 < brush.radius * brush.radius) {
        // MODE 1: BLENDER (Smudge)
        if (brush.smudge > 0.5) {
            let c = textureLoad(density_in, coords, 0);

            // --- NEW: STRONGER BLUR ---
            var total_color = vec4<f32>(0.0);
            var count = 0.0;
            
            // Radius 4 means we sample a 9x9 box (81 pixels!)
            // Increase this number to 5 or 6 for an even bigger blur.
            let blur_radius = 4; 

            for (var x = -blur_radius; x <= blur_radius; x++) {
                for (var y = -blur_radius; y <= blur_radius; y++) {
                    let neighbor_pos = coords + vec2<i32>(x, y);
                    
                    // Safety: clamp to screen size so we don't crash at the edges
                    let safe_pos = clamp(neighbor_pos, vec2<i32>(0, 0), dims - vec2<i32>(1, 1));
                    
                    total_color += textureLoad(density_in, safe_pos, 0);
                    count += 1.0;
                }
            }

            let blurred = total_color / count;
            
            // Mix 90% of the blurred color in (was 0.5 before)
            // This makes the smudge instant instead of gradual.
            final_density = mix(c, blurred, 0.9);
            // ---------------------------

            // CRITICAL: We still add velocity! 
            let velocity_add = (brush.mouse_pos - brush.last_mouse_pos) * brush.velocity_factor;
            final_velocity = vec4<f32>(final_velocity.xy + velocity_add, 0.0, 0.0);

        } else {
            // MODE 2: PAINTER (Add Ink)
            // The Alpha of the brush IS the amount we add.
            let amount = brush.brush_color.a;
            
            // Target Mix (Marker Style)
            // Interpolate current density towards 1.0 based on alpha
            let new_alpha = mix(final_density.a, 1.0, amount);
            
            // Color Mix
            // Interpolate current color towards brush color based on alpha
            let mixed_rgb = mix(final_density.rgb, brush.brush_color.rgb, amount);
            
            final_density = vec4<f32>(mixed_rgb, new_alpha);

            // Add Velocity
            let velocity_add = (brush.mouse_pos - brush.last_mouse_pos) * brush.velocity_factor; // Tweak force here
            final_velocity = vec4<f32>(final_velocity.xy + velocity_add, 0.0, 0.0);
        }
    }

    // 4. ALWAYS Write to Output
    // This ensures the advection (movement/fading) is applied to the whole screen
    textureStore(density_out, coords, final_density);
    textureStore(velocity_out, coords, final_velocity);
}
