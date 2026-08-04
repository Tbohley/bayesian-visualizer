use super::{GraphIR, NodeIR, ParamIR, PlateIR};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub(crate) struct NormalizedPlates {
    pub(crate) root: NormalizedScope,
    pub(crate) node_paths: HashMap<u32, Vec<u32>>,
}

#[derive(Debug)]
pub(crate) struct NormalizedScope {
    pub(crate) plate: Option<NormalizedPlate>,
    pub(crate) nodes: Vec<u32>,
    pub(crate) children: Vec<NormalizedScope>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedPlate {
    pub(crate) id: u32,
    pub(crate) n: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}

impl GraphIR {
    /// Validate plate containment and all parameter dependencies between scopes.
    ///
    /// A dependency is valid when the producer's plate path is a prefix of the
    /// consumer's path. This permits values to flow into the same scope or a
    /// nested scope, while rejecting flows out of a plate or across siblings.
    pub fn validate_plate_semantics(&self) -> Result<(), String> {
        self.validated_plates().map(|_| ())
    }

    pub(crate) fn validated_plates(&self) -> Result<NormalizedPlates, String> {
        let normalized = self.normalize_plates()?;
        self.validate_dependency_scopes(&normalized)?;
        Ok(normalized)
    }

    //Check that plates don't overlap and have legal node contents, create a pathway to each node through its nested plates
    //
    pub(crate) fn normalize_plates(&self) -> Result<NormalizedPlates, String> {
        let mut parents = HashMap::<u32, u32>::new();
        let mut node_owners = HashMap::<u32, u32>::new();
        let mut plate_ids = self.plates.keys().copied().collect::<Vec<_>>();
        plate_ids.sort_unstable();

        for plate_id in plate_ids {
            let plate = self
                .plates
                .get(&plate_id)
                .expect("plate ID came from the plate map");
            if plate.id != plate_id {
                return Err(format!(
                    "plate map key {plate_id} does not match its stored ID {}",
                    plate.id
                ));
            }
            if plate.data.is_empty() {
                return Err(format!("plate {plate_id} has no dataset"));
            }

            ensure_unique(&plate.nodes, |node_id| {
                format!("plate {plate_id} lists node {node_id} more than once")
            })?;
            ensure_unique(&plate.plates, |child_id| {
                format!("plate {plate_id} lists child plate {child_id} more than once")
            })?;

            for &node_id in &plate.nodes {
                if !self.nodes.contains_key(&node_id) {
                    return Err(format!("plate {plate_id} contains missing node {node_id}"));
                }
                if let Some(previous_owner) = node_owners.insert(node_id, plate_id) {
                    return Err(format!(
                        "node {node_id} belongs directly to both plate {previous_owner} and plate {plate_id}"
                    ));
                }
            }

            for (&node_id, column) in &plate.mapping {
                if !plate.nodes.contains(&node_id) {
                    return Err(format!(
                        "plate {plate_id} maps column {column:?} to node {node_id}, which is not a direct member"
                    ));
                }
                if !plate.data.contains_key(column) {
                    return Err(format!(
                        "plate {plate_id} maps node {node_id} to missing column {column:?}"
                    ));
                }
                if matches!(self.nodes.get(&node_id), Some(NodeIR::Compute { .. })) {
                    return Err(format!(
                        "plate {plate_id} cannot map dataset column {column:?} to compute node {node_id}"
                    ));
                }
            }

            for &child_id in &plate.plates {
                if child_id == plate_id {
                    return Err(format!("plate {plate_id} cannot contain itself"));
                }
                if !self.plates.contains_key(&child_id) {
                    return Err(format!(
                        "plate {plate_id} contains missing child plate {child_id}"
                    ));
                }
                if let Some(previous_parent) = parents.insert(child_id, plate_id) {
                    return Err(format!(
                        "plate {child_id} has multiple parents: {previous_parent} and {plate_id}"
                    ));
                }
            }
        }

        self.check_plate_cycles()?;

        let mut root_node_ids = self
            .nodes
            .keys()
            .filter(|node_id| !node_owners.contains_key(node_id))
            .copied()
            .collect::<Vec<_>>();
        root_node_ids.sort_unstable();

        let mut root_plate_ids = self
            .plates
            .keys()
            .filter(|plate_id| !parents.contains_key(plate_id))
            .copied()
            .collect::<Vec<_>>();
        root_plate_ids.sort_unstable();

        let mut node_paths = HashMap::with_capacity(self.nodes.len());
        for &node_id in &root_node_ids {
            node_paths.insert(node_id, Vec::new());
        }

        let mut root_children = Vec::with_capacity(root_plate_ids.len());
        for plate_id in root_plate_ids {
            root_children.push(self.build_normalized_scope(
                plate_id,
                &mut Vec::new(),
                &mut node_paths,
            )?);
        }

        Ok(NormalizedPlates {
            root: NormalizedScope {
                plate: None,
                nodes: root_node_ids,
                children: root_children,
            },
            node_paths,
        })
    }

    fn build_normalized_scope(
        &self,
        plate_id: u32,
        parent_path: &mut Vec<u32>,
        node_paths: &mut HashMap<u32, Vec<u32>>,
    ) -> Result<NormalizedScope, String> {
        let plate = self
            .plates
            .get(&plate_id)
            .ok_or_else(|| format!("missing plate {plate_id} during normalization"))?;

        parent_path.push(plate_id);

        let mut nodes = plate.nodes.clone();
        nodes.sort_unstable();
        for &node_id in &nodes {
            node_paths.insert(node_id, parent_path.clone());
        }

        let mut child_ids = plate.plates.clone();
        child_ids.sort_unstable();
        let mut children = Vec::with_capacity(child_ids.len());
        for child_id in child_ids {
            children.push(self.build_normalized_scope(child_id, parent_path, node_paths)?);
        }

        parent_path.pop();

        Ok(NormalizedScope {
            plate: Some(NormalizedPlate {
                id: plate.id,
                n: plate.n,
            }),
            nodes,
            children,
        })
    }

    fn check_plate_cycles(&self) -> Result<(), String> {
        fn visit(
            plate_id: u32,
            plates: &HashMap<u32, PlateIR>,
            states: &mut HashMap<u32, VisitState>,
            stack: &mut Vec<u32>,
        ) -> Result<(), String> {
            match states.get(&plate_id) {
                Some(VisitState::Visited) => return Ok(()),
                Some(VisitState::Visiting) => {
                    let cycle_start = stack.iter().position(|id| *id == plate_id).unwrap_or(0);
                    let mut cycle = stack[cycle_start..].to_vec();
                    cycle.push(plate_id);
                    return Err(format!("plate hierarchy contains a cycle: {cycle:?}"));
                }
                None => {}
            }

            states.insert(plate_id, VisitState::Visiting);
            stack.push(plate_id);

            let plate = plates
                .get(&plate_id)
                .ok_or_else(|| format!("missing plate {plate_id} during cycle checking"))?;
            for &child_id in &plate.plates {
                visit(child_id, plates, states, stack)?;
            }

            stack.pop();
            states.insert(plate_id, VisitState::Visited);
            Ok(())
        }

        let mut states = HashMap::new();
        let mut stack = Vec::new();
        let mut plate_ids = self.plates.keys().copied().collect::<Vec<_>>();
        plate_ids.sort_unstable();

        for plate_id in plate_ids {
            visit(plate_id, &self.plates, &mut states, &mut stack)?;
        }
        Ok(())
    }

    //ensure there are no links from within plates to outside of plates, from sibling plates to each other, etc
    fn validate_dependency_scopes(&self, normalized: &NormalizedPlates) -> Result<(), String> {
        let mut consumer_ids = self.nodes.keys().copied().collect::<Vec<_>>();
        consumer_ids.sort_unstable();

        for consumer_id in consumer_ids {
            let node = self
                .nodes
                .get(&consumer_id)
                .expect("node ID came from the node map");
            let consumer_path = normalized.node_paths.get(&consumer_id).ok_or_else(|| {
                format!("node {consumer_id} has no scope after plate normalization")
            })?;

            for param in node_params(node) {
                let producer_path =
                    normalized.node_paths.get(&param.from_node).ok_or_else(|| {
                        format!(
                            "node {consumer_id} references missing node {}",
                            param.from_node
                        )
                    })?;

                if !consumer_path.starts_with(producer_path) {
                    return Err(format!(
                        "invalid plate dependency from node {} in scope {} to node {consumer_id} in scope {}",
                        param.from_node,
                        format_scope(producer_path),
                        format_scope(consumer_path),
                    ));
                }
            }
        }
        Ok(())
    }
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
        "root".to_string()
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
            params: vec![ParamIR {
                from_node,
                param_name: None,
            }],
        }
    }

    fn plate(id: u32, n: usize, nodes: Vec<u32>, plates: Vec<u32>) -> PlateIR {
        PlateIR {
            id,
            n,
            nodes,
            plates,
            data: HashMap::from([("value".to_string(), vec![0.0; n])]),
            mapping: HashMap::new(),
        }
    }

    #[test]
    fn normalizes_arbitrarily_nested_plates_and_root_nodes() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, scalar(1));
        graph.nodes.insert(2, scalar(2));
        graph.nodes.insert(3, scalar(3));
        graph.plates.insert(10, plate(10, 0, vec![2], vec![11]));
        graph.plates.insert(11, plate(11, 7, vec![3], vec![]));

        let normalized = graph.normalize_plates().unwrap();

        assert_eq!(normalized.root.nodes, vec![1]);
        assert_eq!(normalized.root.children[0].plate.unwrap().id, 10);
        assert_eq!(normalized.root.children[0].plate.unwrap().n, 0);
        assert_eq!(
            normalized.root.children[0].children[0].plate.unwrap().id,
            11
        );
        assert_eq!(normalized.node_paths[&1], Vec::<u32>::new());
        assert_eq!(normalized.node_paths[&2], vec![10]);
        assert_eq!(normalized.node_paths[&3], vec![10, 11]);
    }

    #[test]
    fn rejects_missing_and_multiply_owned_contents() {
        let mut missing = GraphIR::new();
        missing.plates.insert(10, plate(10, 1, vec![99], vec![]));
        assert!(missing.validate_plate_semantics().is_err());

        let mut duplicate_node = GraphIR::new();
        duplicate_node.nodes.insert(1, scalar(1));
        duplicate_node
            .plates
            .insert(10, plate(10, 1, vec![1], vec![]));
        duplicate_node
            .plates
            .insert(11, plate(11, 1, vec![1], vec![]));
        assert!(duplicate_node.validate_plate_semantics().is_err());

        let mut duplicate_parent = GraphIR::new();
        duplicate_parent
            .plates
            .insert(10, plate(10, 1, vec![], vec![12]));
        duplicate_parent
            .plates
            .insert(11, plate(11, 1, vec![], vec![12]));
        duplicate_parent
            .plates
            .insert(12, plate(12, 1, vec![], vec![]));
        assert!(duplicate_parent.validate_plate_semantics().is_err());
    }

    #[test]
    fn rejects_plate_cycles() {
        let mut graph = GraphIR::new();
        graph.plates.insert(10, plate(10, 1, vec![], vec![11]));
        graph.plates.insert(11, plate(11, 1, vec![], vec![10]));

        assert!(graph.validate_plate_semantics().is_err());
    }

    #[test]
    fn allows_dependencies_into_same_or_nested_scope() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, scalar(1));
        graph.nodes.insert(2, compute(2, 1));
        graph.nodes.insert(3, compute(3, 2));
        graph.nodes.insert(4, compute(4, 3));
        graph.plates.insert(10, plate(10, 3, vec![2, 3], vec![11]));
        graph.plates.insert(11, plate(11, 4, vec![4], vec![]));

        assert_eq!(graph.validate_plate_semantics(), Ok(()));
    }

    #[test]
    fn rejects_dependencies_out_of_or_across_plates() {
        let mut outward = GraphIR::new();
        outward.nodes.insert(1, scalar(1));
        outward.nodes.insert(2, compute(2, 1));
        outward.plates.insert(10, plate(10, 3, vec![1], vec![]));
        assert!(outward.validate_plate_semantics().is_err());

        let mut sibling = GraphIR::new();
        sibling.nodes.insert(1, scalar(1));
        sibling.nodes.insert(2, compute(2, 1));
        sibling.plates.insert(10, plate(10, 3, vec![1], vec![]));
        sibling.plates.insert(11, plate(11, 3, vec![2], vec![]));
        assert!(sibling.validate_plate_semantics().is_err());
    }
}
