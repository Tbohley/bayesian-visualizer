use super::*;
use fugue::Model;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// The values produced by one execution of a compiled graph model, keyed by
/// [`GraphNode`](crate::nodes::GraphNode) ID.
pub type ModelValues = HashMap<u32, ModelResult>;

/// Model execution can fail after an upstream random value has been sampled.
/// For example, that value may be used as an invalid standard deviation by a
/// downstream distribution. Keeping the error in the model's result lets us
/// build the complete hierarchy without panicking inside a `bind` closure.
pub type GraphModel = Model<Result<ModelValues, String>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}

#[derive(Clone, PartialEq, Debug)]
pub enum ModelResult {
    Scalar(f64),
    Plate(Vec<ModelResult>),
}

impl GraphIR {
    /// Validates this graph once and retains its reusable execution metadata.
    pub fn compile(self) -> Result<super::CompiledGraph, String> {
        super::model_compilation::CompiledGraph::new(self)
    }

    /// Detects dependency cycles among graph nodes using depth-first search.
    pub fn check_cycles(&self) -> Result<(), Vec<u32>> {
        /// Returns the parameter list for nodes that consume upstream values.
        fn params(node: &NodeIR) -> &[ParamIR] {
            match node {
                NodeIR::Random { params, .. } => params,
                NodeIR::Compute { params, .. } => params,
                NodeIR::Scalar { .. } => &[],
            }
        }

        /// Traverses one node's dependencies and reports the cycle currently on the DFS stack.
        fn visit(
            node_id: u32,
            graph: &GraphIR,
            states: &mut HashMap<u32, VisitState>,
            stack: &mut Vec<u32>,
        ) -> Result<(), Vec<u32>> {
            if states.get(&node_id) == Some(&VisitState::Visiting) {
                let cycle_start = stack
                    .iter()
                    .position(|id| *id == node_id)
                    .expect("visiting node should be in DFS stack");

                return Err(stack[cycle_start..].to_vec());
            }

            if states.get(&node_id) == Some(&VisitState::Visited) {
                return Ok(());
            }

            states.insert(node_id, VisitState::Visiting);
            stack.push(node_id);

            if let Some(node) = graph.nodes.get(&node_id) {
                for param in params(node) {
                    visit(param.from_node, graph, states, stack)?;
                }
            }

            stack.pop();
            states.insert(node_id, VisitState::Visited);

            Ok(())
        }

        let mut states = HashMap::new();
        let mut stack = Vec::new();

        for &node_id in self.nodes.keys() {
            visit(node_id, self, &mut states, &mut stack)?;
        }

        Ok(())
    }

    /// Produces a stable dependency-respecting order for all graph nodes.
    pub fn topological_sort(&self) -> Result<Vec<u32>, String> {
        let mut indegrees = HashMap::new();
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();

        for (&id, node) in &self.nodes {
            let params: &[ParamIR] = match node {
                NodeIR::Random { params, .. } | NodeIR::Compute { params, .. } => params.as_slice(),
                NodeIR::Scalar { .. } => &[],
            };
            indegrees.insert(id, params.len());
            for param in params {
                if !self.nodes.contains_key(&param.from_node) {
                    return Err(format!(
                        "node {id} references missing node {}",
                        param.from_node
                    ));
                }
                children.entry(param.from_node).or_default().push(id);
            }
        }
        let mut ready: BinaryHeap<Reverse<u32>> = indegrees
            .iter()
            .filter_map(|(&id, &degree)| (degree == 0).then_some(Reverse(id)))
            .collect();

        let mut order = Vec::with_capacity(self.nodes.len());

        while let Some(Reverse(id)) = ready.pop() {
            order.push(id);

            for &child in children.get(&id).into_iter().flatten() {
                let degree = indegrees
                    .get_mut(&child)
                    .expect("child should have an indegree");

                *degree -= 1;

                if *degree == 0 {
                    ready.push(Reverse(child));
                }
            }
        }
        Ok(order)
    }

    /// Samples every node in dependency order from the graph's prior model.
    pub fn ancestral_sample(&self) -> Result<HashMap<u32, ModelResult>, String> {
        let model = self.create_model()?;
        let mut rng = rand::thread_rng();
        let (result, _trace) = fugue::runtime::handler::run(
            fugue::runtime::interpreters::PriorHandler {
                rng: &mut rng,
                trace: fugue::runtime::trace::Trace::default(),
            },
            model,
        );
        result
    }

    /// Compiles this graph into a hierarchical Fugue model after validating its structure.
    /// Compilation errors are returned immediately, while value-dependent errors are retained in the model result.
    pub fn create_model(&self) -> Result<GraphModel, String> {
        super::model_compilation::create_model(self)
    }
}
