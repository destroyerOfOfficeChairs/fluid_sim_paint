use super::pipelines::advect_pipeline::{AdvectionPipeline, AdvectionUniforms};
use super::pipelines::brush_pipeline::{BrushPipeline, BrushUniforms};
use super::pipelines::pressure_pipeline::PressurePipeline;
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
    pub divergence: Texture,
    pub pressure_a: Texture,
    pub pressure_b: Texture,

    brush_pipeline: BrushPipeline,
    advect_pipeline: AdvectionPipeline,
    pressure_pipeline: PressurePipeline,

    advect_bind_group: BindGroup, // Reads A -> Writes B
    brush_bind_group: BindGroup,  // Reads B -> Writes A
    div_bind_group: BindGroup,
    jacobi_bind_groups: Vec<BindGroup>, // Needs A->B and B->A
    sub_bind_group: BindGroup,
}

impl FluidSim {
    pub fn new(device: &Device, width: u32, height: u32) -> Self {
        let (density_a, density_b, velocity_a, velocity_b, pressure_a, pressure_b, divergence) =
            create_sim_textures(device, width, height);

        let brush_pipeline = BrushPipeline::new(device);
        let advect_pipeline = AdvectionPipeline::new(device, width, height);
        let pressure_pipeline = PressurePipeline::new(device, width, height);

        // ADVECTION: Read A -> Write B
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

        // BRUSH: Read B -> Write A
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

        // Divergence Bind Group (Read Vel A -> Write Div)
        let div_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Divergence BG"),
            layout: &pressure_pipeline.div_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: pressure_pipeline.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&velocity_a.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&divergence.view),
                },
            ],
        });

        // Jacobi Bind Groups (Ping Pong)
        let create_jacobi = |in_p: &Texture, out_p: &Texture| -> BindGroup {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Jacobi BG"),
                layout: &pressure_pipeline.jacobi_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: pressure_pipeline.uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&in_p.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&divergence.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&out_p.view),
                    },
                ],
            })
        };
        let jacobi_bind_groups = vec![
            create_jacobi(&pressure_a, &pressure_b), // 0: A -> B
            create_jacobi(&pressure_b, &pressure_a), // 1: B -> A
        ];

        // Subtract Gradient (Read Press A + Vel A -> Write Vel B)
        // Note: We write to B temporarily, then we'll copy back to A.
        let sub_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Subtract BG"),
            layout: &pressure_pipeline.sub_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: pressure_pipeline.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&pressure_a.view),
                }, // Use A as final pressure
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&velocity_a.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&velocity_b.view),
                }, // Write to B
            ],
        });

        Self {
            width,
            height,
            density_a,
            density_b,
            velocity_a,
            velocity_b,
            divergence,
            pressure_a,
            pressure_b,
            div_bind_group,
            jacobi_bind_groups,
            sub_bind_group,
            brush_pipeline,
            advect_pipeline,
            pressure_pipeline,
            advect_bind_group,
            brush_bind_group,
        }
    }

    pub fn project(&mut self, encoder: &mut CommandEncoder) {
        let x_groups = (self.width as f32 / 16.0).ceil() as u32;
        let y_groups = (self.height as f32 / 16.0).ceil() as u32;

        // 1. Calculate Divergence
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Div Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pressure_pipeline.div_pipeline);
            pass.set_bind_group(0, &self.div_bind_group, &[]);
            pass.dispatch_workgroups(x_groups, y_groups, 1);
        }

        // 2. Solve Pressure (Jacobi Iteration)
        // Run this 20-50 times to propagate pressure across the grid
        for i in 0..40 {
            let in_index = i % 2;
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Jacobi Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pressure_pipeline.jacobi_pipeline);
            pass.set_bind_group(0, &self.jacobi_bind_groups[in_index], &[]);
            pass.dispatch_workgroups(x_groups, y_groups, 1);
        }

        // 3. Subtract Gradient
        // Uses final pressure (A) and current velocity (A) to write new velocity (B)
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Sub Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pressure_pipeline.sub_pipeline);
            pass.set_bind_group(0, &self.sub_bind_group, &[]);
            pass.dispatch_workgroups(x_groups, y_groups, 1);
        }

        // 4. Enforce Invariant: A is always valid
        // Copy B (Result) -> A
        encoder.copy_texture_to_texture(
            self.velocity_b.texture.as_image_copy(),
            self.velocity_a.texture.as_image_copy(),
            self.velocity_a.texture.size(),
        );
    }

    // Update signature to accept Queue and Params
    pub fn advect(&self, queue: &Queue, encoder: &mut CommandEncoder, params: &GuiParams) {
        // 1. Create the new Uniform data from the UI params
        let uniforms = AdvectionUniforms {
            dt: 0.016,
            width: self.width as f32,
            height: self.height as f32,
            // Connect UI Sliders to Physics
            velocity_decay: params.velocity_decay,
            ink_decay: params.ink_decay,
            _padding: [0.0; 3],
        };

        // 2. Upload it to the GPU
        queue.write_buffer(
            &self.advect_pipeline.uniform_buffer,
            0,
            bytemuck::cast_slice(&[uniforms]),
        );

        // 3. Dispatch (Same as before)
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
            velocity_factor: params.velocity_factor,
            radius: params.brush_size / params.zoom_level,
            _padding: [0.0; 2], // Zero out padding
            brush_color: params.brush_color,
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

    pub fn clear(&self, encoder: &mut CommandEncoder) {
        let mut clear_tex = |tex: &Texture| {
            encoder.clear_texture(
                &tex.texture,
                &wgpu::ImageSubresourceRange {
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                },
            );
        };

        // Wipe everything
        clear_tex(&self.density_a);
        clear_tex(&self.density_b);
        clear_tex(&self.velocity_a);
        clear_tex(&self.velocity_b);
        clear_tex(&self.pressure_a);
        clear_tex(&self.pressure_b);
        clear_tex(&self.divergence);
    }
}
