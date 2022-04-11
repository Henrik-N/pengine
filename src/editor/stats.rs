use super::FrameData;
use crate::time;

pub struct StatsPanel {
    pub enabled: bool,
    frame_time_history: FrameTimeHistory,
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

mod panel {
    use super::*;

    impl std::default::Default for StatsPanel {
        fn default() -> Self {
            Self {
                enabled: true,
                frame_time_history: FrameTimeHistory::default(),
            }
        }
    }

    impl StatsPanel {
        pub fn update(&mut self, context: &egui::CtxRef, frame_data: &FrameData) {
            self.frame_time_history.update(frame_data.clock);

            context.request_repaint();

            egui::SidePanel::left("stats panel").show(context, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("💻 Stats");
                });

                ui.separator();

                self.frame_time_history.ui(ui);
            });
        }
    }
}

mod frame_time_history {
    use super::*;

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
        pub fn update(&mut self, clock: &time::Clock) {
            self.frame_times.add(
                clock.start_time.elapsed().as_secs_f64(),
                clock.last_delta_time.as_secs_f32(),
            );
        }

        pub fn ui(&mut self, ui: &mut egui::Ui) {
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

                    ui.horizontal_wrapped(|ui| {
                        ui.radio_value(&mut self.graph_style, GraphStyle::Histogram, "Histogram");
                        ui.radio_value(&mut self.graph_style, GraphStyle::LineGraph, "LineGraph");
                    });
                });
        }

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
                        let point_pos = to_graph_rect
                            .transform_pos_clamped(egui::Pos2::new(x_age, frame_duration));

                        // line from point to previous point
                        let line_from = match previous_pos {
                            Some(previous_pos) => previous_pos,
                            None => {
                                previous_pos = Some(point_pos);
                                continue;
                            }
                        };
                        let line_to = point_pos;

                        let line_shape =
                            egui::Shape::line_segment([line_from, line_to], line_stroke);
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
    }
}
