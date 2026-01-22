use egui::Context;
use egui_wgpu::{Renderer, RendererOptions}; // Import RendererOptions
use egui_winit::State as EguiWinitState;
use wgpu::{Device, Queue, TextureFormat};
use winit::{event::WindowEvent, window::Window};

pub struct GuiParams {
    pub zoom_level: f32,
    pub brush_size: f32,
    pub canvas_width: u32,
    pub canvas_height: u32,
}

impl Default for GuiParams {
    fn default() -> Self {
        Self {
            zoom_level: 0.8,
            brush_size: 20.0,
            canvas_width: 1920,
            canvas_height: 1080,
        }
    }
}

pub struct Gui {
    pub context: Context,
    state: EguiWinitState,
    renderer: Renderer,
    pub params: GuiParams,
}

impl Gui {
    pub fn new(window: &Window, device: &Device, output_format: TextureFormat) -> Self {
        let context = Context::default();

        let id = context.viewport_id();

        let state = EguiWinitState::new(context.clone(), id, window, None, None, None);

        let renderer_options = RendererOptions {
            ..Default::default()
        };

        let renderer = Renderer::new(device, output_format, renderer_options);

        Self {
            context,
            state,
            renderer,
            params: GuiParams::default(),
        }
    }

    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) {
        let _ = self.state.on_window_event(window, event);
    }

    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut wgpu::CommandEncoder,
        window: &Window,
        view: &wgpu::TextureView,
        screen_descriptor: egui_wgpu::ScreenDescriptor,
    ) {
        let raw_input = self.state.take_egui_input(window);
        self.context.begin_pass(raw_input);

        egui::Window::new("Controls")
            .resizable(true)
            .vscroll(true)
            .default_width(200.0)
            .show(&self.context, |ui| {
                ui.heading("Settings");
                ui.separator();

                ui.label("Brush Settings");
                ui.add(egui::Slider::new(&mut self.params.brush_size, 1.0..=100.0).text("Size"));

                ui.separator();
                ui.label("View Settings");
                ui.add(egui::Slider::new(&mut self.params.zoom_level, 0.1..=5.0).text("Zoom"));

                ui.separator();
                ui.label("Canvas Dimensions");
                ui.horizontal(|ui| {
                    ui.label("W:");
                    ui.add(egui::DragValue::new(&mut self.params.canvas_width));
                    ui.label("H:");
                    ui.add(egui::DragValue::new(&mut self.params.canvas_height));
                });
            });

        // Tessellate shapes into primitives
        let output = self.context.end_pass();
        let primitives = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);

        // Update Textures
        for (id, image_delta) in &output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        // Update Buffers (Upload geometry to GPU)
        self.renderer
            .update_buffers(device, queue, encoder, &primitives, &screen_descriptor);

        // Begin Render Pass
        let mut render_pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();

        self.renderer
            .render(&mut render_pass, &primitives, &screen_descriptor);

        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }
}
