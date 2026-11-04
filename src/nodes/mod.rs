//! Shared node components and public construction/editing entry points.
mod operation;
pub use operation::Operation;
mod editing;
pub use editing::{autofill_next_param, set_node_name, update_node_observation_colors};
mod interaction;
pub use interaction::{on_background_click, on_node_click, open_node_inspection};

pub mod compute_node;
pub mod random_node;
pub mod scalar_node;
pub mod vector_node;
use bevy::prelude::*;
pub use compute_node::*;
pub use random_node::*;
pub use scalar_node::*;
pub use vector_node::*;

//on all node entities
#[derive(Component)]
pub struct GraphNode(pub u32);

/// Select the latent node whose automatic posterior histogram is being opened.
#[derive(Event)]
pub struct OpenNodeInspection {
    pub node_id: u32,
}

#[derive(Component)]
pub struct NodeLabel;

#[derive(Component)]
pub struct NodeInterior;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeType {
    Random,
    Compute,
    Scalar,
    Vector,
}

#[derive(Component)]
pub struct NodeMode(pub NodeType);

#[derive(Debug, Clone)]
pub struct ParamValue(pub &'static str, pub Option<Entity>);

#[derive(Event)]
pub struct AutofillNextParam {
    pub node: Entity,
    pub linked_node: Entity,
}

#[derive(Event)]
pub struct SetNodeName {
    pub entity: Entity,
    pub name: String,
}

pub trait NodeDisplay {
    fn label(&self) -> String;
}

#[derive(Component)]
pub struct SelectedIndicator;

#[derive(Component)]
pub struct ObservedNode;
