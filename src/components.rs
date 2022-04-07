use egui::Ui;
use hecs::Entity;
use macaw as m;

/// Draws the ui editor UI for an entity and it's components.
pub fn penguin_entity_ui(entity: hecs::EntityRef<'_>, ui: &mut egui::Ui) {
    fn ui_draw<T>(entity: hecs::EntityRef<'_>, ui: &mut egui::Ui) -> Option<()>
    where
        T: hecs::Component + PenguinComponent,
    {
        let mut component: hecs::RefMut<T> = entity.get_mut::<T>()?;

        component.penguin_editor(ui);

        Some(())
    }

    const DRAW_UI_FUNCS: &[&dyn Fn(hecs::EntityRef<'_>, &mut egui::Ui) -> Option<()>] =
        &[&ui_draw::<EntityName>, &ui_draw::<Transform>];

    for func in DRAW_UI_FUNCS {
        let _ = func(entity, ui);
    }
}

pub trait PenguinComponent {
    fn penguin_editor(&mut self, ui: &mut egui::Ui);
}

/// Name of an entity in the editor
pub struct EntityName(pub String);
impl From<&str> for EntityName {
    fn from(str: &str) -> Self {
        Self(str.to_owned())
    }
}
impl PenguinComponent for EntityName {
    fn penguin_editor(&mut self, ui: &mut Ui) {
        ui.separator();
        ui.text_edit_singleline(&mut self.0);
    }
}

#[derive(PartialEq, Clone, Debug)]
/// Transform component.
pub struct Transform {
    pub translation: m::Vec3,
    pub rotation: m::Vec3, // todo quaternion rotations
    pub scale: m::Vec3,
    /// Weather this transform needs to be reused to update a model matrix in gpu memory.
    pub is_dirty: bool,
}
impl std::fmt::Display for Transform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::default::Default for Transform {
    fn default() -> Self {
        Self {
            translation: m::Vec3::ZERO,
            rotation: m::Vec3::ZERO,
            scale: m::Vec3::ONE,
            is_dirty: false,
        }
    }
}

impl Transform {
    pub fn model_matrix(&mut self) -> m::Mat4 {
        let rot = m::Quat::from_euler(
            m::EulerRot::XYZ,
            self.rotation.x,
            self.rotation.y,
            self.rotation.z,
        );

        m::Mat4::from_scale_rotation_translation(self.scale, rot, self.translation)
    }
}
impl PenguinComponent for Transform {
    fn penguin_editor(&mut self, ui: &mut egui::Ui) {
        let previous = self.clone();

        use std::f32::consts::PI;

        egui::CollapsingHeader::new("Transform")
            .default_open(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Translation").small());
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::DragValue::new(&mut self.translation.x).speed(0.1));
                    ui.add(egui::DragValue::new(&mut self.translation.y).speed(0.1));
                    ui.add(egui::DragValue::new(&mut self.translation.z).speed(0.1));
                });

                ui.label(egui::RichText::new("Rotation").small());
                ui.horizontal_wrapped(|ui| {
                    ui.drag_angle_tau(&mut self.rotation.x);
                    ui.drag_angle_tau(&mut self.rotation.y);
                    ui.drag_angle_tau(&mut self.rotation.z);
                });

                ui.label(egui::RichText::new("Scale").small());
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::DragValue::new(&mut self.scale.x).speed(0.1));
                    ui.add(egui::DragValue::new(&mut self.scale.y).speed(0.1));
                    ui.add(egui::DragValue::new(&mut self.scale.z).speed(0.1));
                });
            });

        if previous != *self {
            self.is_dirty = true;
        }
    }
}
