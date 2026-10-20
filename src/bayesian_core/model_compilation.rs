#[cfg(test)]
use super::ParamIR;
use super::execution::{ExecutionPlan, Mode};
use super::graph_checks::{GraphModel, ModelResult, ModelValues, PosteriorValues};
use super::plate_validation::NormalizedPlates;
use super::{GraphIR, NodeIR};
use fugue::Model;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// Validated graph and immutable execution plans shared by every model run.
#[derive(Clone)]
pub struct CompiledGraph {
    graph: Arc<GraphIR>,
    normalized: Arc<NormalizedPlates>,
    order: Arc<[u32]>,
    inference_plan: Arc<ExecutionPlan>,
    predictive_plan: Arc<ExecutionPlan>,
    full_plan: Arc<OnceLock<Result<Arc<ExecutionPlan>, String>>>,
}

impl CompiledGraph {
    pub(crate) fn new(graph: GraphIR) -> Result<Self, String> {
        if let Err(cycle) = graph.check_cycles() {
            return Err(format!(
                "graph contains a cycle including node IDs: {cycle:?}"
            ));
        }
        let order = graph.topological_sort()?;
        let normalized = graph.validated_plates()?;
        let inference_plan = Arc::new(ExecutionPlan::compile(
            &graph,
            &normalized,
            &order,
            Mode::Inference,
            false,
        )?);
        let predictive_plan = Arc::new(ExecutionPlan::compile(
            &graph,
            &normalized,
            &order,
            Mode::Predictive,
            false,
        )?);
        Ok(Self {
            graph: Arc::new(graph),
            normalized: Arc::new(normalized),
            order: order.into(),
            inference_plan,
            predictive_plan,
            full_plan: Arc::new(OnceLock::new()),
        })
    }
    pub fn graph(&self) -> &GraphIR {
        &self.graph
    }
    pub fn node_plate_path(&self, node_id: u32) -> Option<&[u32]> {
        self.normalized.node_paths.get(&node_id).map(Vec::as_slice)
    }
    /// Full values for occasional graph inspection; inference uses flat latent draws.
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
    pub(crate) fn predictive_model(&self) -> Result<GraphModel, String> {
        Ok(self.predictive_plan.model())
    }
}

impl GraphIR {
    /// Describes the optimized execution plan, including folded constants.
    #[cfg(test)]
    pub fn bind_debug_string(&self) -> Result<String, String> {
        let compiled = CompiledGraph::new(self.clone())?;
        compiled.bind_debug_string()
    }

    /// Formats every sampled node value as human-readable instance lines.
    pub fn format_model_values(&self, values: &ModelValues) -> Result<String, String> {
        let normalized = self.validated_plates()?;
        let mut node_ids = values.keys().copied().collect::<Vec<_>>();
        node_ids.sort_unstable();
        let mut lines = Vec::new();

        for node_id in node_ids {
            let value = values
                .get(&node_id)
                .ok_or_else(|| format!("sample results are missing node {node_id}"))?;
            format_node_instances(self, &normalized, node_id, value, &mut lines)?;
        }

        Ok(lines.join("\n"))
    }

    /// Formats one node's scalar or plated result as human-readable instance lines.
    pub fn format_node_value(&self, node_id: u32, value: &ModelResult) -> Result<String, String> {
        let normalized = self.validated_plates()?;
        let mut lines = Vec::new();
        format_node_instances(self, &normalized, node_id, value, &mut lines)?;

        if lines.is_empty() {
            let node = self
                .nodes
                .get(&node_id)
                .ok_or_else(|| format!("graph is missing node {node_id}"))?;
            return Ok(format!(
                "{} @ node#{node_id} = {value:?}",
                node_display_name(node)
            ));
        }

        Ok(lines.join("\n"))
    }
}

impl CompiledGraph {
    /// Describes the optimized instructions actually used by inference.
    pub fn bind_debug_string(&self) -> Result<String, String> {
        Ok(self.inference_plan.debug_string())
    }
}

/// Formats all instances of a node according to its normalized plate path.
fn format_node_instances(
    graph: &GraphIR,
    normalized: &NormalizedPlates,
    node_id: u32,
    value: &ModelResult,
    lines: &mut Vec<String>,
) -> Result<(), String> {
    let plate_ids = normalized
        .node_paths
        .get(&node_id)
        .ok_or_else(|| format!("node {node_id} has no normalized plate path"))?;
    let display_name = node_display_name(
        graph
            .nodes
            .get(&node_id)
            .ok_or_else(|| format!("graph is missing node {node_id}"))?,
    );
    format_instances(
        node_id,
        &display_name,
        plate_ids,
        value,
        0,
        &mut Vec::new(),
        lines,
    )
}

/// Walks a nested model result and appends one display line for each scalar instance.
fn format_instances(
    node_id: u32,
    display_name: &str,
    plate_ids: &[u32],
    value: &ModelResult,
    depth: usize,
    indices: &mut Vec<usize>,
    lines: &mut Vec<String>,
) -> Result<(), String> {
    match (depth == plate_ids.len(), value) {
        (true, ModelResult::Scalar(value)) => {
            lines.push(format!(
                "{display_name} @ {} = {value:?}",
                instance_address(node_id, plate_ids, indices)
            ));
            Ok(())
        }
        (false, ModelResult::Plate(items)) => {
            for (index, item) in items.iter().enumerate() {
                indices.push(index);
                format_instances(
                    node_id,
                    display_name,
                    plate_ids,
                    item,
                    depth + 1,
                    indices,
                    lines,
                )?;
                indices.pop();
            }
            Ok(())
        }
        (true, ModelResult::Plate(_)) => Err(format!(
            "node {node_id} has an unexpected extra plate dimension"
        )),
        (false, ModelResult::Scalar(_)) => Err(format!(
            "node {node_id} is missing plate dimension {}",
            plate_ids[depth]
        )),
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

/// Returns a node's label when present, or a fallback name based on its ID.
fn node_display_name(node: &NodeIR) -> String {
    match node {
        NodeIR::Random {
            id,
            label: Some(label),
            ..
        } => format!("{label} (node {id})"),
        _ => format!("node {}", node_id(node)),
    }
}

/// Extracts the ID stored by any graph node variant.
fn node_id(node: &NodeIR) -> u32 {
    match node {
        NodeIR::Random { id, .. } | NodeIR::Scalar { id, .. } | NodeIR::Compute { id, .. } => *id,
    }
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
        graph.nodes.insert(1, NodeIR::Scalar { id: 1, value: 0.0 });
        graph.nodes.insert(2, NodeIR::Scalar { id: 2, value: 1.0 });
        graph.nodes.insert(
            3,
            NodeIR::Random {
                id: 3,
                label: None,
                dist_type: "Normal".to_string(),
                params: vec![ParamIR { from_node: 1 }, ParamIR { from_node: 2 }],
            },
        );
        graph.nodes.insert(
            4,
            NodeIR::Scalar {
                id: 4,
                value: 999.0,
            },
        );
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
        graph.nodes.insert(1, NodeIR::Scalar { id: 1, value: 0.0 });
        graph.nodes.insert(2, NodeIR::Scalar { id: 2, value: 1.0 });
        graph.nodes.insert(
            3,
            NodeIR::Random {
                id: 3,
                label: Some("x".into()),
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
        graph.nodes.insert(1, NodeIR::Scalar { id: 1, value: 7.0 });
        graph.nodes.insert(2, NodeIR::Scalar { id: 2, value: 8.0 });
        graph.nodes.insert(3, NodeIR::Scalar { id: 3, value: 9.0 });
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
        graph.nodes.insert(1, NodeIR::Scalar { id: 1, value: 99.0 });
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
