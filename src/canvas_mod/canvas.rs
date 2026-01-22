use super::pipelines::brush_pipeline::{BrushPipeline, BrushUniforms};
use super::pipelines::draw::record_render_pass;
use super::pipelines::render_pipeline::{ViewUniforms, create_render_setup};
use super::resources::quad::create_canvas_quad;
use super::resources::texture::{Texture, create_sim_textures};
use crate::gui_mod::gui::GuiParams;
use crate::state::InteractionState;
use wgpu::util::DeviceExt;
use wgpu::{BindGroup, Buffer, CommandEncoder, Device, Queue, RenderPipeline, TextureView}; // We'll need to make InteractionState public in state.rs

pub struct SimState {
    pub width: u32,
    pub height: u32,
    pub density_a: Texture,
    pub density_b: Texture,
    pub velocity_a: Texture,
    pub velocity_b: Texture,
}

pub struct Canvas {
    // 1. The Physics World
    sim: SimState,
    frame_num: usize,

    // 2. The Tools (Pipelines)
    brush_pipeline: BrushPipeline,
    render_pipeline: RenderPipeline,

    // 3. The Wiring (Bind Groups)
    brush_bind_groups: Vec<BindGroup>,
    render_bind_groups: Vec<BindGroup>,

    // 4. Data
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
        // A. Setup Sim Textures
        let (density_a, density_b, velocity_a, velocity_b, _p_a, _p_b, _div) =
            create_sim_textures(device, width, height);

        let sim = SimState {
            width,
            height,
            density_a,
            density_b,
            velocity_a,
            velocity_b,
        };

        // B. Setup Geometry
        let (vertex_buffer, index_buffer, num_indices) = create_canvas_quad(device);

        // C. Setup View Uniforms
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

        // D. Setup Pipelines
        let brush_pipeline = BrushPipeline::new(device);
        let (render_pipeline, render_layout) = create_render_setup(device, config);

        // E. Create Bind Groups
        let create_render_bg = |tex: &Texture| -> BindGroup {
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

        let create_brush_bg = |in_den: &Texture,
                               out_den: &Texture,
                               in_vel: &Texture,
                               out_vel: &Texture|
         -> BindGroup {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Brush Group"),
                layout: &brush_pipeline.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: brush_pipeline.brush_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&in_den.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&out_den.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&in_vel.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&out_vel.view),
                    },
                ],
            })
        };
        let brush_bind_groups = vec![
            create_brush_bg(
                &sim.density_a,
                &sim.density_b,
                &sim.velocity_a,
                &sim.velocity_b,
            ),
            create_brush_bg(
                &sim.density_b,
                &sim.density_a,
                &sim.velocity_b,
                &sim.velocity_a,
            ),
        ];

        Self {
            sim,
            frame_num: 0,
            brush_pipeline,
            render_pipeline,
            brush_bind_groups,
            render_bind_groups,
            view_buffer,
            vertex_buffer,
            index_buffer,
            num_indices,
        }
    }

    pub fn update_brush(
        &self,
        queue: &Queue,
        input: &InteractionState,
        params: &GuiParams,
        screen_size: (u32, u32),
    ) {
        if !input.mouse_pressed {
            return;
        }

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

        let brush_data = BrushUniforms {
            mouse_pos: current_grid,
            last_mouse_pos: last_grid,
            radius: params.brush_size / params.zoom_level,
            strength: 1.0,
        };

        queue.write_buffer(
            &self.brush_pipeline.brush_buffer,
            0,
            bytemuck::cast_slice(&[brush_data]),
        );
    }

    pub fn render(
        &mut self,
        encoder: &mut CommandEncoder,
        view: &TextureView,
        queue: &Queue,
        params: &GuiParams,
        screen_size: (u32, u32),
        input: &InteractionState,
    ) {
        let in_index = self.frame_num % 2;
        let render_index = (self.frame_num + 1) % 2;

        // 1. Update View Buffer (Zoom/Pan)
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

        // 2. Run Brush (Compute)
        if input.mouse_pressed {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Brush Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.brush_pipeline.pipeline);
            compute_pass.set_bind_group(0, &self.brush_bind_groups[in_index], &[]);

            let x_groups = (self.sim.width as f32 / 16.0).ceil() as u32;
            let y_groups = (self.sim.height as f32 / 16.0).ceil() as u32;
            compute_pass.dispatch_workgroups(x_groups, y_groups, 1);
        }

        // 3. Render Canvas
        record_render_pass(
            encoder,
            view,
            &self.render_pipeline,
            &self.render_bind_groups[render_index],
            &self.vertex_buffer,
            &self.index_buffer,
            self.num_indices,
        );

        self.frame_num += 1;
    }
}
