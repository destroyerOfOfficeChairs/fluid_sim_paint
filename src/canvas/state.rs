use super::state_new_helpers::quad::create_canvas_quad;
use super::state_new_helpers::render_pipeline::create_render_setup;
use super::state_new_helpers::texture::Texture;
use super::state_new_helpers::texture::create_sim_textures;
use super::state_new_helpers::wgpu_init::wgpu_init;
use super::state_render_helpers::draw::record_render_pass;
use std::iter;
use std::sync::Arc;
use wgpu::Buffer;
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
}

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let (surface, device, queue, config) = wgpu_init(window.clone()).await;

        // Hardcoded for now, but these values need to be handled dynamically in the future.
        let sim_width = 80;
        let sim_height = 45;
        let (density_a, density_b, velocity_a, velocity_b, pressure_a, pressure_b, divergence) =
            create_sim_textures(&device, sim_width, sim_height);

        let (vertex_buffer, index_buffer, num_indices) = create_canvas_quad(&device);

        // Create the Render Pipeline (The "Viewer")
        // We bind density_a for now. Since it's empty, it will look like a white page.
        let (render_pipeline, render_bind_group) =
            create_render_setup(&device, &config, &density_a);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            window,
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
        })
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

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

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

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        self.frame_num += 1;

        Ok(())
    }
}
