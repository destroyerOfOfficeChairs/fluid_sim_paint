use super::pipelines::advect_pipeline::AdvectionPipeline;
use super::pipelines::brush_pipeline::{BrushPipeline, BrushUniforms};
use super::resources::texture::{Texture, create_sim_textures};
use crate::gui_mod::gui::GuiParams;
use wgpu::{BindGroup, CommandEncoder, Device, Queue};

pub struct FluidSim {
    pub width: u32,
    pub height: u32,

    // Data (Public so Canvas can bind to them for rendering)
    pub density_a: Texture,
    pub density_b: Texture,
    pub velocity_a: Texture,
    pub velocity_b: Texture,

    // Internals
    brush_pipeline: BrushPipeline,
    brush_bind_groups: Vec<BindGroup>,
    advect_pipeline: AdvectionPipeline,
    advect_bind_groups: Vec<BindGroup>,

    pub frame_num: usize,
}

impl FluidSim {
    pub fn new(device: &Device, width: u32, height: u32) -> Self {
        // Create Physics State
        let (density_a, density_b, velocity_a, velocity_b, _p_a, _p_b, _div) =
            create_sim_textures(device, width, height);

        // Create Compute Pipeline
        let brush_pipeline = BrushPipeline::new(device);

        // Create Advection Pipeline
        let advect_pipeline = AdvectionPipeline::new(device, width, height);

        // Wire up Bind Groups (Internal wiring)
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
            create_brush_bg(&density_a, &density_b, &velocity_a, &velocity_b),
            create_brush_bg(&density_b, &density_a, &velocity_b, &velocity_a),
        ];

        // Create Advection Bind Groups
        let create_advect_bg = |in_vel: &Texture,
                                in_den: &Texture,
                                out_vel: &Texture,
                                out_den: &Texture|
         -> BindGroup {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Advection Group"),
                layout: &advect_pipeline.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: advect_pipeline.uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&in_vel.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&in_den.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&out_vel.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&out_den.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&in_den.sampler),
                    },
                ],
            })
        };

        let advect_bind_groups = vec![
            // Frame 0: Read A -> Write B
            create_advect_bg(&velocity_a, &density_a, &velocity_b, &density_b),
            // Frame 1: Read B -> Write A
            create_advect_bg(&velocity_b, &density_b, &velocity_a, &density_a),
        ];

        Self {
            width,
            height,
            density_a,
            density_b,
            velocity_a,
            velocity_b,
            brush_pipeline,
            brush_bind_groups,
            advect_pipeline,
            advect_bind_groups,
            frame_num: 0,
        }
    }

    pub fn advect(&self, encoder: &mut CommandEncoder) {
        let in_index = self.frame_num % 2;

        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Advection Pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&self.advect_pipeline.pipeline);
        compute_pass.set_bind_group(0, &self.advect_bind_groups[in_index], &[]);

        let x_groups = (self.width as f32 / 16.0).ceil() as u32;
        let y_groups = (self.height as f32 / 16.0).ceil() as u32;
        compute_pass.dispatch_workgroups(x_groups, y_groups, 1);
    }

    pub fn add_forces(
        &mut self,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        current_pos: [f32; 2], // <--- Pre-calculated Grid Coords
        last_pos: [f32; 2],    // <--- Pre-calculated Grid Coords
        params: &GuiParams,
    ) {
        let in_index = self.frame_num % 2;

        let brush_data = BrushUniforms {
            mouse_pos: current_pos,
            last_mouse_pos: last_pos,
            radius: params.brush_size / params.zoom_level,
            strength: 1.0,
        };

        queue.write_buffer(
            &self.brush_pipeline.brush_buffer,
            0,
            bytemuck::cast_slice(&[brush_data]),
        );

        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Brush Pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&self.brush_pipeline.pipeline);
        compute_pass.set_bind_group(0, &self.brush_bind_groups[in_index], &[]);

        let x_groups = (self.width as f32 / 16.0).ceil() as u32;
        let y_groups = (self.height as f32 / 16.0).ceil() as u32;
        compute_pass.dispatch_workgroups(x_groups, y_groups, 1);
    }

    pub fn step(&mut self) {
        self.frame_num += 1;
    }
}
