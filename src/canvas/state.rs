use super::state_new_helpers::quad::create_canvas_quad;
use super::state_new_helpers::texture::Texture;
use super::state_new_helpers::texture::create_sim_textures;
use super::state_new_helpers::wgpu_init::wgpu_init;
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

        // TODO: implement render function

        self.frame_num += 1;

        Ok(())
    }
}
