use super::FrameData;
use crate::components;
use legion::IntoQuery;

#[derive(Default)]
pub struct ScenePanel {
    pub enabled: bool,
    l_selected_entity: Option<legion::Entity>,
}
impl ScenePanel {
    pub fn update(&mut self, context: &egui::CtxRef, frame_data: &mut FrameData) {
        egui::SidePanel::right("scene panel")
            .default_width(250.)
            .show(context, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Scene");
                    ui.separator();
                });

                let mut query = <(legion::Entity, &components::EntityName)>::query();

                for (ent, name) in query.iter(frame_data.l_world) {
                    if ui.small_button(&name.0).clicked() {
                        self.l_selected_entity = Some(*ent);

                        frame_data.ui_storage.select_entity(*ent);

                        break;
                    }
                }

                // draw entity ui if an entity is selected
                if let Some(e) = self.l_selected_entity {
                    frame_data.ui_storage.draw_entities_component_editors(
                        &mut frame_data.l_world,
                        e,
                        ui,
                    );
                }

                ui.separator();
            });
    }
}
