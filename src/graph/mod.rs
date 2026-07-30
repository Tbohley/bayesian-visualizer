pub mod edge;
pub mod plate;
pub use edge::*;
pub use plate::*;

use bevy::prelude::*;

//on links between nodes
#[derive(Component)]    
pub struct GraphLink{
    pub from: Entity,
    pub to: Option<Entity>
}

//on unfinished (invisible) arrows
#[derive(Component)]
pub struct UnfinishedLink;

//on currently selected node
#[derive(Component)]
pub struct Selected;

#[derive(Clone, Copy, Debug)]
pub struct PlateBounds {
    pub min: Vec2,
    pub max: Vec2,
}

#[derive(Component, Debug)]
pub struct Plate {
    pub origin: Vec2,
    pub bounds: PlateBounds,
    pub n: usize
}

impl Plate{
    pub fn get_corner_pos(
        &self
    ) -> Vec3 {
        Vec3{
            x: (self.bounds.max.x - self.bounds.min.x) / 2. - 5.,
            y: -(self.bounds.max.y - self.bounds.min.y) / 2. + 5.,
            z: 1.0
        }
    }
}

#[derive(Component)]
pub struct PlateDraft;

#[derive(Component, Clone, Copy)]
pub(crate) enum PlateBorder {
    Top,
    Right,
    Bottom,
    Left,
}