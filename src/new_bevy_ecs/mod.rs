use crate::new_bevy_ecs::time::TimePlugin;
use bevy_app::{App, PluginGroup, PluginGroupBuilder};
use bevy_log::{Level, LogSettings};

mod graphics_context;
mod meshes;

pub fn new_main() {
    App::new()
        .insert_resource(LogSettings {
            filter: "wgpu=warn".to_owned(),
            level: Level::DEBUG,
        })
        .add_plugins(core::CorePlugins)
        .add_plugin(time::TimePlugin)
        .add_plugin(graphics_context::GraphicsContextPlugin)
        .run();
}

mod time {
    use bevy_app::{App, Plugin};
    use bevy_ecs::prelude::*;
    use bevy_window::RequestRedraw;

    pub struct Time {
        clock: crate::time::Clock,
    }

    pub struct TimePlugin;
    impl Plugin for TimePlugin {
        fn build(&self, app: &mut App) {
            app.add_startup_system(startup.system())
                .add_system(update_delta_time.system());
        }
    }

    fn startup(mut cmd: Commands) {
        cmd.insert_resource(Time::default());
    }

    fn update_delta_time(e: EventReader<RequestRedraw>, mut time: ResMut<Time>) {
        if !e.is_empty() {
            time.clock.tick();
        }
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
}

mod core {
    pub struct CorePlugins;

    use bevy_app::{PluginGroup, PluginGroupBuilder};
    use bevy_input::InputPlugin;
    use bevy_log::LogPlugin;
    use bevy_window::WindowPlugin;
    use bevy_winit::WinitPlugin;

    impl PluginGroup for CorePlugins {
        fn build(&mut self, group: &mut PluginGroupBuilder) {
            group
                .add(LogPlugin)
                .add(InputPlugin::default())
                .add(WindowPlugin::default())
                .add(WinitPlugin);
        }
    }
}
