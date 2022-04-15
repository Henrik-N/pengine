use std::ops::DerefMut;
use crate::input;
use winit::event_loop::EventLoopProxy;

#[derive(Debug)]
pub enum PenguinEvent {
    Input(input::InputEvent),
    Window(event::WindowResizeEvent),
}

pub struct PenguinEventProxy(pub std::sync::Mutex<winit::event_loop::EventLoopProxy<PenguinEvent>>);

impl From<winit::event_loop::EventLoopProxy<PenguinEvent>> for PenguinEventProxy {
    fn from(event_loop_proxy: EventLoopProxy<PenguinEvent>) -> Self {
        Self(std::sync::Mutex::new(event_loop_proxy))
    }
}

impl PenguinEventProxy {
    pub fn send_event(&self, event: PenguinEvent) {
        self.0.lock().unwrap().send_event(event).ok();
    }
}

pub struct PenguinEventSender(pub std::sync::Arc<PenguinEventProxy>);

impl std::clone::Clone for PenguinEventSender {
    fn clone(&self) -> Self {
        Self(std::sync::Arc::clone(&self.0))
    }
}

impl std::ops::Deref for PenguinEventSender {
    type Target = std::sync::Arc<PenguinEventProxy>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PenguinEventSender {
    pub fn init(event_loop_proxy: winit::event_loop::EventLoopProxy<PenguinEvent>) -> Self {
        Self(std::sync::Arc::new(PenguinEventProxy::from(
            event_loop_proxy,
        )))
    }
}

pub mod event {
    /// Event fired when the window is resized, or if the window's scale factor changes.
    #[derive(Debug)]
    pub struct WindowResizeEvent {
        pub size: winit::dpi::PhysicalSize<u32>,
        pub scale_factor: Option<f64>,
    }

    pub use crate::input::InputEvent;
}

// new events --------------

// Unique identifier for an event
// todo: Just a wrapper used to debug the name and type, right?
// #[derive(Eq, PartialEq, Ord, PartialOrd, Hash, Copy, Clone)]
// pub struct EventId<EventType> {
//     pub id: usize,
//     _marker: std::marker::PhantomData<EventType>,
// }

pub struct EventId(pub usize);

pub struct Event<EventType> {
    pub event_id: EventId,
    pub event: EventType,
}

enum State {
    A,
    B,
}


pub struct EventWrites<EventType> {
    writes: Vec<Event<EventType>>,

}




/// Resource containing events of type T
pub struct Events<EventType> {
    events_a: Vec<Event<EventType>>,
    events_b: Vec<Event<EventType>>,
    a_start_event_count: usize,
    b_start_event_count: usize,
    event_count: usize,
    state: State,
}
impl<T> Default for Events<T> {
    fn default() -> Self {
        Self {
            events_a: Vec::new(),
            events_b: Vec::new(),
            a_start_event_count: 0,
            b_start_event_count: 0,
            event_count: 0,
            state: State::A,
        }
    }
}
impl<T> Events<T> {
    pub fn send(&mut self, event: T) {
        let event_id = EventId(self.event_count);

        let event_instance = Event { event_id: event_id, event, };

        match self.state {
            State::A => self.events_a.push(event_instance),
            State::B => self.events_b.push(event_instance),
        }

        self.event_count += 1;
    }
}

// impl AtomicRefCell<>

// impl<T> DerefMut for Events<T> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//
//         todo!()
//     }
// }


// pub struct EventWriter<'a, EventType: ?Sized + 'a> {
//     events: atomic_refcell::AtomicRefCell<EventType>,
// }


// pub struct EventWriter<EventType> {
//     events: atomic_refcell::AtomicRefMut<Events<'a, EventType>>,
//     // &'static mut Events<EventType>
// }


// pub struct EventWriter<'a, EventType, EventsWriter: Events<EventType>> {
//
// }



// pub struct EventWriter<EventType: legion::systems::Resource> {
//     events: Events<EventType>,
// }





#[test]
fn test_events() {

}


