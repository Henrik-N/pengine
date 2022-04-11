mod component_editor;
mod scene;
mod stats;

use crate::{events, input, time, GraphicsContext};

mod leg {
    pub use legion::storage::*;
    pub use legion::world::*;
}
pub use component_editor::{ComponentEditor, EditorComponentStorage};

/// Data that the UI needs every frame
pub struct FrameData<'a> {
    pub clock: &'a time::Clock,
    pub l_world: &'a mut legion::world::World,
    pub ui_storage: &'a component_editor::EditorComponentStorage,
}

/// Contains the necessary data for rendering and managing the editor and it's UI.
pub struct EditorState {
    pub platform: egui_winit_platform::Platform,
    pub render_pass: egui_wgpu_backend::RenderPass,
    // render pass data ---------
    paint_jobs: Vec<egui::ClippedMesh>,
    screen_descriptor: egui_wgpu_backend::ScreenDescriptor,
    // ----------
    panels: Panels,
    is_consuming_input: bool,
}

/// Contains all UI panels
#[derive(Default)]
struct Panels {
    stats: stats::StatsPanel,
    scene: scene::ScenePanel,
}

impl EditorState {
    pub fn new(context: &GraphicsContext) -> Self {
        let screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
            physical_width: context.size.width as _,
            physical_height: context.size.height as _,
            scale_factor: context.scale_factor as _,
        };

        let platform =
            egui_winit_platform::Platform::new(egui_winit_platform::PlatformDescriptor {
                physical_width: screen_descriptor.physical_width,
                physical_height: screen_descriptor.physical_height,
                scale_factor: screen_descriptor.scale_factor as _,
                font_definitions: egui::FontDefinitions::default(),
                style: egui::style::Style::default(),
            });

        let render_pass = egui_wgpu_backend::RenderPass::new(
            &context.device,
            context
                .surface
                .get_preferred_format(&context.adapter)
                .unwrap(),
            1,
        );

        Self {
            platform,
            render_pass,
            paint_jobs: vec![],
            screen_descriptor,
            panels: Panels::default(),
            is_consuming_input: false,
        }
    }

    /// Called on a winit::event::Event
    pub fn handle_platform_event<T>(&mut self, event: &winit::event::Event<T>) {
        self.is_consuming_input = false;

        self.platform.handle_event(event);

        if self.platform.context().wants_keyboard_input() {
            self.is_consuming_input = true;
        }

        if self.platform.context().is_pointer_over_area() {
            self.is_consuming_input = true;
        }
    }

    /// Called on a PenguinEvent
    pub fn on_event(&mut self, event: &events::PenguinEvent) -> bool {
        use events::{event::WindowResizeEvent, PenguinEvent};

        match event {
            PenguinEvent::Window(WindowResizeEvent { size, scale_factor }) => {
                self.screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
                    physical_width: size.width,
                    physical_height: size.height,
                    scale_factor: if let Some(scale_factor) = scale_factor {
                        *scale_factor as f32
                    } else {
                        self.screen_descriptor.scale_factor
                    },
                };
                false
            }
            PenguinEvent::Input(input::InputEvent::Key(input::KeyEvent { .. })) => {
                self.is_consuming_input
            }
            _ => false,
        }
    }

    /// Update UI
    pub fn update(
        &mut self,
        context: &GraphicsContext,
        window: &winit::window::Window,
        frame_data: &mut FrameData,
    ) {
        self.platform
            .update_time(frame_data.clock.start_time.elapsed().as_secs_f64());
        self.platform.begin_frame();

        self.draw_ui(&self.platform.context(), frame_data);

        let (_output, paint_commands) = self.platform.end_frame(Some(window));
        self.paint_jobs = self.platform.context().tessellate(paint_commands);

        {
            // upload gpu resources
            self.render_pass.update_texture(
                &context.device,
                &context.queue,
                &self.platform.context().font_image(),
            );

            self.render_pass
                .update_user_textures(&context.device, &context.queue);

            self.render_pass.update_buffers(
                &context.device,
                &context.queue,
                &self.paint_jobs,
                &self.screen_descriptor,
            );
        }
    }

    pub fn render_commands(
        &mut self,
        device: &wgpu::Device,
        output: &wgpu::TextureView,
        encoder: Option<wgpu::CommandEncoder>,
    ) -> wgpu::CommandEncoder {
        let mut cmd = match encoder {
            Some(encoder) => encoder,
            None => device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render commands encoder"),
            }),
        };

        self.render_pass
            .execute(
                &mut cmd,
                output,
                &self.paint_jobs,
                &self.screen_descriptor,
                None,
            )
            .expect("failed to execute egui render pass");

        cmd
    }
}

impl EditorState {
    fn draw_ui(&mut self, context: &egui::CtxRef, frame_data: &mut FrameData) {
        Self::top_bar(context, &mut self.panels);

        if self.panels.stats.enabled {
            self.panels.stats.update(context, frame_data);
        }

        if self.panels.scene.enabled {
            self.panels.scene.update(context, frame_data);
        }
    }

    fn top_bar(context: &egui::CtxRef, panels: &mut Panels) {
        egui::TopBottomPanel::top("top menu").show(context, |ui| {
            egui::trace!(ui);

            ui.horizontal_wrapped(|ui| {
                egui::widgets::global_dark_light_mode_switch(ui);

                if cfg!(debug_assertions) {
                    ui.separator();

                    ui.label(
                        egui::RichText::new("Debug build")
                            .small()
                            .color(egui::Color32::RED),
                    )
                    .on_hover_text("This is a debug build of penguin engine.");
                }

                ui.separator();

                ui.checkbox(&mut panels.stats.enabled, "💻 Stats");

                ui.checkbox(&mut panels.scene.enabled, "Scene");
            });
        });
    }
}
