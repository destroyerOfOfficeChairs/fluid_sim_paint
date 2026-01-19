use super::state_new_helpers::compute_pipeline::create_compute_setup;
use super::state_new_helpers::quad::create_canvas_quad;
use super::state_new_helpers::render_pipeline::create_render_setup;
use super::state_new_helpers::resource_setup::create_ping_pong_textures;
use super::state_new_helpers::wgpu_init::wgpu_init;
use super::state_render_helpers::compute::record_compute_pass;
use super::state_render_helpers::draw::record_render_pass;
use crate::texture;
use std::{iter, sync::Arc};
use wgpu::util::DeviceExt;
use winit::{
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
    window::{Fullscreen, Window},
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SimParams {
    time: f32,
    dt: f32,
    padding: [f32; 2], // GPU structs must be 16-byte aligned
}

pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub is_surface_configured: bool,
    pub window: Arc<Window>,

    // Render Pipeline (Drawing to Screen)
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,

    // Compute Pipeline (The Physics/Simulation)
    compute_pipeline: wgpu::ComputePipeline,
    params_buffer: wgpu::Buffer,

    // The Ping-Pong Resources
    sim_width: u32,
    sim_height: u32,

    #[allow(unused)]
    texture_a: texture::Texture,
    #[allow(unused)]
    texture_b: texture::Texture,

    #[allow(unused)]
    velocity_a: texture::Texture,
    #[allow(unused)]
    velocity_b: texture::Texture,

    // Bind Groups for COMPUTING (Input -> Output)
    compute_bind_group_a: wgpu::BindGroup, // Read A -> Write B
    compute_bind_group_b: wgpu::BindGroup, // Read B -> Write A

    // Bind Groups for RENDERING (Sampling)
    render_bind_group_a: wgpu::BindGroup, // Draw A
    render_bind_group_b: wgpu::BindGroup, // Draw B

    frame_num: usize,
}

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let (surface, device, queue, config) = wgpu_init(window.clone()).await;

        // Hardcoded for now, but these values need to be handled dynamically in the future.
        let sim_width = 80;
        let sim_height = 45;
        let (texture_a, texture_b, velocity_a, velocity_b) =
            create_ping_pong_textures(&device, &queue, sim_width, sim_height);

        // 1. Create the initial data
        let sim_params = SimParams {
            time: 0.0,
            dt: 0.0,
            padding: [0.0; 2],
        };

        // 2. Create the Buffer (using wgpu::util::DeviceExt)
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sim Params Buffer"),
            contents: bytemuck::cast_slice(&[sim_params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let (compute_pipeline, compute_bind_group_a, compute_bind_group_b) =
            create_compute_setup(&device, &texture_a, &texture_b, &velocity_a, &params_buffer);

        let (render_pipeline, render_bind_group_a, render_bind_group_b) =
            create_render_setup(&device, &config, &texture_a, &texture_b);

        let (vertex_buffer, index_buffer, num_indices) = create_canvas_quad(&device);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            window,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
            compute_pipeline,
            params_buffer,
            sim_width,
            sim_height,
            texture_a,
            texture_b,
            velocity_a,
            velocity_b,
            compute_bind_group_a,
            compute_bind_group_b,
            render_bind_group_a,
            render_bind_group_b,
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

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Update the time uniform
        let params = SimParams {
            time: self.frame_num as f32 * 0.01,
            dt: 1.0, // Hardcode 1.0 for now so the wind is strong enough to see
            padding: [0.0; 2],
        };
        self.queue
            .write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[params]));

        // 1. Run Physics
        record_compute_pass(
            &mut encoder,
            &self.compute_pipeline,
            &self.compute_bind_group_a,
            &self.compute_bind_group_b,
            self.frame_num,
            self.sim_width,
            self.sim_height,
        );

        // 2. Draw to Screen
        record_render_pass(
            &mut encoder,
            &view,
            &self.render_pipeline,
            &self.render_bind_group_a,
            &self.render_bind_group_b,
            &self.vertex_buffer,
            &self.index_buffer,
            self.num_indices,
            self.frame_num,
        );

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        self.frame_num += 1;

        Ok(())
    }
}
