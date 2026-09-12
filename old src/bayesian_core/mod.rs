use std::collections::HashMap;
use crate::nodes::Operation;
pub mod graph_checks;
mod model_compilation;
mod plate_validation;
mod inference;

pub use inference::{
    ControlledInferenceResult, InferenceResult, NodeInstanceSamples, PosteriorSample,
};
pub use model_compilation::CompiledGraph;

#[derive(Clone)]
/// Intermediate representation of the complete probabilistic graph and its plates.
pub struct GraphIR {
    pub nodes: HashMap<u32, NodeIR>,  // keyed by GraphNode id
    pub plates: HashMap<u32, PlateIR>,
}

impl GraphIR{
    /// Creates an empty graph intermediate representation with no nodes, edges, or plates.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::<u32, NodeIR>::new(),
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
/// Reference from a node parameter to the node that supplies its value.
pub struct ParamIR {
    pub from_node: u32, // param fed by node with this id
}

#[derive(Clone, Debug)]
/// Dataset-backed repeated scope containing its direct nodes and child plates.
pub struct PlateIR {
    pub id: u32,
    pub n: usize,
    pub nodes: Vec<u32>,
    pub plates: Vec<u32>,
    pub data: HashMap<String, Vec<f64>>,
    pub mapping: HashMap<u32, String>,
}
