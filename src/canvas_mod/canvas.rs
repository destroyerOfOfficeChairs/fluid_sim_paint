use super::fluid_sim::FluidSim;
use super::pipelines::draw::record_render_pass;
use super::pipelines::render_pipeline::{ViewUniforms, create_render_setup};
use super::resources::quad::create_canvas_quad;
use crate::gui_mod::gui::GuiParams;
use crate::state::InteractionState;
use wgpu::util::DeviceExt;
use wgpu::{BindGroup, Buffer, CommandEncoder, Device, Queue, RenderPipeline, TextureView};

pub struct Canvas {
    pub sim: FluidSim, // Public so State can query width/height if needed

    // Renderer Internals
    render_pipeline: RenderPipeline,
    render_bind_groups: Vec<BindGroup>,
    view_buffer: Buffer,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    num_indices: u32,
}

impl Canvas {
    pub fn new(
        device: &Device,
        config: &wgpu::SurfaceConfiguration,
        width: u32,
        height: u32,
        default_zoom: f32,
    ) -> Self {
        // 1. Initialize the Physics Engine
        let sim = FluidSim::new(device, width, height);

        // 2. Setup Geometry (The Quad)
        let (vertex_buffer, index_buffer, num_indices) = create_canvas_quad(device);

        // 3. Setup View Uniforms (Camera)
        let initial_uniforms = ViewUniforms {
            screen_size: [config.width as f32, config.height as f32],
            canvas_size: [width as f32, height as f32],
            pan: [0.0, 0.0],
            zoom: default_zoom,
            _padding: 0,
        };

        let view_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("View Uniform Buffer"),
            contents: bytemuck::cast_slice(&[initial_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // 4. Setup Render Pipeline
        let (render_pipeline, render_layout) = create_render_setup(device, config);

        // 5. Create Render Bind Groups
        // CRITICAL: We bind to the textures OWNED by 'sim'
        let create_render_bg = |tex: &super::resources::texture::Texture| -> BindGroup {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Render Group"),
                layout: &render_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&tex.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&tex.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: view_buffer.as_entire_binding(),
                    },
                ],
            })
        };

        let render_bind_groups = vec![
            create_render_bg(&sim.density_a),
            create_render_bg(&sim.density_b),
        ];

        Self {
            sim,
            render_pipeline,
            render_bind_groups,
            view_buffer,
            vertex_buffer,
            index_buffer,
            num_indices,
        }
    }

    pub fn update_brush(
        &mut self,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        input: &InteractionState,
        params: &GuiParams,
        screen_size: (u32, u32),
    ) {
        // 1. Always update View Buffer (so panning/zooming works even if not drawing)
        let current_uniforms = ViewUniforms {
            screen_size: [screen_size.0 as f32, screen_size.1 as f32],
            canvas_size: [self.sim.width as f32, self.sim.height as f32],
            pan: [0.0, 0.0],
            zoom: params.zoom_level,
            _padding: 0,
        };
        queue.write_buffer(
            &self.view_buffer,
            0,
            bytemuck::cast_slice(&[current_uniforms]),
        );

        // 2. Logic Check: Only run physics if mouse is clicked
        if !input.mouse_pressed {
            self.sim.step();
            return;
        }

        // 3. Coordinate Transformation (Screen -> Grid)
        let to_grid = |screen_pos: [f32; 2]| -> [f32; 2] {
            let screen_center_x = screen_size.0 as f32 / 2.0;
            let screen_center_y = screen_size.1 as f32 / 2.0;
            let offset_x = screen_pos[0] - screen_center_x;
            let offset_y = screen_pos[1] - screen_center_y;
            let zoom = params.zoom_level;
            let grid_center_x = self.sim.width as f32 / 2.0;
            let grid_center_y = self.sim.height as f32 / 2.0;
            [
                grid_center_x + (offset_x / zoom),
                grid_center_y + (offset_y / zoom),
            ]
        };

        let current_grid = to_grid(input.mouse_pos);
        let last_grid = to_grid(input.last_mouse_pos);

        // 4. Delegate to Sim
        self.sim
            .add_forces(queue, encoder, current_grid, last_grid, params);

        // 5. Advance Time
        // We only step if we actually drew something (for now)
        self.sim.step();
    }

    pub fn render(&self, encoder: &mut CommandEncoder, view: &TextureView) {
        // Render the texture that matches the current Sim frame
        // If Sim just wrote to B (frame 0), we want to render B.
        // But Sim.step() incremented frame to 1.
        // So we want index (1-1)%2 = 0?
        // Sync logic: Sim writes to 'frame % 2'. We display 'frame % 2'.
        // Wait, if Sim writes B, B is the latest. We want to see B.
        // Let's stick to: Render index matches the "Output" of the previous step.
        let render_index = (self.sim.frame_num + 1) % 2;

        record_render_pass(
            encoder,
            view,
            &self.render_pipeline,
            &self.render_bind_groups[render_index],
            &self.vertex_buffer,
            &self.index_buffer,
            self.num_indices,
        );
    }
}
