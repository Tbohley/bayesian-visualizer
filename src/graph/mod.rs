pub mod edge;
pub use edge::*;

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

