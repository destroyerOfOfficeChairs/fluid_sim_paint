use super::super::gui_mod::gui::Gui;
use super::state_new_helpers::brush_pipeline::{BrushPipeline, BrushUniforms};
use super::state_new_helpers::quad::create_canvas_quad;
use super::state_new_helpers::render_pipeline::{ViewUniforms, create_render_setup};
use super::state_new_helpers::texture::{Texture, create_sim_textures};
use super::state_new_helpers::wgpu_init::wgpu_init;
use super::state_render_helpers::draw::record_render_pass;
use std::iter;
use std::sync::Arc;
use wgpu::Buffer;
use wgpu::util::DeviceExt;
use winit::event::{ElementState, MouseButton};
use winit::{
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
    window::{Fullscreen, Window},
};

// --- NEW STRUCT: Interaction State ---
// Holds all input-related data
pub struct InteractionState {
    pub mouse_pos: [f32; 2],
    pub last_mouse_pos: [f32; 2],
    pub mouse_pressed: bool,
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            mouse_pos: [0.0, 0.0],
            last_mouse_pos: [0.0, 0.0],
            mouse_pressed: false,
        }
    }
}

// --- NEW STRUCT: Simulation State ---
// Holds all Physics Textures and Dimensions
pub struct SimState {
    pub width: u32,
    pub height: u32,
    pub density_a: Texture,
    pub density_b: Texture,
    pub velocity_a: Texture,
    pub velocity_b: Texture,
    #[allow(dead_code)] // Keep these for future steps
    pub pressure_a: Texture,
    #[allow(dead_code)]
    pub pressure_b: Texture,
    #[allow(dead_code)]
    pub divergence: Texture,
}

impl SimState {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let (density_a, density_b, velocity_a, velocity_b, pressure_a, pressure_b, divergence) =
            create_sim_textures(device, width, height);

        Self {
            width,
            height,
            density_a,
            density_b,
            velocity_a,
            velocity_b,
            pressure_a,
            pressure_b,
            divergence,
        }
    }
}

// --- MAIN STATE ---
#[allow(dead_code)]
pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub is_surface_configured: bool,
    pub window: Arc<Window>,
    gui: Gui,

    // Sub-Structs (Organization!)
    pub sim: SimState,
    pub interaction: InteractionState,

    // Buffers
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    num_indices: u32,
    view_buffer: wgpu::Buffer,

    frame_num: usize,

    // Pipelines & Bind Groups
    // Note: We keep pipelines here as they bridge the gap between "System" and "Sim"
    render_pipeline: wgpu::RenderPipeline,
    render_bind_groups: Vec<wgpu::BindGroup>,
    brush_pipeline: BrushPipeline,
    brush_bind_groups: Vec<wgpu::BindGroup>,
}

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let (surface, device, queue, config) = wgpu_init(window.clone()).await;
        let gui = Gui::new(&window, &device, config.format);

        // 1. Initialize Simulation State
        // (We moved the texture creation inside SimState::new)
        let sim = SimState::new(&device, gui.params.canvas_width, gui.params.canvas_height);

        // 2. Initialize Interaction State
        let interaction = InteractionState::default();

        // 3. Geometry
        let (vertex_buffer, index_buffer, num_indices) = create_canvas_quad(&device);

        // 4. Pipelines
        let brush_pipeline = BrushPipeline::new(&device);

        // 5. View Uniforms
        let initial_uniforms = ViewUniforms {
            screen_size: [config.width as f32, config.height as f32],
            canvas_size: [sim.width as f32, sim.height as f32],
            pan: [0.0, 0.0],
            zoom: gui.params.zoom_level,
            _padding: 0,
        };

        let view_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("View Uniform Buffer"),
            contents: bytemuck::cast_slice(&[initial_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // 6. Setup Render Pipeline
        // Note: create_render_setup now returns (Pipeline, Layout) based on your fix
        let (render_pipeline, render_layout) = create_render_setup(&device, &config);

        // 7. Create Render Bind Groups
        // We reference textures via 'sim.' now
        let create_render_bg = |tex: &Texture, name: &str| -> wgpu::BindGroup {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(name),
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
            create_render_bg(&sim.density_a, "Render Group A"),
            create_render_bg(&sim.density_b, "Render Group B"),
        ];

        // 8. Create Brush Bind Groups
        let create_brush_bg = |in_den: &Texture,
                               out_den: &Texture,
                               in_vel: &Texture,
                               out_vel: &Texture,
                               name: &str|
         -> wgpu::BindGroup {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(name),
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
                "Brush A->B",
            ),
            create_brush_bg(
                &sim.density_b,
                &sim.density_a,
                &sim.velocity_b,
                &sim.velocity_a,
                "Brush B->A",
            ),
        ];

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            window,
            gui,
            sim,         // <--- Clean!
            interaction, // <--- Clean!
            vertex_buffer,
            index_buffer,
            num_indices,
            view_buffer,
            frame_num: 0,
            render_pipeline,
            render_bind_groups,
            brush_pipeline,
            brush_bind_groups,
        })
    }

    pub fn handle_mouse(&mut self, pos: [f32; 2]) {
        self.interaction.mouse_pos = pos;
    }

    pub fn handle_click(&mut self, state: ElementState, button: MouseButton) {
        if button == MouseButton::Left {
            self.interaction.mouse_pressed = state == ElementState::Pressed;
        }
    }

    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) {
        self.gui.handle_event(&self.window, event);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.is_surface_configured = true;
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: KeyCode, pressed: bool) {
        if !pressed {
            return;
        }
        match key {
            KeyCode::Escape => event_loop.exit(),
            KeyCode::F11 => match self.window.fullscreen() {
                Some(_) => self.window.set_fullscreen(None),
                None => self
                    .window
                    .set_fullscreen(Some(Fullscreen::Borderless(None))),
            },
            _ => {}
        }
    }

    pub fn update(&mut self) {}

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.window.request_redraw();
        if !self.is_surface_configured {
            return Ok(());
        }

        let in_index = self.frame_num % 2;
        let render_index = (self.frame_num + 1) % 2;

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Update View Uniforms
        // Note: access sim.width/height
        let current_uniforms = ViewUniforms {
            screen_size: [self.config.width as f32, self.config.height as f32],
            canvas_size: [
                self.gui.params.canvas_width as f32,
                self.gui.params.canvas_height as f32,
            ],
            pan: [0.0, 0.0],
            zoom: self.gui.params.zoom_level,
            _padding: 0,
        };

        self.queue.write_buffer(
            &self.view_buffer,
            0,
            bytemuck::cast_slice(&[current_uniforms]),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Brush Logic
        if self.interaction.mouse_pressed {
            let to_grid = |screen_pos: [f32; 2]| -> [f32; 2] {
                let screen_center_x = self.config.width as f32 / 2.0;
                let screen_center_y = self.config.height as f32 / 2.0;
                let offset_x = screen_pos[0] - screen_center_x;
                let offset_y = screen_pos[1] - screen_center_y;
                let zoom = self.gui.params.zoom_level;
                // Use sim.width/height
                let grid_center_x = self.sim.width as f32 / 2.0;
                let grid_center_y = self.sim.height as f32 / 2.0;
                [
                    grid_center_x + (offset_x / zoom),
                    grid_center_y + (offset_y / zoom),
                ]
            };

            let current_grid = to_grid(self.interaction.mouse_pos);
            let last_grid = to_grid(self.interaction.last_mouse_pos);

            let brush_data = BrushUniforms {
                mouse_pos: current_grid,
                last_mouse_pos: last_grid,
                radius: self.gui.params.brush_size / self.gui.params.zoom_level,
                strength: 1.0,
            };

            self.queue.write_buffer(
                &self.brush_pipeline.brush_buffer,
                0,
                bytemuck::cast_slice(&[brush_data]),
            );

            {
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
        }

        record_render_pass(
            &mut encoder,
            &view,
            &self.render_pipeline,
            &self.render_bind_groups[render_index],
            &self.vertex_buffer,
            &self.index_buffer,
            self.num_indices,
        );

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        self.gui.render(
            &self.device,
            &self.queue,
            &mut encoder,
            &self.window,
            &view,
            screen_descriptor,
        );

        // Update Mouse
        self.interaction.last_mouse_pos = self.interaction.mouse_pos;

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        self.frame_num += 1;

        Ok(())
    }
}
