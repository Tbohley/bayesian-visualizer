#[cfg(test)]
use super::NodeIR;
#[cfg(test)]
use super::ParamIR;
use super::execution::{ExecutionPlan, Mode};
use super::graph_checks::PosteriorValues;
#[cfg(test)]
use super::graph_checks::{GraphModel, ModelResult};
use super::plate_validation::NormalizedPlates;
use super::{CompilationError, GraphIR, NodeShape};
use fugue::Model;
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(test)]
use std::sync::OnceLock;

/// Validated graph and immutable execution plans shared by every model run.
#[derive(Clone)]
pub struct CompiledGraph {
    graph: Arc<GraphIR>,
    shapes: Arc<HashMap<u32, NodeShape>>,
    normalized: Arc<NormalizedPlates>,
    #[cfg(test)]
    order: Arc<[u32]>,
    inference_plan: Arc<ExecutionPlan>,
    #[cfg(test)]
    predictive_plan: Arc<ExecutionPlan>,
    #[cfg(test)]
    full_plan: Arc<OnceLock<Result<Arc<ExecutionPlan>, String>>>,
}

impl CompiledGraph {
    pub(crate) fn new(graph: GraphIR) -> Result<Self, CompilationError> {
        if let Err(cycle) = graph.check_cycles() {
            return Err(CompilationError {
                node_ids: cycle.clone(),
                ..format!("graph contains a cycle including node IDs: {cycle:?}").into()
            });
        }
        let mut ids = graph.nodes.keys().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        for id in ids {
            let params = match &graph.nodes[&id] {
                super::NodeIR::Compute { params, .. } | super::NodeIR::Random { params, .. } => {
                    params.as_slice()
                }
                _ => &[],
            };
            for (index, param) in params.iter().enumerate() {
                if !graph.nodes.contains_key(&param.from_node) {
                    return Err(CompilationError {
                        parameter: Some(index),
                        ..CompilationError::at(
                            id,
                            format!("references missing node {}", param.from_node),
                        )
                    });
                }
            }
        }
        let order = graph.topological_sort()?;
        let normalized = graph.validated_plates()?;
        let shapes = super::type_checking::check_types(&graph, &normalized, &order)?;
        let inference_plan = Arc::new(ExecutionPlan::compile(
            &graph,
            &normalized,
            &order,
            Mode::Inference,
            false,
        )?);
        #[cfg(test)]
        let predictive_plan = Arc::new(ExecutionPlan::compile(
            &graph,
            &normalized,
            &order,
            Mode::Predictive,
            false,
        )?);
        Ok(Self {
            graph: Arc::new(graph),
            shapes: Arc::new(shapes),
            normalized: Arc::new(normalized),
            #[cfg(test)]
            order: order.into(),
            inference_plan,
            #[cfg(test)]
            predictive_plan,
            #[cfg(test)]
            full_plan: Arc::new(OnceLock::new()),
        })
    }
    pub fn node_shapes(&self) -> &HashMap<u32, NodeShape> {
        &self.shapes
    }
    pub fn graph(&self) -> &GraphIR {
        &self.graph
    }
    pub fn node_plate_path(&self, node_id: u32) -> Option<&[u32]> {
        self.normalized.node_paths.get(&node_id).map(Vec::as_slice)
    }
    /// Full-value reference execution for inference and plate-shape tests.
    #[cfg(test)]
    pub fn model(&self) -> Result<GraphModel, String> {
        let plan = self.full_plan.get_or_init(|| {
            ExecutionPlan::compile(
                &self.graph,
                &self.normalized,
                &self.order,
                Mode::Inference,
                true,
            )
            .map(Arc::new)
            .map_err(|error| error.to_string())
        });
        Ok(plan.as_ref().map_err(Clone::clone)?.model())
    }
    pub(crate) fn inference_model(&self) -> Model<Result<PosteriorValues, String>> {
        self.inference_plan.inference_model()
    }
    pub(crate) fn posterior_node_extents(&self) -> HashMap<u32, Vec<usize>> {
        self.inference_plan.latent_extents()
    }
    #[cfg(test)]
    pub(crate) fn predictive_model(&self) -> Result<GraphModel, String> {
        Ok(self.predictive_plan.model())
    }
}

impl GraphIR {
    /// Describes the optimized execution plan, including folded constants.
    #[cfg(test)]
    pub fn bind_debug_string(&self) -> Result<String, String> {
        let compiled = CompiledGraph::new(self.clone()).map_err(|error| error.to_string())?;
        compiled.bind_debug_string()
    }
}

impl CompiledGraph {
    /// Describes the optimized instructions actually used by inference.
    pub fn bind_debug_string(&self) -> Result<String, String> {
        Ok(self.inference_plan.debug_string())
    }
}

/// Builds the stable textual address for a node instance and its plate indices.
pub(super) fn instance_address(node_id: u32, plate_ids: &[u32], indices: &[usize]) -> String {
    let mut address = format!("node#{node_id}");
    for (&plate_id, &index) in plate_ids.iter().zip(indices) {
        address.push_str(&format!("/plate#{plate_id}[{index}]"));
    }
    address
}

#[cfg(test)]
#[path = "tests/model_compilation.rs"]
mod tests;
