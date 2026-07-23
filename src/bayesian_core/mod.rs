use std::collections::HashMap;
use crate::nodes::Operation;
pub mod graph_checks;
use fugue::*;

#[derive(Clone)]
pub struct GraphIR {
    pub nodes: HashMap<u32, NodeIR>,  // keyed by GraphNode id
    pub edges: Vec<EdgeIR>,
    pub plates: HashMap<u32, PlateIR>,
}

impl GraphIR{
    pub fn new() -> Self {
        Self {
            nodes: HashMap::<u32, NodeIR>::new(),
            edges: Vec::<EdgeIR>::new(),
            plates: HashMap::<u32, PlateIR>::new(),
        }
    }
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub struct ParamIR {
    pub from_node: u32,            // param fed by node with this id
    pub param_name: Option<String>
}

#[derive(Clone)]
pub struct EdgeIR {
    pub from: u32,
    pub to: u32,
}

#[derive(Clone, Debug)]
pub struct PlateIR {
    pub id: u32,
    /// Nodes directly contained by this plate.
    pub nodes: Vec<u32>,
    /// Plates directly nested inside this plate.
    pub plates: Vec<u32>,
}
