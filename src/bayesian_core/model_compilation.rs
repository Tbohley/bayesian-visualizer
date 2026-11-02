use super::{GraphIR, CompilationError, NodeShape};
#[cfg(test)]
use super::NodeIR;
#[cfg(test)]
use super::ParamIR;
use super::execution::{ExecutionPlan, Mode};
use super::graph_checks::PosteriorValues;
#[cfg(test)]
use super::graph_checks::{GraphModel, ModelResult};
use super::plate_validation::NormalizedPlates;
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
            return Err(CompilationError { node_ids: cycle.clone(), ..format!("graph contains a cycle including node IDs: {cycle:?}").into() });
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
mod tests {
    use super::*;
    use crate::bayesian_core::PlateIR;

    fn empty_plate(id: u32, n: usize, nodes: Vec<u32>) -> PlateIR {
        PlateIR {
            id,
            n,
            nodes,
            plates: vec![],
            data: HashMap::new(),
            mapping: HashMap::new(),
        }
    }

    #[test]
    /// Verifies mapped data columns observe random nodes and override scalar literals.
    fn linked_column_observes_random_nodes_and_replaces_scalar_literals() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, NodeIR::Scalar { value: 0.0 });
        graph.nodes.insert(2, NodeIR::Scalar { value: 1.0 });
        graph.nodes.insert(
            3,
            NodeIR::Random {
                dist_type: "Normal".to_string(),
                params: vec![ParamIR { from_node: 1 }, ParamIR { from_node: 2 }],
            },
        );
        graph.nodes.insert(4, NodeIR::Scalar { value: 999.0 });
        graph.plates.insert(
            10,
            PlateIR {
                id: 10,
                n: 2,
                nodes: vec![3, 4],
                plates: Vec::new(),
                data: HashMap::from([("x".to_string(), vec![1.25, -0.5])]),
                mapping: HashMap::from([(3, "x".to_string()), (4, "x".to_string())]),
            },
        );

        let values = graph.ancestral_sample().unwrap();
        let expected =
            ModelResult::Plate(vec![ModelResult::Scalar(1.25), ModelResult::Scalar(-0.5)]);

        assert_eq!(values[&3], expected);
        assert_eq!(values[&4], expected);
    }

    #[test]
    fn bind_debug_string_shows_folded_plated_observations() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, NodeIR::Scalar { value: 0.0 });
        graph.nodes.insert(2, NodeIR::Scalar { value: 1.0 });
        graph.nodes.insert(
            3,
            NodeIR::Random {
                dist_type: "Normal".into(),
                params: vec![ParamIR { from_node: 1 }, ParamIR { from_node: 2 }],
            },
        );
        graph.plates.insert(
            10,
            PlateIR {
                id: 10,
                n: 2,
                nodes: vec![3],
                plates: Vec::new(),
                data: HashMap::from([("x".to_string(), vec![1.25, -0.5])]),
                mapping: HashMap::from([(3, "x".to_string())]),
            },
        );

        let code = graph.bind_debug_string().unwrap();

        assert!(code.contains("2 effects, 0 computations, 0 dynamic slots"));
        assert!(code.contains("observe(\"node#3/plate#10[0]\", Normal(0.0, 1.0), 1.25)"));
        assert!(code.contains("observe(\"node#3/plate#10[1]\", Normal(0.0, 1.0), -0.5)"));
        assert!(!code.contains("sample("));
    }

    #[test]
    fn one_two_and_three_dimensional_shapes_are_cartesian_products() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, NodeIR::Scalar { value: 7.0 });
        graph.nodes.insert(2, NodeIR::Scalar { value: 8.0 });
        graph.nodes.insert(3, NodeIR::Scalar { value: 9.0 });
        graph.plates.insert(10, empty_plate(10, 2, vec![1, 2, 3]));
        graph.plates.insert(20, empty_plate(20, 3, vec![2, 3]));
        graph.plates.insert(30, empty_plate(30, 4, vec![3]));
        let values = graph.ancestral_sample().unwrap();

        fn scalars(value: &ModelResult) -> usize {
            match value {
                ModelResult::Scalar(_) => 1,
                ModelResult::Plate(items) => items.iter().map(scalars).sum(),
            }
        }
        assert_eq!(scalars(&values[&1]), 2);
        assert_eq!(scalars(&values[&2]), 6);
        assert_eq!(scalars(&values[&3]), 24);
        assert_eq!(
            instance_address(3, &[10, 20, 30], &[1, 2, 3]),
            "node#3/plate#10[1]/plate#20[2]/plate#30[3]"
        );
    }

    #[test]
    fn observation_owner_broadcasts_across_additional_dimensions() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, NodeIR::Scalar { value: 99.0 });
        let mut owner = empty_plate(10, 2, vec![1]);
        owner.data.insert("x".into(), vec![4.0, 5.0]);
        owner.mapping.insert(1, "x".into());
        graph.plates.insert(10, owner);
        graph.plates.insert(20, empty_plate(20, 3, vec![1]));

        let values = graph.ancestral_sample().unwrap();
        assert_eq!(
            values[&1],
            ModelResult::Plate(vec![
                ModelResult::Plate(vec![ModelResult::Scalar(4.0); 3]),
                ModelResult::Plate(vec![ModelResult::Scalar(5.0); 3]),
            ])
        );
    }
}
