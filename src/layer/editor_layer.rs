use legion::Resources;
use legion::systems::{CommandBuffer, Step};
use crate::Layer;

pub struct EditorLayer;

impl Layer for EditorLayer {
    fn init(self, cmd: &mut CommandBuffer, r: &mut Resources) {

        todo!()
    }

    fn startup_steps() -> Option<Vec<Step>> {
        None
    }

    fn run_steps() -> Option<Vec<Step>> {
        None
    }
}
