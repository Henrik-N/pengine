use super::*;
use crate::editor::ComponentEditor;

mod entity_name {
    use super::*;

    impl ComponentEditor for Name {
        type ComponentEditorState = ();

        fn init_component_editor_state(&self) -> Self::ComponentEditorState {
            ()
        }

        fn penguin_editor(&mut self, ui: &mut Ui, _state: &mut Self::ComponentEditorState) {
            ui.separator();
            ui.text_edit_singleline(&mut self.0);
        }
    }
}

mod translation {
    use super::*;

    impl ComponentEditor for Translation {
        type ComponentEditorState = ();

        fn init_component_editor_state(&self) -> Self::ComponentEditorState {
            ()
        }

        fn penguin_editor(&mut self, ui: &mut Ui, _state: &mut Self::ComponentEditorState) {
            egui::CollapsingHeader::new("Translation")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(egui::DragValue::new(&mut self.x).speed(0.1));
                        ui.separator();
                        ui.add(egui::DragValue::new(&mut self.y).speed(0.1));
                        ui.separator();
                        ui.add(egui::DragValue::new(&mut self.z).speed(0.1));
                    });
                });
        }
    }
}

mod rotation {
    use super::*;

    /// Keeps a state in euler angles when modifying the rotation
    #[derive(Clone, PartialEq, Default)]
    pub struct RotationEditorState {
        euler: m::Vec3,
    }

    impl ComponentEditor for Rotation {
        type ComponentEditorState = RotationEditorState;

        fn init_component_editor_state(&self) -> Self::ComponentEditorState {
            RotationEditorState {
                euler: self.0.to_euler(m::EulerRot::XYZ).into(),
            }
        }

        fn penguin_editor(&mut self, ui: &mut Ui, state: &mut Self::ComponentEditorState) {
            fn drag_angle_tau(ui: &mut Ui, rads: &mut f32) {
                use std::f32::consts::TAU;

                let mut taus = *rads / TAU;
                let _response = ui.add(
                    egui::DragValue::new(&mut taus)
                        .speed(0.01)
                        .suffix("τ")
                        .fixed_decimals(2)
                        .min_decimals(2),
                );

                *rads = taus * TAU;
            }

            fn drag_angle(ui: &mut Ui, rads: &mut f32) {
                let mut degrees = rads.to_degrees();
                let _response = ui.add(
                    egui::DragValue::new(&mut degrees)
                        .speed(1.0)
                        .suffix("°")
                        .fixed_decimals(2)
                        .min_decimals(2),
                );

                *rads = degrees.to_radians();
            }

            let previous = state.clone();

            egui::CollapsingHeader::new("Rotation")
                .default_open(true)
                .show(ui, |ui| {
                    ui.label("Degs");
                    ui.horizontal_wrapped(|ui| {
                        drag_angle(ui, &mut state.euler.x);
                        drag_angle(ui, &mut state.euler.y);
                        drag_angle(ui, &mut state.euler.z);
                    });

                    ui.label("Rads");
                    ui.vertical_centered_justified(|ui| {
                        ui.horizontal(|ui| {
                            drag_angle_tau(ui, &mut state.euler.x);
                            drag_angle_tau(ui, &mut state.euler.y);
                            drag_angle_tau(ui, &mut state.euler.z);
                        });
                    });
                });

            if *state != previous {
                self.0 = m::Quat::from_euler(
                    m::EulerRot::XYZ,
                    state.euler.x,
                    state.euler.y,
                    state.euler.z,
                );
            }
        }
    }
}

mod scale {
    use super::*;

    impl ComponentEditor for Scale {
        type ComponentEditorState = ();

        fn init_component_editor_state(&self) -> Self::ComponentEditorState {
            ()
        }

        fn penguin_editor(&mut self, ui: &mut Ui, _state: &mut Self::ComponentEditorState) {
            egui::CollapsingHeader::new("Scale")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(egui::DragValue::new(&mut self.x).speed(0.1));
                        ui.add(egui::DragValue::new(&mut self.y).speed(0.1));
                        ui.add(egui::DragValue::new(&mut self.z).speed(0.1));
                    });
                });
        }
    }
}
