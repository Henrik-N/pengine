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
