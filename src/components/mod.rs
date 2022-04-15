mod component_editors;

use egui::Ui;
use macaw as m;

/// The name of an entity.
pub struct Name(pub String);
impl From<&str> for Name {
    fn from(str: &str) -> Self {
        Self(str.to_owned())
    }
}
impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub use transform::*;
mod transform {
    use super::*;
    use penguin_util::{impl_default, impl_deref};

    /// Translation component
    #[derive(Debug, PartialEq, Default, Clone)]
    pub struct Translation(pub m::Vec3);
    impl_deref!(mut Translation, m::Vec3);

    /// Rotation component
    #[derive(Debug, PartialEq, Default, Clone)]
    pub struct Rotation(pub m::Quat);
    impl_deref!(mut Rotation, m::Quat);

    /// Scale component
    #[derive(Debug, PartialEq, Clone)]
    pub struct Scale(pub m::Vec3);
    impl_deref!(mut Scale, m::Vec3);
    impl_default!(Scale, Self(m::Vec3::ONE));
}

type MeshAssetIndex = usize;
pub struct MeshComponent(pub MeshAssetIndex);
