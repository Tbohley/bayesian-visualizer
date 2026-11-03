use super::*;
use crate::nodes::Operation;

fn scalar() -> NodeIR {
    NodeIR::Scalar { value: 1.0 }
}
fn compute(from_node: u32) -> NodeIR {
    NodeIR::Compute {
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
    graph.nodes.insert(1, scalar());
    graph.nodes.insert(2, scalar());
    graph.plates.insert(20, plate(20, 3, vec![2]));
    graph.plates.insert(10, plate(10, 2, vec![2]));
    let normalized = graph.normalize_plates().unwrap();
    assert_eq!(normalized.node_paths[&1], Vec::<u32>::new());
    assert_eq!(normalized.node_paths[&2], vec![10, 20]);
}

#[test]
fn non_dataset_plate_with_positive_extent_is_valid() {
    let mut graph = GraphIR::new();
    graph.nodes.insert(1, scalar());
    graph.plates.insert(10, plate(10, 4, vec![1]));
    assert_eq!(graph.validate_plate_semantics(), Ok(()));
}

#[test]
fn dataset_rows_define_and_must_match_extent() {
    let mut graph = GraphIR::new();
    graph.nodes.insert(1, scalar());
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
    graph.nodes.insert(1, scalar());
    graph.nodes.insert(2, compute(1));
    graph.nodes.insert(3, compute(2));
    graph.plates.insert(10, plate(10, 2, vec![2, 3]));
    graph.plates.insert(20, plate(20, 3, vec![3]));
    assert!(graph.validate_plate_semantics().is_ok());
}

#[test]
fn cross_sibling_and_deeper_to_shallower_dependencies_are_clear_errors() {
    let mut siblings = GraphIR::new();
    siblings.nodes.insert(1, scalar());
    siblings.nodes.insert(2, compute(1));
    siblings.plates.insert(10, plate(10, 2, vec![1]));
    siblings.plates.insert(20, plate(20, 2, vec![2]));
    assert!(
        siblings
            .validate_plate_semantics()
            .unwrap_err()
            .contains("producer dimensions must be a subset")
    );

    let mut outward = GraphIR::new();
    outward.nodes.insert(1, scalar());
    outward.nodes.insert(2, compute(1));
    outward.plates.insert(10, plate(10, 2, vec![1, 2]));
    outward.plates.insert(20, plate(20, 2, vec![1]));
    assert!(outward.validate_plate_semantics().is_err());
}

#[test]
fn conflicting_observation_owners_are_rejected() {
    let mut graph = GraphIR::new();
    graph.nodes.insert(1, scalar());
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
