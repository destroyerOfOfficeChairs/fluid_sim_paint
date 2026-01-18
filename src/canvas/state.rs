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
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance, // Request decent GPU
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync, // Vsync ON
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        // -----------------------------------------------------------------------
        // 1. Create the Ping-Pong Textures
        // -----------------------------------------------------------------------
        // We use a fixed grid size for the simulation (e.g., 512x512)
        // This is independent of the window size!
        let sim_width = 512;
        let sim_height = 512;

        let texture_a = texture::Texture::create_storage_texture(
            &device,
            sim_width,
            sim_height,
            Some("Texture A"),
        );
        let texture_b = texture::Texture::create_storage_texture(
            &device,
            sim_width,
            sim_height,
            Some("Texture B"),
        );

        // -----------------------------------------------------------------------
        // 2. Initial Data Upload (Random Noise)
        // -----------------------------------------------------------------------
        // We fill Texture A with random noise so we have something to fade.
        let mut initial_data = Vec::with_capacity((sim_width * sim_height * 4) as usize);
        for _ in 0..(sim_width * sim_height) {
            let r: u8 = rand::random();
            let g: u8 = rand::random();
            let b: u8 = rand::random();
            initial_data.extend_from_slice(&[r, g, b, 255]);
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture_a.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &initial_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * sim_width),
                rows_per_image: Some(sim_height),
            },
            wgpu::Extent3d {
                width: sim_width,
                height: sim_height,
                depth_or_array_layers: 1,
            },
        );

        // -----------------------------------------------------------------------
        // 3. Compute Pipeline Setup
        // -----------------------------------------------------------------------
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/compute.wgsl").into()),
        });

        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Compute Bind Group Layout"),
                entries: &[
                    // Binding 0: Input Texture (Read Only)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // Binding 1: Output Texture (Storage Write)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba8Unorm, // Must match texture creation!
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compute Pipeline Layout"),
                bind_group_layouts: &[&compute_bind_group_layout],
                immediate_size: 0,
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // -----------------------------------------------------------------------
        // 4. Render Pipeline Setup (The Visualization)
        // -----------------------------------------------------------------------
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Render Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        });

        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Render Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&render_bind_group_layout],
                immediate_size: 0,
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // -----------------------------------------------------------------------
        // 5. Create All Bind Groups
        // -----------------------------------------------------------------------

        // COMPUTE A: Read A -> Write B
        let compute_bind_group_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_a.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_b.view),
                },
            ],
            label: Some("Compute Bind Group A"),
        });

        // COMPUTE B: Read B -> Write A
        let compute_bind_group_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_b.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_a.view),
                },
            ],
            label: Some("Compute Bind Group B"),
        });

        // RENDER A: Draw A
        let render_bind_group_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &render_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_a.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture_a.sampler),
                },
            ],
            label: Some("Render Bind Group A"),
        });

        // RENDER B: Draw B
        let render_bind_group_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &render_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_b.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture_b.sampler),
                },
            ],
            label: Some("Render Bind Group B"),
        });

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
