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
    let expected = ModelResult::Plate(vec![ModelResult::Scalar(1.25), ModelResult::Scalar(-0.5)]);

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
            ModelResult::Scalar(_) | ModelResult::Integer(_) | ModelResult::Vector(_) => 1,
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
