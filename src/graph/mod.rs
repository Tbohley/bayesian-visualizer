pub mod edge;
pub mod load_preset;
pub mod plate;
pub mod reduced_view;
use std::collections::HashMap;

pub use edge::*;
pub use load_preset::*;
pub use plate::*;
pub use reduced_view::*;

use bevy::prelude::*;

//on links between nodes
#[derive(Component)]
pub struct GraphLink {
    pub from: Entity,
    pub to: Option<Entity>,
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

#[derive(Clone, Debug)]
pub struct Dataset {
    pub name: String,
    pub n: usize,
    pub data: HashMap<String, Vec<f64>>,
}

#[derive(Component, Debug)]
pub struct Plate {
    pub origin: Vec2,
    pub bounds: PlateBounds,
    pub data: Dataset,
    pub mapping: HashMap<Entity, String>, //maps observed nodes/scalar nodes to column names
}

#[derive(Component)]
pub struct PlateDraft;

#[derive(Component)]
pub struct PlateIndexHandle;

#[derive(Component)]
pub struct PlateIndexLabel;

#[derive(Component, Clone, Copy)]
pub(crate) enum PlateBorder {
    Top,
    Right,
    Bottom,
    Left,
}
