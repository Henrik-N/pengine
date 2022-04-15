use crate::mesh;
use crate::render_scene;
use penguin_util::handle::{Handle, HandleMap};

/// Individual, non-instanced draws for every object in the pass.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct RenderBatch {
    pub pass_object_h: Handle<PassObject>,
    /// Sort key/hash for mesh+material combination.
    pub sort_key: u64,
}

/// Covers a range in the flat_batches array. Maps directly to a DrawIndirect command - uses
/// uses instancing to draw a set of objects.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct IndirectBatch {
    pub mesh_h: Handle<mesh::Mesh>,
    pub pass_material: PassMaterial,
    /// First object on the render batch array
    pub first: u32,
    /// Number of objects in the render batch array (not used for anything currently)
    pub count: u32,
}

type Material = usize; // temp

#[derive(Clone, Copy, Default, Eq, PartialEq)]
// todo: Pass material
pub struct PassMaterial {
    material_h: Handle<Material>,
}

/// Reference to the related data of a RenderObject in a RenderScene.
#[derive(Copy, Clone)]
pub struct PassObject {
    pass_material: PassMaterial,
    mesh_h: Handle<mesh::Mesh>,
    /// The RenderObject this PassObject was created from.
    pub original_render_object: Handle<render_scene::RenderObject>,
    // ID to draw command in indirect_batches.
    pub draw_command_id: u32,
}

/// Information needed to render a pass of meshes for a part of the renderer.
/// Batches draws together into draw IndirectBatches that can be used to create draw commands.
pub struct MeshPass {
    /// Draw indirect batches.
    pub indirect_batches: Vec<IndirectBatch>,
    /// Pass objects sorted by mesh and material combination.
    pub sorted_render_batches: Vec<RenderBatch>,
    /// List of objects handled by this MeshPass.
    /// When the MeshPass updates, the RenderScene uses this array to build the draw commands / flat_batches base array.
    pub objects: HandleMap<PassObject>,
    /// Render objects pending addition
    pub unbatched_objects: Vec<Handle<render_scene::RenderObject>>,
}

impl MeshPass {
    pub fn new() -> Self {
        Self {
            indirect_batches: Vec::new(),
            sorted_render_batches: Vec::new(),
            objects: HandleMap::new(),
            unbatched_objects: Vec::new(),
        }
    }

    /// Updates the mesh pass
    pub fn update_batches(
        &mut self,
        render_objects: &HandleMap<render_scene::RenderObject>,
    ) -> bool {
        // only rebuild if there are new objects to add
        if self.unbatched_objects.is_empty() {
            return false;
        }

        // add new pass objects to the pass objects array and create new render batches from them
        //
        let new_render_batches: Vec<RenderBatch> = {
            self.objects.reserve(self.unbatched_objects.len());

            println!("MeshPass: adding render objects...");
            let mut index = 0;
            let new_render_batches = self
                .unbatched_objects
                .clone()
                .into_iter()
                .map(|render_obj_to_add| {
                    let render_object: &super::RenderObject = &render_objects[render_obj_to_add];

                    let pass_object = PassObject {
                        pass_material: PassMaterial::default(), // todo
                        mesh_h: render_object.mesh,
                        original_render_object: render_obj_to_add,
                        draw_command_id: 0,
                    };

                    let pass_object_h = self.objects.push(pass_object);

                    let sort_key = (pass_object.mesh_h.id as u64)
                        | ((pass_object.pass_material.material_h.id as u64) << 32);
                    println!("RenderObject {}: sort_key = {}", index, sort_key);

                    index += 1;

                    RenderBatch {
                        pass_object_h,
                        sort_key,
                    }
                })
                .collect::<Vec<_>>();

            self.unbatched_objects.clear();

            println!("\n");

            new_render_batches
        };

        // add new render batches to the render batches array and sort it by mesh and material
        //
        let render_batches: &Vec<RenderBatch> = {
            self.sorted_render_batches.extend(new_render_batches);
            self.sorted_render_batches
                .sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
            &self.sorted_render_batches
        };

        // group render batches with the same mesh and material into instanced indirect draw commands
        //
        let indirect_batches: Vec<IndirectBatch> = {
            let first_pass_object: PassObject = self.objects[render_batches[0].pass_object_h];

            let mut indirect_batches = Vec::new();

            self.objects[render_batches[0].pass_object_h].draw_command_id =
                indirect_batches.len() as _;

            indirect_batches.push(IndirectBatch {
                mesh_h: first_pass_object.mesh_h,
                pass_material: first_pass_object.pass_material,
                first: 0,
                count: 0,
            });

            debug_assert_eq!(render_batches.len(), self.objects.len());

            for (index, &render_batch) in render_batches.iter().enumerate() {
                let pass_object = self.objects[render_batch.pass_object_h];

                // get mesh and material for this pass object
                let mesh_h = pass_object.mesh_h;
                let material = pass_object.pass_material;

                let mut previous: &mut IndirectBatch = indirect_batches.last_mut().unwrap();

                let same_mesh_as_previous = mesh_h.id == previous.mesh_h.id;
                let same_material_as_previous = material == previous.pass_material;

                if same_mesh_as_previous && same_material_as_previous {
                    // if the batch can be instanced, just increase the max instance count
                    // (this count isn't used for anything currently, just storing it in case
                    // I need it for something later)
                    previous.count += 1;
                } else {
                    // otherwise, create a new draw command
                    indirect_batches.push(IndirectBatch {
                        mesh_h,
                        pass_material: material,
                        first: index as _,
                        count: 1,
                    });
                }

                // associate the draw command with the correct pass object
                self.objects[render_batch.pass_object_h].draw_command_id =
                    (indirect_batches.len() - 1) as _;
            }

            indirect_batches
        };

        self.indirect_batches = indirect_batches;

        return true;
    }
}
