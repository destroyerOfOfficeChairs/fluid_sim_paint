use super::state_helpers::compute_pipeline::create_compute_setup;
use super::state_helpers::render_pipeline::create_render_setup;
use super::state_helpers::resource_setup::create_ping_pong_textures;
use super::state_helpers::wgpu_init::wgpu_init;
use crate::canvas::quad::*;
use crate::texture;
use std::{iter, sync::Arc};
use wgpu::util::DeviceExt;
use winit::{
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
    window::{Fullscreen, Window},
};

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

    // The Ping-Pong Resources
    texture_a: texture::Texture,
    texture_b: texture::Texture,

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

        let (texture_a, texture_b) = create_ping_pong_textures(&device, &queue);

        let (compute_pipeline, compute_bind_group_a, compute_bind_group_b) =
            create_compute_setup(&device, &texture_a, &texture_b);

        let (render_pipeline, render_bind_group_a, render_bind_group_b) =
            create_render_setup(&device, &config, &texture_a, &texture_b);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

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
            num_indices: INDICES.len() as u32,
            compute_pipeline,
            texture_a,
            texture_b,
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

        // -------------------------------------------------------------------
        // 1. COMPUTE PASS (The Physics)
        // -------------------------------------------------------------------
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.compute_pipeline);

            // Ping-Pong Logic
            if self.frame_num % 2 == 0 {
                // Even Frame: Read A -> Write B
                compute_pass.set_bind_group(0, &self.compute_bind_group_a, &[]);
            } else {
                // Odd Frame: Read B -> Write A
                compute_pass.set_bind_group(0, &self.compute_bind_group_b, &[]);
            }

            // Dispatch 512x512 threads (in blocks of 16x16)
            // 512 / 16 = 32
            compute_pass.dispatch_workgroups(32, 32, 1);
        }

        // -------------------------------------------------------------------
        // 2. RENDER PASS (The Drawing)
        // -------------------------------------------------------------------
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);

            // Determine which texture holds the "latest" result to draw
            if self.frame_num % 2 == 0 {
                // We just wrote to B, so draw B
                render_pass.set_bind_group(0, &self.render_bind_group_b, &[]);
            } else {
                // We just wrote to A, so draw A
                render_pass.set_bind_group(0, &self.render_bind_group_a, &[]);
            }

            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        self.frame_num += 1;

        Ok(())
    }
}
