use crate::time;
use crate::RendererState;

/// Data that the UI needs every frame
pub struct FrameData<'a> {
    pub clock: &'a time::Clock,
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
}

/// Contains all UI panels
#[derive(Default)]
struct Panels {
    stats: StatsPanel,
}

impl EditorState {
    pub fn new(state: &RendererState, window: &winit::window::Window) -> Self {
        let screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
            physical_width: state.size.width as _,
            physical_height: state.size.height as _,
            scale_factor: window.scale_factor() as _, // todo: maybe add window.scale_factor() to support that
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
            &state.device,
            state.surface.get_preferred_format(&state.adapter).unwrap(),
            1,
        );

        Self {
            platform,
            render_pass,
            paint_jobs: vec![],
            screen_descriptor,
            panels: Panels::default(),
        }
    }

    pub fn handle_event<T>(&mut self, event: &winit::event::Event<T>) {
        self.platform.handle_event(event);
    }

    /// Update UI
    pub fn update(
        &mut self,
        state: &RendererState,
        window: &winit::window::Window,
        frame_data: &FrameData,
    ) {
        self.platform.begin_frame();

        let context = self.platform.context();

        self.draw_ui(&context, frame_data);

        let (_output, paint_commands) = self.platform.end_frame(Some(window));
        self.paint_jobs = self.platform.context().tessellate(paint_commands);

        // upload gpu resources
        self.screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
            physical_width: state.size.width,
            physical_height: state.size.height,
            scale_factor: window.scale_factor() as _,
        };
        self.render_pass.update_texture(
            &state.device,
            &state.queue,
            &self.platform.context().font_image(),
        );
        self.render_pass
            .update_user_textures(&state.device, &state.queue);
        self.render_pass.update_buffers(
            &state.device,
            &state.queue,
            &self.paint_jobs,
            &self.screen_descriptor,
        );
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
    fn draw_ui(&mut self, context: &egui::CtxRef, frame_data: &FrameData) {
        Self::top_bar(context, &mut self.panels);
        if self.panels.stats.enabled {
            self.panels.stats.update(context, frame_data);
        }
    }

    fn top_bar(context: &egui::CtxRef, panels: &mut Panels) {
        egui::TopBottomPanel::top("top menu").show(context, |ui| {
            egui::trace!(ui);

            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut panels.stats.enabled, "💻 General");
                ui.separator();

                if cfg!(debug_assertions) {
                    ui.label(
                        egui::RichText::new("Debug build")
                            .small()
                            .color(egui::Color32::RED),
                    )
                    .on_hover_text("This is a debug build of penguin engine.");
                }

                egui::widgets::global_dark_light_mode_switch(ui);
            });
        });
    }
}

struct StatsPanel {
    enabled: bool,
    frame_time_history: FrameTimeHistory,
}
impl std::default::Default for StatsPanel {
    fn default() -> Self {
        Self {
            enabled: true,
            frame_time_history: FrameTimeHistory::default(),
        }
    }
}

#[derive(PartialEq)]
enum GraphStyle {
    Histogram,
    LineGraph,
}

struct FrameTimeHistory {
    frame_times: egui::util::History<f32>,
    graph_style: GraphStyle,
}
impl std::default::Default for FrameTimeHistory {
    fn default() -> Self {
        // at most 1 second between updates
        let max_age = 1.0_f32;
        let max_length = (max_age * 150.0_f32).round() as _;

        Self {
            frame_times: egui::util::History::new(0..max_length, max_age),
            graph_style: GraphStyle::Histogram,
        }
    }
}

impl FrameTimeHistory {
    fn average_frame_time(&self) -> f32 {
        self.frame_times.average().unwrap_or_default()
    }

    fn average_fps(&self) -> f32 {
        1.0 / self.frame_times.mean_time_interval().unwrap_or_default()
    }

    fn graph(&mut self, ui: &mut egui::Ui) {
        let history = &self.frame_times;

        let height = 100.0_f32;
        let size = egui::vec2(ui.available_size_before_wrap().x, height);
        let (rect, response) = ui.allocate_at_least(size, egui::Sense::hover());
        let style = ui.style().noninteractive();

        // rect containing the graph
        let graph_rect = egui::Shape::Rect(egui::epaint::RectShape {
            rect,
            corner_radius: style.corner_radius,
            fill: ui.visuals().extreme_bg_color,
            stroke: ui.style().noninteractive().bg_stroke,
        });

        let graph_top_y_value = 0.100; // graph's highest point

        let to_graph_rect: egui::emath::RectTransform = {
            let x_range = history.max_age()..=0.0;
            let y_range = graph_top_y_value..=0.0;

            let graph_rect = egui::Rect::from_x_y_ranges(x_range, y_range);

            egui::emath::RectTransform::from_to(graph_rect, rect)
        };

        let mut shapes = Vec::with_capacity(1 + 2 * self.frame_times.len());
        shapes.push(graph_rect);

        let color = ui.visuals().text_color();
        let radius = 1.0;
        let rightmost_time = ui.input().time;
        let line_stroke = egui::Stroke::new(1.0, color);

        match self.graph_style {
            GraphStyle::Histogram => {
                let inner_shapes = history
                    .iter()
                    .flat_map(|(time, frame_duration)| {
                        let x_age = (rightmost_time - time) as f32;
                        let point_pos = to_graph_rect
                            .transform_pos_clamped(egui::Pos2::new(x_age, frame_duration));

                        // line from bottom to top
                        let line_from = egui::pos2(point_pos.x, rect.bottom());
                        let line_to = point_pos;

                        let line_shape =
                            egui::Shape::line_segment([line_from, line_to], line_stroke);

                        // circle on top
                        let circle_shape = egui::Shape::circle_filled(point_pos, radius, color);

                        [line_shape, circle_shape]
                    })
                    .collect::<Vec<_>>();

                shapes.extend(inner_shapes);
            }
            GraphStyle::LineGraph => {
                let mut inner_shapes = Vec::with_capacity(history.len() * 2);

                let mut previous_pos = None;

                for (time, frame_duration) in history.iter() {
                    let x_age = (rightmost_time - time) as f32;
                    let point_pos =
                        to_graph_rect.transform_pos_clamped(egui::Pos2::new(x_age, frame_duration));

                    // line from point to previous point
                    let line_from = match previous_pos {
                        Some(previous_pos) => previous_pos,
                        None => {
                            previous_pos = Some(point_pos);
                            continue;
                        }
                    };
                    let line_to = point_pos;

                    let line_shape = egui::Shape::line_segment([line_from, line_to], line_stroke);
                    inner_shapes.push(line_shape);

                    previous_pos = Some(point_pos);

                    // circle on top
                    inner_shapes.push(egui::Shape::circle_filled(point_pos, radius, color));
                }

                shapes.extend(inner_shapes);
            }
        }

        // mouse interactivity
        let rect = rect.shrink(4.0);
        let color = ui.visuals().text_color();
        let line_stroke = egui::Stroke::new(1.0, color);

        if let Some(pointer_pos) = response.hover_pos() {
            let y = pointer_pos.y;
            shapes.push(egui::Shape::line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                line_stroke,
            ));

            let cpu_usage = to_graph_rect.inverse().transform_pos(pointer_pos).y;
            let text = format!("{:.1} ms", 1e3 * cpu_usage);
            shapes.push(egui::Shape::text(
                ui.fonts(),
                egui::pos2(rect.left(), y),
                egui::Align2::LEFT_BOTTOM,
                text,
                egui::epaint::TextStyle::Monospace,
                color,
            ));
        }

        ui.painter().extend(shapes);
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.label(format!("Frame: {}", self.frame_times.total_count(),));

        ui.label(format!(
            "Frame time (avg): {:.2} ms",
            1e3 * self.average_frame_time()
        ));

        ui.label(format!("FPS (avg): {:.0}", self.average_fps()));

        egui::CollapsingHeader::new("📊 CPU usage")
            .default_open(true)
            .show(ui, |ui| {
                // graph
                ui.label("Frame time history:");

                self.graph(ui);

                // graph style selection
                ui.radio_value(&mut self.graph_style, GraphStyle::Histogram, "Histogram");
                ui.radio_value(&mut self.graph_style, GraphStyle::LineGraph, "LineGraph");
            });
    }

    fn update(&mut self, clock: &time::Clock) {
        self.frame_times.add(
            clock.start_time.elapsed().as_secs_f64(),
            clock.last_delta_time.as_secs_f32(),
        );
    }
}

#[derive(Copy, Clone)]
pub struct TimeData {
    pub now: f64,
    pub previous_frame_time: Option<f32>,
}

impl StatsPanel {
    fn update(&mut self, context: &egui::CtxRef, frame_data: &FrameData) {
        self.frame_time_history.update(frame_data.clock);

        context.request_repaint();

        egui::SidePanel::left("stats panel").show(context, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("💻 Stats");
            });

            ui.separator();

            let reset_button = ui
                .button("Reset UI")
                .on_hover_text("Reset UI to default state");

            if reset_button.clicked() {
                *context.memory() = std::default::Default::default();
            }

            ui.separator();

            self.frame_time_history.ui(ui);
        });
    }
}
