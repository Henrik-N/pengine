///! Systems to update cpu-side render objects data and mark the updated data as "should reupload to gpu memory".
// todo: Separate model matrices from the render objects.
use super::*;
use crate::components::{Rotation, Scale, Translation};
use legion::component;
use legion::maybe_changed;

pub fn steps() -> Vec<Step> {
    Schedule::builder()
        .add_system(translation_system())
        .add_system(translation_rotation_system())
        .add_system(translation_rotation_scale_system())
        .build()
        .into_vec()
}

#[system(for_each)]
#[filter(
    maybe_changed::<Translation>()
    & !component::<Rotation>()
    & !component::<Scale >()
)]
fn translation(
    render_obj: &Handle<RenderObject>,
    translation: &Translation,
    #[resource] render_objs: &mut RenderObjects,
) {
    render_objs.enqueue_model_matrix_update(*render_obj, m::Mat4::from_translation(translation.0));
}

#[system(for_each)]
#[filter(
    maybe_changed::<Translation>()
    | maybe_changed::<Rotation>()
    & !component::<Scale>()
)]
fn translation_rotation(
    render_obj: &Handle<RenderObject>,
    translation: &Translation,
    rotation: &Rotation,
    #[resource] render_objs: &mut RenderObjects,
) {
    render_objs.enqueue_model_matrix_update(
        *render_obj,
        m::Mat4::from_rotation_translation(rotation.0, translation.0),
    );
}

#[system(for_each)]
#[filter(
    maybe_changed::<Translation>()
    | maybe_changed::<Rotation>()
    | maybe_changed::<Scale>()
)]
fn translation_rotation_scale(
    render_obj: &Handle<RenderObject>,
    translation: &Translation,
    rotation: &Rotation,
    scale: &Scale,
    #[resource] render_objs: &mut RenderObjects,
) {
    render_objs.enqueue_model_matrix_update(
        *render_obj,
        m::Mat4::from_scale_rotation_translation(scale.0, rotation.0, translation.0),
    );
}
