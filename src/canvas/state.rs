use super::super::gui_mod::gui::Gui;
use super::state_new_helpers::brush_pipeline::{BrushPipeline, BrushUniforms};
use super::state_new_helpers::quad::create_canvas_quad;
use super::state_new_helpers::render_pipeline::{ViewUniforms, create_render_setup};
use super::state_new_helpers::texture::Texture;
use super::state_new_helpers::texture::create_sim_textures;
use super::state_new_helpers::wgpu_init::wgpu_init;
use super::state_render_helpers::draw::record_render_pass;
use std::iter;
use std::sync::Arc;
use wgpu::Buffer;
use wgpu::util::DeviceExt; // Needed for create_buffer_init
use winit::event::{ElementState, MouseButton};
use winit::{
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
    window::{Fullscreen, Window},
};

#[allow(dead_code)]
pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub is_surface_configured: bool,
    pub window: Arc<Window>,

    gui: Gui,
    view_buffer: wgpu::Buffer,
    pub mouse_pos: [f32; 2],
    pub mouse_pressed: bool,

    vertex_buffer: Buffer,
    index_buffer: Buffer,
    num_indices: u32,

    sim_width: u32,
    sim_height: u32,

    density_a: Texture,
    density_b: Texture,
    velocity_a: Texture,
    velocity_b: Texture,
    pressure_a: Texture,
    pressure_b: Texture,
    divergence: Texture,

    frame_num: usize,

    render_pipeline: wgpu::RenderPipeline,
    render_bind_group: wgpu::BindGroup,
    brush_pipeline: BrushPipeline,
}

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let (surface, device, queue, config) = wgpu_init(window.clone()).await;

        let gui = Gui::new(&window, &device, config.format);
        let initial_uniforms = ViewUniforms {
            screen_size: [config.width as f32, config.height as f32],
            canvas_size: [1920.0, 1080.0],
            pan: [0.0, 0.0],
            zoom: gui.params.zoom_level,
            _padding: 0,
        };
        let view_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("View Uniform Buffer"),
            contents: bytemuck::cast_slice(&[initial_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let sim_width = gui.params.canvas_width;
        let sim_height = gui.params.canvas_height;
        let (density_a, density_b, velocity_a, velocity_b, pressure_a, pressure_b, divergence) =
            create_sim_textures(&device, sim_width, sim_height);

        let (vertex_buffer, index_buffer, num_indices) = create_canvas_quad(&device);

        // Create the Render Pipeline (The "Viewer")
        // We bind density_a for now. Since it's empty, it will look like a white page.
        let (render_pipeline, render_bind_group) =
            create_render_setup(&device, &config, &density_a, &view_buffer);

        let brush_pipeline = BrushPipeline::new(&device);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            window,
            gui,
            view_buffer,
            mouse_pos: [0.0, 0.0],
            mouse_pressed: false,
            vertex_buffer,
            index_buffer,
            num_indices,
            sim_width,
            sim_height,
            density_a,
            density_b,
            velocity_a,
            velocity_b,
            pressure_a,
            pressure_b,
            divergence,
            frame_num: 0,
            render_pipeline,
            render_bind_group,
            brush_pipeline,
        })
    }

    // NEW: Helper to update mouse state
    pub fn handle_mouse(&mut self, pos: [f32; 2]) {
        self.mouse_pos = pos;
    }

    pub fn handle_click(&mut self, state: ElementState, button: MouseButton) {
        if button == MouseButton::Left {
            self.mouse_pressed = state == ElementState::Pressed;
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

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Get data from GUI
        let current_uniforms = ViewUniforms {
            // The Real Window Size
            screen_size: [self.config.width as f32, self.config.height as f32],

            // The Desired Canvas Size (From your UI)
            canvas_size: [
                self.gui.params.canvas_width as f32,
                self.gui.params.canvas_height as f32,
            ],

            pan: [0.0, 0.0],
            zoom: self.gui.params.zoom_level,
            _padding: 0,
        };

        // Write to GPU
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

        // --- BRUSH STEP ---

        if self.mouse_pressed {
            // 1. Calculate Grid Coordinates (The Math)
            // Screen Center
            let screen_center_x = self.config.width as f32 / 2.0;
            let screen_center_y = self.config.height as f32 / 2.0;

            // Mouse offset from center
            let offset_x = self.mouse_pos[0] - screen_center_x;
            let offset_y = self.mouse_pos[1] - screen_center_y;

            // Apply Zoom (Inverse)
            let zoom = self.gui.params.zoom_level;
            let unscaled_x = offset_x / zoom;
            let unscaled_y = offset_y / zoom;

            // Grid Center
            let grid_center_x = self.sim_width as f32 / 2.0;
            let grid_center_y = self.sim_height as f32 / 2.0;

            // Final Grid Pos
            let grid_x = grid_center_x + unscaled_x;
            let grid_y = grid_center_y + unscaled_y; // Note: Y might need flipping depending on coords

            // 2. Update Uniforms
            let brush_data = BrushUniforms {
                mouse_pos: [grid_x, grid_y],
                radius: self.gui.params.brush_size / zoom, // Scale radius so it stays visually consistent
                strength: 0.5,                             // Hardcoded opacity for now
            };

            self.queue.write_buffer(
                &self.brush_pipeline.brush_buffer,
                0,
                bytemuck::cast_slice(&[brush_data]),
            );

            // 3. Create Bind Group (Connect Input A -> Output B)
            // Note: We are reading A and writing B. We should actually be writing to 'density_b'
            // and reading 'density_a', then swapping. For this simple step, let's just use A -> B.
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Brush Bind Group"),
                layout: &self.brush_pipeline.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.brush_pipeline.brush_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.density_a.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&self.density_b.view),
                    },
                ],
            });

            // 4. Dispatch Compute Shader
            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Brush Pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.brush_pipeline.pipeline);
                compute_pass.set_bind_group(0, &bind_group, &[]);
                // Dispatch: Width/16, Height/16
                let x_groups = (self.sim_width as f32 / 16.0).ceil() as u32;
                let y_groups = (self.sim_height as f32 / 16.0).ceil() as u32;
                compute_pass.dispatch_workgroups(x_groups, y_groups, 1);
            }

            // 5. SWAP TEXTURES (Cheap trick for Phase 1)
            // Since we wrote to B, we want to Render B.
            // But our Render Pipeline is bound to A.
            // For this specific test, let's just Copy B back to A so we see it.
            encoder.copy_texture_to_texture(
                self.density_b.texture.as_image_copy(),
                self.density_a.texture.as_image_copy(),
                self.density_a.texture.size(),
            );
        }

        // Draw the Canvas
        record_render_pass(
            &mut encoder,
            &view,
            &self.render_pipeline,
            &self.render_bind_group,
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

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        self.frame_num += 1;

        Ok(())
    }
}
