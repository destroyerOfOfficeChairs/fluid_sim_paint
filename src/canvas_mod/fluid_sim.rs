use super::pipelines::advect_pipeline::AdvectionPipeline;
use super::pipelines::brush_pipeline::{BrushPipeline, BrushUniforms};
use super::resources::texture::{Texture, create_sim_textures};
use crate::gui_mod::gui::GuiParams;
use wgpu::{BindGroup, CommandEncoder, Device, Queue};

pub struct FluidSim {
    pub width: u32,
    pub height: u32,

    pub density_a: Texture,
    pub density_b: Texture,
    pub velocity_a: Texture,
    pub velocity_b: Texture,

    brush_pipeline: BrushPipeline,
    advect_pipeline: AdvectionPipeline,

    // NEW: We don't need Vectors of bind groups anymore.
    // We just need specific, hard-wired connections.
    advect_bind_group: BindGroup, // Reads A -> Writes B
    brush_bind_group: BindGroup,  // Reads B -> Writes A
}

impl FluidSim {
    pub fn new(device: &Device, width: u32, height: u32) -> Self {
        let (density_a, density_b, velocity_a, velocity_b, _p_a, _p_b, _div) =
            create_sim_textures(device, width, height);

        let brush_pipeline = BrushPipeline::new(device);
        let advect_pipeline = AdvectionPipeline::new(device, width, height);

        // 1. ADVECTION: Read A -> Write B
        let advect_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Advect A->B"),
            layout: &advect_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: advect_pipeline.uniform_buffer.as_entire_binding(),
                },
                // Input: A
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&velocity_a.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&density_a.view),
                },
                // Output: B
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&velocity_b.view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&density_b.view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&density_a.sampler),
                },
            ],
        });

        // 2. BRUSH: Read B -> Write A
        // This ensures we add ink ON TOP of the advected result
        let brush_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Brush B->A"),
            layout: &brush_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: brush_pipeline.brush_buffer.as_entire_binding(),
                },
                // Input: B (The result of Advection)
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&density_b.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&density_a.view),
                },
                // Input: B
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&velocity_b.view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&velocity_a.view),
                },
            ],
        });

        Self {
            width,
            height,
            density_a,
            density_b,
            velocity_a,
            velocity_b,
            brush_pipeline,
            advect_pipeline,
            advect_bind_group,
            brush_bind_group,
        }
    }

    pub fn advect(&self, encoder: &mut CommandEncoder) {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Advection Pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&self.advect_pipeline.pipeline);
        compute_pass.set_bind_group(0, &self.advect_bind_group, &[]);

        let x_groups = (self.width as f32 / 16.0).ceil() as u32;
        let y_groups = (self.height as f32 / 16.0).ceil() as u32;
        compute_pass.dispatch_workgroups(x_groups, y_groups, 1);
    }

    pub fn add_forces(
        &mut self,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        current_pos: [f32; 2],
        last_pos: [f32; 2],
        params: &GuiParams,
    ) {
        // Update Uniforms
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

        // Dispatch
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Brush Pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&self.brush_pipeline.pipeline);
        compute_pass.set_bind_group(0, &self.brush_bind_group, &[]);

        let x_groups = (self.width as f32 / 16.0).ceil() as u32;
        let y_groups = (self.height as f32 / 16.0).ceil() as u32;
        compute_pass.dispatch_workgroups(x_groups, y_groups, 1);
    }
}
