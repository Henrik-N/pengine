use legion::EntityStore;

mod leg {
    pub use legion::storage::*;
    pub use legion::world::*;
}

/// Trait to implement for components that should be editable in the editor.
pub trait ComponentEditor
where
    Self: leg::Component,
{
    type ComponentEditorState: Sized + Default;

    fn init_component_editor_state(&self) -> Self::ComponentEditorState;
    fn penguin_editor(&mut self, ui: &mut egui::Ui, state: &mut Self::ComponentEditorState);

    fn try_draw_editor_yeet(
        entry: &mut legion::world::EntryMut,
        ui: &mut egui::Ui,
        component_editor_state_storage: &std::cell::RefCell<ComponentEditorStateStorage>,
    ) -> bool {
        let component_type_id = leg::ComponentTypeId::of::<Self>();

        // access the component
        let component: &mut Self = entry.get_component_mut::<Self>().unwrap();

        // // get the editor state for this component editor state
        let mut state_storage = component_editor_state_storage.borrow_mut();

        let state = match state_storage.0.get_mut(&component_type_id) {
            Some(s) => s,
            None => {
                let state = component.init_component_editor_state();

                let state = ComponentEditorState(std::boxed::Box::new(state));

                state_storage.0.insert(component_type_id, state);
                state_storage.0.get_mut(&component_type_id).unwrap()
            }
        };

        let mut actual_state = state
            .0
            .downcast_mut::<Self::ComponentEditorState>()
            .unwrap();

        component.penguin_editor(ui, &mut actual_state);

        true
    }
}

use draw_function::*;
mod draw_function {
    use super::*;

    /// Function that draws the editor ui for a component.
    pub(super) struct DrawComponentEditorFunc {
        component_type_id: leg::ComponentTypeId,
        pub draw_func: fn(
            &mut legion::world::EntryMut,
            &mut egui::Ui,
            &std::cell::RefCell<ComponentEditorStateStorage>,
        ),
    }

    impl DrawComponentEditorFunc {
        pub fn new<ComponentType: ComponentEditor>() -> Self {
            Self {
                component_type_id: leg::ComponentTypeId::of::<ComponentType>(),
                draw_func: Self::draw_editor::<ComponentType>,
            }
        }

        /// Checks if this draw function is the draw function for the given component
        pub fn is_for_component(&self, component_type_id: leg::ComponentTypeId) -> bool {
            component_type_id.eq(&self.component_type_id)
        }

        fn draw_editor<T>(
            entry: &mut legion::world::EntryMut,
            ui: &mut egui::Ui,
            component_editor_state_storage: &std::cell::RefCell<ComponentEditorStateStorage>,
        ) where
            T: legion::storage::Component + ComponentEditor,
        {
            let component_type_id = leg::ComponentTypeId::of::<T>();

            // access the component
            let component: &mut T = entry.get_component_mut::<T>().unwrap();

            // // get the editor state for this component editor state
            let mut state_storage = component_editor_state_storage.borrow_mut();

            let state: &mut ComponentEditorState = match state_storage.0.get_mut(&component_type_id)
            {
                Some(s) => s,
                None => {
                    let state = component.init_component_editor_state();

                    let state = ComponentEditorState(std::boxed::Box::new(state));

                    state_storage.0.insert(component_type_id, state);
                    state_storage.0.get_mut(&component_type_id).unwrap()
                }
            };

            let mut actual_state = state.0.downcast_mut::<T::ComponentEditorState>().unwrap();

            component.penguin_editor(ui, &mut actual_state);
        }
    }
}

pub use component_editor_state::*;
mod component_editor_state {
    use super::*;

    /// Holds the state for a component's editor-only data. // , when the component is visible in the UI.
    pub struct ComponentEditorState(pub Box<dyn std::any::Any>);

    /// Contains the state for components currently visible in the editor, such as the components of the currently selected entity.
    #[derive(Default)]
    pub struct ComponentEditorStateStorage(
        pub std::collections::HashMap<leg::ComponentTypeId, ComponentEditorState>,
    );

    /// Stores the functions needed to draw the ui of components, as well as state for active component in the editor.
    #[derive(Default)]
    pub struct EditorComponentStorage {
        // Functions that draw the editor for a component UI
        draw_funcs: Vec<DrawComponentEditorFunc>,
        /// The currently selected entity
        ui_states: std::cell::RefCell<ComponentEditorStateStorage>,
        selected_entity: std::cell::Cell<Option<legion::Entity>>,
    }

    // testing the new version
    impl EditorComponentStorage {
        pub fn register_component_editor<ComponentType>(&mut self)
        where
            ComponentType: ComponentEditor,
        {
            self.draw_funcs
                .push(DrawComponentEditorFunc::new::<ComponentType>());
        }

        pub fn select_entity(&self, entity: legion::Entity) {
            if let Some(selected_entity) = self.selected_entity.get() {
                // if still the same selected entity, don't change anything
                if selected_entity == entity {
                    return;
                }

                // clear the previous entity's components' states
                self.ui_states.borrow_mut().0.clear();

                // set the new entity
                self.selected_entity.set(Some(entity));
            }
        }

        /// Draw the components of a given entity in the UI, and provides mutable access to these components
        pub fn draw_entities_component_editors(
            &self,
            world: &mut legion::World,
            entity: legion::Entity,
            ui: &mut egui::Ui,
        ) {
            if let Ok(mut e) = world.entry_mut(entity) {
                // get around the borrow checker
                let component_type_ids = e
                    .archetype()
                    .layout()
                    .component_types()
                    .into_iter()
                    .map(|ty| *ty)
                    .collect::<Vec<_>>();

                for component_type_id in component_type_ids {
                    // find the draw function for this component (if any) and execute it
                    self.draw_funcs.iter().find(|draw_func| {
                        if draw_func.is_for_component(component_type_id) {
                            let func = draw_func.draw_func;
                            func(&mut e, ui, &self.ui_states);

                            true
                        } else {
                            false
                        }
                    });
                }
            }
        }
    }
}
