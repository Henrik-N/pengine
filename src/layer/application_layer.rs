use crate::{GraphicsContext, Layer};
use legion::system;
use legion::systems::{CommandBuffer, Step};
use legion::{Resources, Schedule};

pub struct Time {
    clock: crate::time::Clock,
}

pub struct ApplicationLayer;

impl Layer for ApplicationLayer {
    fn init(self, _cmd: &mut CommandBuffer, r: &mut Resources) {
        log::warn!("INIT APPLICATION LAYER ----------------");

        r.insert(Time::default());
    }

    fn startup_steps() -> Option<Vec<Step>> {
        None
    }

    fn run_steps() -> Option<Vec<Step>> {
        Some(
            Schedule::builder()
                .add_system(update_delta_time_system())
                .build()
                .into_vec(),
        )
    }
}

#[system]
fn update_delta_time(#[resource] dt: &mut Time) {
    dt.clock.tick();
}

penguin_util::impl_default!(
    Time,
    Self {
        clock: crate::time::Clock::start(),
    }
);
impl Time {
    pub fn delta_time(&self) -> std::time::Duration {
        self.clock.last_delta_time
    }

    pub fn start_time(&self) -> std::time::Instant {
        self.clock.start_time
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.clock.start_time.elapsed()
    }

    pub fn elapsed_f32(&self) -> f32 {
        self.elapsed().as_secs_f32()
    }

    pub fn elapsed_f64(&self) -> f64 {
        self.elapsed().as_secs_f64()
    }

    pub fn dt_f32(&self) -> f32 {
        self.delta_time().as_secs_f32()
    }

    pub fn dt_f64(&self) -> f64 {
        self.delta_time().as_secs_f64()
    }
}
