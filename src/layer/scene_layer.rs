use crate::{m, Layer};
use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use legion::systems::{CommandBuffer, Step};
use legion::world::SubWorld;
use legion::{component, system, Entity, Query, Resources, Schedule};
use std::collections::HashMap;

// contains mesh index (todo: temp)
use crate::components::*;
use crate::layer::application_layer::Time;
use crate::layer::scene_layer::WriteState::A;

pub struct MeshAssets(Vec<&'static str>);
penguin_util::impl_deref!(MeshAssets, Vec<&'static str>);

pub struct SceneEntityHandles(Vec<Entity>);

enum WriteState {
    A,
    B,
}
impl WriteState {
    fn swap(&mut self) {
        *self = match self {
            WriteState::A => WriteState::B,
            WriteState::B => WriteState::A,
        }
    }
}

use legion::systems::Resource;

pub struct Events<E: Resource> {
    events: Vec<E>,
}

// #[system]
// fn testy_sys(world: &mut SubWorld, query: &mut Query<&Translation>) {
//
// }

pub struct Events2<EventType: legion::systems::Resource> {
    events_a: AtomicRefCell<Vec<EventType>>,
    events_b: AtomicRefCell<Vec<EventType>>,
    write_state: WriteState,
}
impl<T: legion::systems::Resource> Events2<T> {
    fn new() -> Self {
        Self {
            events_a: AtomicRefCell::new(Vec::new()),
            events_b: AtomicRefCell::new(Vec::new()),
            write_state: WriteState::A,
        }
    }
}

#[derive(Default)]
pub struct EventWrites<E: legion::systems::Resource> {
    data: AtomicRefCell<Vec<E>>,
}

#[derive(Default)]
pub struct EventReads<E: legion::systems::Resource> {
    data: AtomicRefCell<Vec<E>>,
    read_count: usize,
}

fn register_event_type<E: legion::systems::Resource>(r: &mut Resources) {
    r.insert(EventWrites::<E> {
        data: AtomicRefCell::new(Vec::new()),
    });
    r.insert(EventReads::<E> {
        data: AtomicRefCell::new(Vec::new()),
        read_count: 0,
    });
}

#[system]
fn events_update(
    #[resource] reads: &mut EventReads<SomeEvent>,
    #[resource] writes: &mut EventWrites<SomeEvent>,
) {
    let reads = reads.data.get_mut();

    // reads.into_iter().rev().take(read)

    reads.extend(writes.data.get_mut().drain(..));

    // reads.extend(writes.clone().into_iter());
}

struct SomeEvent {
    some_message: String,
}

pub struct SceneLayer;
impl Layer for SceneLayer {
    fn init(self, cmd: &mut CommandBuffer, r: &mut Resources) {
        let mesh_assets = MeshAssets(vec!["cube.obj", "cone.obj"]);

        let a = cmd.push((
            Name::from("Cube"),
            MeshComponent(0),
            Translation(m::vec3(2., 1., 2.)),
            Rotation::default(),
        ));
        let b = cmd.push((
            Name::from("Cone"),
            MeshComponent(1),
            Translation(m::vec3(0., 4., 0.)),
        ));

        let entity_handles = SceneEntityHandles(vec![a, b]);

        r.insert(mesh_assets);
        r.insert(entity_handles);

        register_event_type::<SomeEvent>(r);
    }

    fn startup_steps() -> Option<Vec<Step>> {
        None
    }

    fn run_steps() -> Option<Vec<Step>> {
        Some(
            Schedule::builder()
                .add_system(update_system())
                .add_system(update2_system())
                .build()
                .into_vec(),
        )
    }
}

#[system(for_each)]
#[filter(!component::<Rotation>())]
fn update(translation: &mut Translation, #[resource] time: &Time) {
    let (x, y) = (time.elapsed_f32().cos() * 2., time.elapsed_f32().sin() * 2.);

    translation.0 = m::vec3(x, y, 0.);
}

#[system(for_each)]
fn update2(translation: &mut Translation, rotation: &mut Rotation, #[resource] time: &Time) {
    let (x, y) = (time.elapsed_f32().cos() * 3., time.elapsed_f32().sin() * 3.);

    translation.0 = m::vec3(x, y, 0.);

    rotation.0 = m::Quat::from_axis_angle(m::Vec3::Z, x);
}
