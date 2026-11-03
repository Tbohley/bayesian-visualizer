use super::*;
use crate::{
    bayesian_core::{ParamIR, PlateIR, ScalarType, graph_checks::ModelResult},
    nodes::Operation,
};

fn constant(graph: &mut GraphIR, id: u32, value: Value) {
    graph.nodes.insert(id, NodeIR::Constant { value });
}
fn compute(graph: &mut GraphIR, id: u32, operation: Operation, inputs: &[u32]) {
    graph.nodes.insert(
        id,
        NodeIR::Compute {
            operation,
            params: inputs
                .iter()
                .map(|&from_node| ParamIR { from_node })
                .collect(),
        },
    );
}
fn normal(graph: &mut GraphIR, id: u32, mean: u32, scale: u32) {
    graph.nodes.insert(
        id,
        NodeIR::Random {
            dist_type: "Normal".into(),
            params: vec![ParamIR { from_node: mean }, ParamIR { from_node: scale }],
        },
    );
}
fn plate(graph: &mut GraphIR, nodes: Vec<u32>) {
    graph.plates.insert(
        10,
        PlateIR {
            id: 10,
            n: 2,
            nodes,
            plates: vec![],
            data: HashMap::new(),
            mapping: HashMap::new(),
        },
    );
}
fn error(graph: GraphIR) -> CompilationError {
    match graph.compile_checked() {
        Err(error) => error,
        Ok(_) => panic!("expected compilation failure"),
    }
}

#[test]
fn vector_components_are_independent_of_plate_instances() {
    let mut graph = GraphIR::new();
    constant(&mut graph, 1, Value::IntegerVector(vec![1, 2, 3].into()));
    constant(&mut graph, 2, Value::Integer(4));
    compute(&mut graph, 3, Operation::Add, &[1, 2]);
    plate(&mut graph, vec![1, 3]);
    let compiled = graph.compile_checked().unwrap();
    assert_eq!(
        compiled.node_shapes()[&3],
        NodeShape {
            value_type: ValueType::Vector {
                element: ScalarType::Integer,
                length: 3
            },
            plate_ids: vec![10],
            plate_extents: vec![2]
        }
    );
    let values = compiled.graph().ancestral_sample().unwrap();
    assert_eq!(
        values[&3],
        ModelResult::Plate(vec![
            ModelResult::Vector(Value::IntegerVector(
                vec![5, 6, 7].into()
            ));
            2
        ])
    );
}

#[test]
fn unused_mismatched_vectors_report_the_consumer_parameter() {
    let mut graph = GraphIR::new();
    constant(&mut graph, 1, Value::RealVector(vec![1.0, 2.0].into()));
    constant(&mut graph, 2, Value::RealVector(vec![1.0, 2.0, 3.0].into()));
    compute(&mut graph, 3, Operation::Add, &[1, 2]);
    let error = error(graph);
    assert_eq!(error.node_ids, vec![3]);
    assert_eq!(error.parameter, Some(1));
    assert_eq!(
        error.expected,
        Some(ValueType::Vector {
            element: ScalarType::Real,
            length: 2
        })
    );
    assert_eq!(
        error.actual,
        Some(ValueType::Vector {
            element: ScalarType::Real,
            length: 3
        })
    );
}

#[test]
fn distributions_reject_vectors_with_structured_diagnostics() {
    let mut graph = GraphIR::new();
    constant(&mut graph, 1, Value::RealVector(vec![1.0].into()));
    constant(&mut graph, 2, Value::Integer(1));
    normal(&mut graph, 3, 1, 2);
    let error = error(graph);
    assert_eq!(error.node_ids, vec![3]);
    assert_eq!(error.parameter, Some(0));
    assert_eq!(error.expected, Some(ValueType::REAL));
    assert_eq!(
        error.actual,
        Some(ValueType::Vector {
            element: ScalarType::Real,
            length: 1
        })
    );
}

#[test]
fn integer_arithmetic_preserves_exact_values_and_real_operations_widen() {
    let mut graph = GraphIR::new();
    constant(&mut graph, 1, Value::Integer(9_007_199_254_740_993));
    constant(&mut graph, 2, Value::Integer(2));
    compute(&mut graph, 3, Operation::Add, &[1, 2]);
    compute(&mut graph, 4, Operation::Divide, &[2, 2]);
    let compiled = graph.compile_checked().unwrap();
    assert_eq!(compiled.node_shapes()[&3].value_type, ValueType::INTEGER);
    assert_eq!(compiled.node_shapes()[&4].value_type, ValueType::REAL);
    let values = compiled.graph().ancestral_sample().unwrap();
    assert_eq!(values[&3], ModelResult::Integer(9_007_199_254_740_995));
    assert_eq!(values[&4], ModelResult::Scalar(1.0));
}

#[test]
fn rejects_unused_invalid_nodes_before_pruning() {
    for operation in [Operation::Logarithm, Operation::Add] {
        let mut graph = GraphIR::new();
        constant(&mut graph, 1, Value::Real(-1.0));
        compute(&mut graph, 2, operation, &[1]);
        assert_eq!(error(graph).node_ids, vec![2]);
    }
    let mut graph = GraphIR::new();
    constant(&mut graph, 1, Value::Integer(i64::MAX));
    constant(&mut graph, 2, Value::Integer(1));
    compute(&mut graph, 3, Operation::Add, &[1, 2]);
    assert!(error(graph).message.contains("overflow"));
}

#[test]
fn empty_and_nonfinite_values_are_compile_errors() {
    for value in [
        Value::Real(f64::NAN),
        Value::RealVector(vec![f64::INFINITY].into()),
        Value::RealVector(vec![].into()),
        Value::IntegerVector(vec![].into()),
    ] {
        let mut graph = GraphIR::new();
        constant(&mut graph, 1, value);
        assert_eq!(error(graph).node_ids, vec![1]);
    }
}

#[test]
fn mapped_integers_remain_typed_and_fractional_rows_are_rejected() {
    let mut graph = GraphIR::new();
    constant(&mut graph, 1, Value::Integer(0));
    plate(&mut graph, vec![1]);
    let rows = graph.plates.get_mut(&10).unwrap();
    rows.mapping.insert(1, "n".into());
    rows.data.insert("n".into(), vec![2.0, 3.0]);
    assert_eq!(
        graph.ancestral_sample().unwrap()[&1],
        ModelResult::Plate(vec![ModelResult::Integer(2), ModelResult::Integer(3)])
    );
    graph
        .plates
        .get_mut(&10)
        .unwrap()
        .data
        .get_mut("n")
        .unwrap()[1] = 3.5;
    let error = error(graph);
    assert_eq!(error.node_ids, vec![1]);
    assert!(error.message.contains("row 1"));
}

#[test]
fn runtime_vector_broadcasting_survives_sampling_and_replay() {
    let mut graph = GraphIR::new();
    constant(&mut graph, 1, Value::Integer(0));
    constant(&mut graph, 2, Value::Integer(1));
    normal(&mut graph, 3, 1, 2);
    constant(&mut graph, 4, Value::IntegerVector(vec![2, 4].into()));
    compute(&mut graph, 5, Operation::Add, &[3, 4]);
    let compiled = graph.compile_checked().unwrap();
    let mut rng = rand::thread_rng();
    let (sample, trace) = fugue::runtime::handler::run(
        fugue::PriorHandler {
            rng: &mut rng,
            trace: fugue::Trace::default(),
        },
        compiled.model().unwrap(),
    );
    let sample = sample.unwrap();
    let ModelResult::Scalar(x) = sample[&3] else {
        panic!("expected scalar")
    };
    assert_eq!(
        sample[&5],
        ModelResult::Vector(Value::RealVector(vec![x + 2.0, x + 4.0].into()))
    );
    let (replayed, _) = fugue::runtime::handler::run(
        fugue::ScoreGivenTrace {
            base: trace,
            trace: fugue::Trace::default(),
        },
        compiled.model().unwrap(),
    );
    assert_eq!(sample, replayed.unwrap());
    let result = compiled.run_inference(42, 4, 2).unwrap();
    assert_eq!(
        result.node_shapes[&5].value_type,
        ValueType::Vector {
            element: ScalarType::Real,
            length: 2
        }
    );
    assert_eq!(result.samples_by_node[&3].len(), 4);
}
