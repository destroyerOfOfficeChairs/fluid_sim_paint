use super::canvas_mod::canvas::Canvas; // Import your new object
use crate::gui_mod::gui::Gui;
use crate::wgpu_utils::wgpu_init;
use std::iter;
use std::sync::Arc;
use winit::event::{ElementState, MouseButton};
use winit::{
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
    window::{Fullscreen, Window},
};

pub struct InteractionState {
    pub mouse_pos: [f32; 2],
    pub last_mouse_pos: [f32; 2],
    pub mouse_pressed: bool,
    pub clear_requested: bool,
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            mouse_pos: [0.0, 0.0],
            last_mouse_pos: [0.0, 0.0],
            mouse_pressed: false,
            clear_requested: false,
        }
    }
}

pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub is_surface_configured: bool,
    pub window: Arc<Window>,

    // COMPOSITION: The Three Pillars
    gui: Gui,
    canvas: Canvas,          // <--- The Engine
    input: InteractionState, // <--- The User
}

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let (surface, device, queue, config) = wgpu_init(window.clone()).await;

        // 1. Init GUI
        let gui = Gui::new(&window, &device, config.format);

        // 2. Init Canvas (The Sim)
        // Notice how we just ask for a "New Canvas" and give it the specs.
        // We don't care about textures or pipelines here anymore.
        let canvas = Canvas::new(
            &device,
            &config,
            gui.params.canvas_width,
            gui.params.canvas_height,
            gui.params.zoom_level,
        );

        // 3. Init Input
        let input = InteractionState::default();

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            window,
            gui,
            canvas,
            input,
        })
    }

    // Input handlers just update 'self.input'
    pub fn handle_mouse(&mut self, pos: [f32; 2]) {
        self.input.mouse_pos = pos;
    }

    pub fn handle_click(&mut self, state: ElementState, button: MouseButton) {
        if button == MouseButton::Left {
            self.input.mouse_pressed = state == ElementState::Pressed;
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
            KeyCode::Delete => {
                self.input.clear_requested = true;
            }
            _ => {}
        }
    }

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

        // UPDATE CANVAS (Physics & Input)
        self.canvas.update(
            &self.queue,
            &mut encoder,
            &self.input,
            &self.gui.params,
            (self.config.width, self.config.height),
        );
        self.input.clear_requested = false;

        // RENDER CANVAS (Draw to Screen)
        self.canvas.render(
            &self.queue,
            &mut encoder,
            &view,
            &self.gui.params,
            (self.config.width, self.config.height),
        );

        // RENDER GUI
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

        // Cleanup
        self.input.last_mouse_pos = self.input.mouse_pos;
        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
