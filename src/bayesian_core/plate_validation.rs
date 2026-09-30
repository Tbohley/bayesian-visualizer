use super::{GraphIR, NodeIR, ParamIR, PlateIR};
use std::collections::{HashMap, HashSet};

/// Validated plate dimensions, keyed independently of geometric nesting.
#[derive(Clone, Debug)]
pub(crate) struct NormalizedPlates {
    pub(crate) node_paths: HashMap<u32, Vec<u32>>,
    pub(crate) extents: HashMap<u32, usize>,
    pub(crate) observation_owners: HashMap<u32, u32>,
}

impl GraphIR {
    pub fn validate_plate_semantics(&self) -> Result<(), String> {
        self.validated_plates().map(|_| ())
    }

    /// Validates plates and derives each node's complete, deterministic dimension list.
    pub(crate) fn validated_plates(&self) -> Result<NormalizedPlates, String> {
        let normalized = self.normalize_plates()?;
        self.validate_dependency_scopes(&normalized)?;
        Ok(normalized)
    }

    pub(crate) fn normalize_plates(&self) -> Result<NormalizedPlates, String> {
        let mut plate_ids = self.plates.keys().copied().collect::<Vec<_>>();
        plate_ids.sort_unstable();
        let mut node_paths = self
            .nodes
            .keys()
            .copied()
            .map(|id| (id, Vec::new()))
            .collect::<HashMap<_, _>>();
        let mut extents = HashMap::with_capacity(plate_ids.len());
        let mut observation_owners = HashMap::new();

        for plate_id in plate_ids {
            let plate = &self.plates[&plate_id];
            validate_plate(self, plate_id, plate)?;
            extents.insert(plate_id, plate.n);
            for &node_id in &plate.nodes {
                node_paths
                    .get_mut(&node_id)
                    .expect("plate membership was validated")
                    .push(plate_id);
            }
            for &node_id in plate.mapping.keys() {
                if let Some(previous) = observation_owners.insert(node_id, plate_id) {
                return Err(format!(
                        "node {node_id} is observed by both plate {previous} and plate {plate_id}; exactly one observation owner is allowed"
                ));
            }
            }
            }

        Ok(NormalizedPlates {
            node_paths,
            extents,
            observation_owners,
        })
    }

    /// Scalar parameters have one producer per consumer only when producer dimensions are a subset.
    fn validate_dependency_scopes(&self, normalized: &NormalizedPlates) -> Result<(), String> {
        let mut consumer_ids = self.nodes.keys().copied().collect::<Vec<_>>();
        consumer_ids.sort_unstable();
        for consumer_id in consumer_ids {
            let consumer_dims = &normalized.node_paths[&consumer_id];
            for param in node_params(&self.nodes[&consumer_id]) {
                let producer_dims =
                    normalized.node_paths.get(&param.from_node).ok_or_else(|| {
                        format!(
                            "node {consumer_id} references missing node {}",
                            param.from_node
                        )
            })?;
                projection_positions(producer_dims, consumer_dims).map_err(|_| {
                    format!(
                        "invalid scalar plate dependency from node {} in scope {} to node {consumer_id} in scope {}: producer dimensions must be a subset of consumer dimensions",
                        param.from_node,
                        format_scope(producer_dims),
                        format_scope(consumer_dims),
                    )
            })?;
                }
        }
        Ok(())
                }
            }

fn validate_plate(graph: &GraphIR, plate_id: u32, plate: &PlateIR) -> Result<(), String> {
    if plate.id != plate_id {
                    return Err(format!(
            "plate map key {plate_id} does not match its stored ID {}",
            plate.id
                    ));
                }
    if plate.n == 0 {
                    return Err(format!(
            "plate {plate_id} extent must be a positive integer"
                    ));
                }
    ensure_unique(&plate.nodes, |node_id| {
        format!("plate {plate_id} lists node {node_id} more than once")
    })?;
    ensure_unique(&plate.plates, |child_id| {
        format!("plate {plate_id} lists contained plate {child_id} more than once")
    })?;
    for &node_id in &plate.nodes {
        if !graph.nodes.contains_key(&node_id) {
            return Err(format!("plate {plate_id} contains missing node {node_id}"));
                }
            }
            for &child_id in &plate.plates {
                if child_id == plate_id {
                    return Err(format!("plate {plate_id} cannot contain itself"));
                }
        if !graph.plates.contains_key(&child_id) {
                    return Err(format!(
                "plate {plate_id} contains missing plate {child_id}"
                    ));
                }
    }
    for (column, values) in &plate.data {
        if values.len() != plate.n {
                    return Err(format!(
                "plate {plate_id} column {column:?} has {} rows, expected {}",
                values.len(),
                plate.n
                    ));
                }
            }
    for (&node_id, column) in &plate.mapping {
        if !plate.nodes.contains(&node_id) {
            return Err(format!(
                "plate {plate_id} maps column {column:?} to node {node_id}, which is not a member"
            ));
        }
        if !plate.data.contains_key(column) {
            return Err(format!(
                "plate {plate_id} maps node {node_id} to missing column {column:?}"
            ));
        }
        if matches!(graph.nodes.get(&node_id), Some(NodeIR::Compute { .. })) {
            return Err(format!(
                "plate {plate_id} cannot map dataset column {column:?} to compute node {node_id}"
            ));
    }
            }
            Ok(())
        }

/// Returns consumer coordinate positions for producer dimensions.
pub(crate) fn projection_positions(
    producer_dims: &[u32],
    consumer_dims: &[u32],
) -> Result<Vec<usize>, ()> {
    let shared = shared_dimension_positions(producer_dims, consumer_dims);
    if shared.len() == producer_dims.len() {
        Ok(shared.into_iter().map(|(_, consumer)| consumer).collect())
    } else {
        Err(())
        }
    }

/// Positional correspondence for every stable plate identity shared by two dimension lists.
pub(crate) fn shared_dimension_positions(left: &[u32], right: &[u32]) -> Vec<(usize, usize)> {
    left.iter()
        .enumerate()
        .filter_map(|(left_position, plate)| {
            right
                .binary_search(plate)
                .ok()
                .map(|right_position| (left_position, right_position))
        })
        .collect()
}

fn ensure_unique(values: &[u32], error: impl Fn(u32) -> String) -> Result<(), String> {
    let mut seen = HashSet::with_capacity(values.len());
    for &value in values {
        if !seen.insert(value) {
            return Err(error(value));
        }
    }
    Ok(())
}

fn node_params(node: &NodeIR) -> &[ParamIR] {
    match node {
        NodeIR::Random { params, .. } | NodeIR::Compute { params, .. } => params,
        NodeIR::Scalar { .. } => &[],
    }
}

fn format_scope(path: &[u32]) -> String {
    if path.is_empty() {
        "root".into()
    } else {
        format!("{path:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nodes::Operation;

    fn scalar(id: u32) -> NodeIR {
        NodeIR::Scalar { id, value: 1.0 }
    }
    fn compute(id: u32, from_node: u32) -> NodeIR {
        NodeIR::Compute {
            id,
            operation: Operation::Exponential,
            params: vec![ParamIR { from_node }],
        }
    }
    fn plate(id: u32, n: usize, nodes: Vec<u32>) -> PlateIR {
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
    fn dimensions_include_all_memberships_in_stable_order() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, scalar(1));
        graph.nodes.insert(2, scalar(2));
        graph.plates.insert(20, plate(20, 3, vec![2]));
        graph.plates.insert(10, plate(10, 2, vec![2]));
        let normalized = graph.normalize_plates().unwrap();
        assert_eq!(normalized.node_paths[&1], Vec::<u32>::new());
        assert_eq!(normalized.node_paths[&2], vec![10, 20]);
    }

    #[test]
    fn non_dataset_plate_with_positive_extent_is_valid() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, scalar(1));
        graph.plates.insert(10, plate(10, 4, vec![1]));
        assert_eq!(graph.validate_plate_semantics(), Ok(()));
    }

    #[test]
    fn dataset_rows_define_and_must_match_extent() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, scalar(1));
        let mut p = plate(10, 2, vec![1]);
        p.data.insert("x".into(), vec![1.0, 2.0]);
        p.mapping.insert(1, "x".into());
        graph.plates.insert(10, p);
        assert!(graph.validate_plate_semantics().is_ok());
        graph.plates.get_mut(&10).unwrap().n = 3;
        assert!(
            graph
                .validate_plate_semantics()
                .unwrap_err()
                .contains("2 rows, expected 3")
        );
    }

    #[test]
    fn subset_dependencies_cover_root_same_and_outer_to_overlap() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, scalar(1));
        graph.nodes.insert(2, compute(2, 1));
        graph.nodes.insert(3, compute(3, 2));
        graph.plates.insert(10, plate(10, 2, vec![2, 3]));
        graph.plates.insert(20, plate(20, 3, vec![3]));
        assert!(graph.validate_plate_semantics().is_ok());
    }

    #[test]
    fn cross_sibling_and_deeper_to_shallower_dependencies_are_clear_errors() {
        let mut siblings = GraphIR::new();
        siblings.nodes.insert(1, scalar(1));
        siblings.nodes.insert(2, compute(2, 1));
        siblings.plates.insert(10, plate(10, 2, vec![1]));
        siblings.plates.insert(20, plate(20, 2, vec![2]));
        assert!(
            siblings
                .validate_plate_semantics()
                .unwrap_err()
                .contains("producer dimensions must be a subset")
        );

        let mut outward = GraphIR::new();
        outward.nodes.insert(1, scalar(1));
        outward.nodes.insert(2, compute(2, 1));
        outward.plates.insert(10, plate(10, 2, vec![1, 2]));
        outward.plates.insert(20, plate(20, 2, vec![1]));
        assert!(outward.validate_plate_semantics().is_err());
    }

    #[test]
    fn conflicting_observation_owners_are_rejected() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, scalar(1));
        for id in [10, 20] {
            let mut p = plate(id, 1, vec![1]);
            p.data.insert("x".into(), vec![1.0]);
            p.mapping.insert(1, "x".into());
            graph.plates.insert(id, p);
        }
        assert!(
            graph
                .validate_plate_semantics()
                .unwrap_err()
                .contains("exactly one observation owner")
        );
    }
}
