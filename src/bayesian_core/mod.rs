use std::collections::HashMap;
use crate::nodes::Operation;
pub mod graph_checks;
use fugue::*;

pub struct GraphIR {
    pub nodes: HashMap<u32, NodeIR>,  // keyed by GraphNode id
    pub edges: Vec<EdgeIR>,
}

impl GraphIR{
    pub fn new() -> Self {
        Self { nodes: HashMap::<u32, NodeIR>::new(), edges: Vec::<EdgeIR>::new() }
    }
}

pub enum NodeIR {
    Random {
        id: u32,
        label: Option<String>,
        dist_type: String,
        params: Vec<ParamIR>,
    },
    Scalar {
        id: u32,
        value: f64
    },
    Compute {
        id: u32,
        operation: Operation,
        params: Vec<ParamIR>
    },
}

pub struct ParamIR {
    pub from_node: u32,            // param fed by node with this id
    pub param_name: Option<String>
}

pub struct EdgeIR {
    pub from: u32,
    pub to: u32,
}