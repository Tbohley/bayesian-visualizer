use super::*;
use crate::bayesian_core::{GraphIR, NodeIR, ParamIR, PlateIR, graph_checks::ModelResult};
use fugue::{Address, Choice, ChoiceValue, Distribution, Normal, ScoreGivenTrace, Trace};
use std::collections::HashMap;

fn reals(values: &[f64]) -> Value {
    Value::RealVector(values.to_vec().into())
}
fn integers(values: &[i64]) -> Value {
    Value::IntegerVector(values.to_vec().into())
}
fn real_values(value: Value) -> Vec<f64> {
    match value {
        Value::RealVector(values) => values.to_vec(),
        _ => panic!("expected real vector"),
    }
}
fn close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-12, "{actual:?} != {expected:?}");
    }
}
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
fn compilation_error(graph: GraphIR) -> super::super::CompilationError {
    match graph.compile_checked() {
        Err(error) => error,
        Ok(_) => panic!("expected compilation failure"),
    }
}

#[test]
fn binary_arithmetic_broadcasts_scalars_and_preserves_operand_order() {
    for (operation, expected) in [
        (Operation::Add, vec![6., 10.]),
        (Operation::Subtract, vec![2., 6.]),
        (Operation::Multiply, vec![8., 16.]),
        (Operation::Divide, vec![2., 4.]),
        (Operation::Power, vec![16., 64.]),
    ] {
        close(
            &real_values(
                operation
                    .evaluate_values(&[reals(&[4., 8.]), Value::Real(2.)])
                    .unwrap(),
            ),
            &expected,
        );
    }
    close(
        &real_values(
            Operation::Divide
                .evaluate_values(&[Value::Real(16.), integers(&[2, 4])])
                .unwrap(),
        ),
        &[8., 4.],
    );
    assert_eq!(
        Operation::Add
            .evaluate_values(&[integers(&[2, 4]), Value::Integer(1)])
            .unwrap(),
        integers(&[3, 5])
    );
    close(
        &real_values(
            Operation::Add
                .evaluate_values(&[integers(&[2, 4]), reals(&[0.5, 1.5])])
                .unwrap(),
        ),
        &[2.5, 5.5],
    );
    assert!(
        Operation::Add
            .evaluate_values(&[integers(&[1]), integers(&[1, 2])])
            .is_err()
    );
}

#[test]
fn logit_and_inverse_logit_are_elementwise_and_numerically_stable() {
    let probabilities = reals(&[0.2, 0.5, 0.8]);
    let logits = Operation::Logit
        .evaluate_values(&[probabilities.clone()])
        .unwrap();
    close(
        &real_values(logits.clone()),
        &[-4.0_f64.ln(), 0., 4.0_f64.ln()],
    );
    close(
        &real_values(Operation::InverseLogit.evaluate_values(&[logits]).unwrap()),
        &[0.2, 0.5, 0.8],
    );
    close(
        &real_values(
            Operation::InverseLogit
                .evaluate_values(&[reals(&[-1000., 0., 1000.])])
                .unwrap(),
        ),
        &[0., 0.5, 1.],
    );
    for invalid in [-1., 0., 1., 2.] {
        assert!(
            Operation::Logit
                .evaluate_values(&[Value::Real(invalid)])
                .is_err()
        );
    }
    let almost_one = f64::from_bits(1.0_f64.to_bits() - 1);
    assert!(
        Operation::Logit
            .evaluate(&[almost_one])
            .unwrap()
            .is_finite()
    );
    assert!(
        Operation::Logit
            .evaluate(&[f64::MIN_POSITIVE])
            .unwrap()
            .is_finite()
    );
}

#[test]
fn normalization_and_softmax_have_distinct_domains_and_results() {
    close(
        &real_values(
            Operation::Normalize
                .evaluate_values(&[reals(&[1., 2.])])
                .unwrap(),
        ),
        &[1. / 3., 2. / 3.],
    );
    let softmax = real_values(
        Operation::Softmax
            .evaluate_values(&[reals(&[1., 2.])])
            .unwrap(),
    );
    close(
        &softmax,
        &[
            1. / (1. + std::f64::consts::E),
            std::f64::consts::E / (1. + std::f64::consts::E),
        ],
    );
    close(
        &real_values(
            Operation::Softmax
                .evaluate_values(&[reals(&[-5., -5.])])
                .unwrap(),
        ),
        &[0.5, 0.5],
    );
    assert!(
        Operation::Normalize
            .evaluate_values(&[reals(&[-1., 2.])])
            .is_err()
    );
    assert!(
        Operation::Normalize
            .evaluate_values(&[integers(&[0, 0])])
            .is_err()
    );
    close(
        &real_values(
            Operation::Normalize
                .evaluate_values(&[reals(&[0., 2.])])
                .unwrap(),
        ),
        &[0., 1.],
    );
    close(
        &real_values(
            Operation::Softmax
                .evaluate_values(&[Value::IntegerVector(vec![0].into())])
                .unwrap(),
        ),
        &[1.],
    );
    for operation in [
        Operation::Normalize,
        Operation::Softmax,
        Operation::Sum,
        Operation::Product,
    ] {
        assert!(operation.evaluate_values(&[Value::Real(2.)]).is_err());
    }
}

#[test]
fn probability_transforms_remain_finite_for_extreme_finite_inputs() {
    close(
        &real_values(
            Operation::Normalize
                .evaluate_values(&[reals(&[f64::MAX, f64::MAX])])
                .unwrap(),
        ),
        &[0.5, 0.5],
    );
    close(
        &real_values(
            Operation::Softmax
                .evaluate_values(&[reals(&[1000., 1000.])])
                .unwrap(),
        ),
        &[0.5, 0.5],
    );
    close(
        &real_values(
            Operation::Softmax
                .evaluate_values(&[reals(&[f64::MAX, -f64::MAX])])
                .unwrap(),
        ),
        &[1., 0.],
    );
    for operation in [Operation::Normalize, Operation::Softmax] {
        assert!(
            operation
                .evaluate_values(&[reals(&[f64::NAN, 1.])])
                .is_err()
        );
        assert!(
            operation
                .evaluate_values(&[reals(&[f64::INFINITY])])
                .is_err()
        );
    }
}

#[test]
fn vector_reductions_preserve_exact_integers_and_reject_overflow() {
    assert_eq!(
        Operation::Sum
            .evaluate_values(&[integers(&[9_007_199_254_740_993, 2])])
            .unwrap(),
        Value::Integer(9_007_199_254_740_995)
    );
    assert_eq!(
        Operation::Product
            .evaluate_values(&[integers(&[2, 3, 4])])
            .unwrap(),
        Value::Integer(24)
    );
    assert_eq!(
        Operation::Sum
            .evaluate_values(&[reals(&[0.5, 1.5])])
            .unwrap(),
        Value::Real(2.)
    );
    assert_eq!(
        Operation::Product
            .evaluate_values(&[reals(&[0.5, 1.5])])
            .unwrap(),
        Value::Real(0.75)
    );
    assert!(
        Operation::Sum
            .evaluate_values(&[integers(&[i64::MAX, 1])])
            .is_err()
    );
    assert!(
        Operation::Product
            .evaluate_values(&[integers(&[i64::MAX, 2])])
            .is_err()
    );
}

#[test]
fn collect_preserves_order_and_promotes_mixed_scalar_types() {
    assert_eq!(
        Operation::Collect
            .evaluate_values(&[
                Value::Integer(3),
                Value::Integer(1),
                Value::Integer(3),
                Value::Integer(2)
            ])
            .unwrap(),
        integers(&[3, 1, 3, 2])
    );
    assert_eq!(
        Operation::Collect
            .evaluate_values(&[Value::Integer(3), Value::Real(0.5)])
            .unwrap(),
        reals(&[3., 0.5])
    );
    assert_eq!(
        Operation::Collect
            .evaluate_values(&[Value::Integer(i64::MAX)])
            .unwrap(),
        integers(&[i64::MAX])
    );
    assert!(Operation::Collect.evaluate_values(&[]).is_err());
    assert!(
        Operation::Collect
            .evaluate_values(&[reals(&[1., 2.])])
            .is_err()
    );
}

#[test]
fn compile_time_errors_identify_collect_and_transform_nodes() {
    let mut graph = GraphIR::new();
    constant(&mut graph, 1, reals(&[1., 2.]));
    compute(&mut graph, 2, Operation::Collect, &[1]);
    let error = compilation_error(graph);
    assert_eq!(error.node_ids, vec![2]);
    assert_eq!(error.parameter, Some(0));
    assert_eq!(error.expected, Some(ValueType::REAL));
    assert_eq!(
        error.actual,
        Some(ValueType::Vector {
            element: ScalarType::Real,
            length: 2
        })
    );
    for (operation, input) in [
        (Operation::Normalize, reals(&[0., 0.])),
        (Operation::Logit, reals(&[0.5, 1.])),
    ] {
        let mut graph = GraphIR::new();
        constant(&mut graph, 1, input);
        compute(&mut graph, 2, operation, &[1]);
        assert_eq!(compilation_error(graph).node_ids, vec![2]);
    }
}

#[test]
fn collect_and_reductions_keep_plate_axes_separate() {
    let mut graph = GraphIR::new();
    constant(&mut graph, 1, Value::Integer(2));
    constant(&mut graph, 2, Value::Integer(4));
    constant(&mut graph, 3, Value::Integer(3));
    compute(&mut graph, 4, Operation::Collect, &[3, 1, 2, 1]);
    compute(&mut graph, 5, Operation::Sum, &[4]);
    compute(&mut graph, 6, Operation::Normalize, &[4]);
    compute(&mut graph, 7, Operation::Product, &[4]);
    graph.plates.insert(
        10,
        PlateIR {
            id: 10,
            n: 2,
            nodes: vec![1, 4, 5, 6, 7],
            plates: vec![],
            data: HashMap::from([("n".into(), vec![1., 2.])]),
            mapping: HashMap::from([(1, "n".into())]),
        },
    );
    let compiled = graph.compile_checked().unwrap();
    assert_eq!(
        compiled.node_shapes()[&4].value_type,
        ValueType::Vector {
            element: ScalarType::Integer,
            length: 4
        }
    );
    assert_eq!(compiled.node_shapes()[&5].value_type, ValueType::INTEGER);
    assert_eq!(compiled.node_shapes()[&4].plate_extents, vec![2]);
    assert_eq!(compiled.node_shapes()[&5].plate_extents, vec![2]);
    let values = compiled.graph().ancestral_sample().unwrap();
    assert_eq!(
        values[&4],
        ModelResult::Plate(vec![
            ModelResult::Vector(integers(&[3, 1, 4, 1])),
            ModelResult::Vector(integers(&[3, 2, 4, 2]))
        ])
    );
    assert_eq!(
        values[&5],
        ModelResult::Plate(vec![ModelResult::Integer(9), ModelResult::Integer(11)])
    );
    assert_eq!(
        values[&7],
        ModelResult::Plate(vec![ModelResult::Integer(12), ModelResult::Integer(48)])
    );
}

#[test]
fn dynamic_collect_transforms_and_reductions_support_sampling_scoring_and_inference() {
    let mut graph = GraphIR::new();
    constant(&mut graph, 1, Value::Real(0.));
    constant(&mut graph, 2, Value::Real(1.));
    graph.nodes.insert(
        3,
        NodeIR::Random {
            dist_type: "Normal".into(),
            params: vec![ParamIR { from_node: 1 }, ParamIR { from_node: 2 }],
        },
    );
    compute(&mut graph, 4, Operation::Collect, &[3, 2, 1, 2]);
    compute(&mut graph, 5, Operation::Softmax, &[4]);
    compute(&mut graph, 6, Operation::Product, &[5]);
    compute(&mut graph, 7, Operation::Sum, &[4]);
    graph.nodes.insert(
        8,
        NodeIR::Random {
            dist_type: "Normal".into(),
            params: vec![ParamIR { from_node: 7 }, ParamIR { from_node: 2 }],
        },
    );
    let compiled = graph.compile_checked().unwrap();
    let mut base = Trace::default();
    for (id, value) in [(3, 0.5), (8, 3.0)] {
        let addr = Address(format!("node#{id}"));
        base.choices.insert(
            addr.clone(),
            Choice {
                addr,
                value: ChoiceValue::F64(value),
                logp: 0.,
            },
        );
    }
    let (values, trace) = fugue::runtime::handler::run(
        ScoreGivenTrace {
            base,
            trace: Trace::default(),
        },
        compiled.model().unwrap(),
    );
    let values = values.unwrap();
    assert_eq!(values[&4], ModelResult::Vector(reals(&[0.5, 1., 0., 1.])));
    assert_eq!(values[&7], ModelResult::Scalar(2.5));
    let ModelResult::Vector(probabilities) = values[&5].clone() else {
        panic!("expected vector")
    };
    assert!((real_values(probabilities).iter().sum::<f64>() - 1.).abs() < 1e-12);
    let expected_score =
        Normal::new(0., 1.).unwrap().log_prob(&0.5) + Normal::new(2.5, 1.).unwrap().log_prob(&3.);
    assert!((trace.total_log_weight() - expected_score).abs() < 1e-12);
    let result = compiled.run_inference(42, 4, 2).unwrap();
    assert_eq!(result.samples_by_node[&3].len(), 4);
    assert_eq!(
        result.node_shapes[&4].value_type,
        ValueType::Vector {
            element: ScalarType::Real,
            length: 4
        }
    );
}

#[test]
fn dynamically_invalid_weights_report_the_failing_node_address() {
    let mut graph = GraphIR::new();
    constant(&mut graph, 1, Value::Real(0.));
    constant(&mut graph, 2, Value::Real(1.));
    graph.nodes.insert(
        3,
        NodeIR::Random {
            dist_type: "Normal".into(),
            params: vec![ParamIR { from_node: 1 }, ParamIR { from_node: 2 }],
        },
    );
    compute(&mut graph, 4, Operation::Collect, &[3, 2, 2]);
    compute(&mut graph, 5, Operation::Normalize, &[4]);
    let compiled = graph.compile_checked().unwrap();
    let addr = Address("node#3".into());
    let mut base = Trace::default();
    base.choices.insert(
        addr.clone(),
        Choice {
            addr,
            value: ChoiceValue::F64(-0.5),
            logp: 0.,
        },
    );
    let (result, _) = fugue::runtime::handler::run(
        ScoreGivenTrace {
            base,
            trace: Trace::default(),
        },
        compiled.model().unwrap(),
    );
    let error = result.unwrap_err();
    assert!(error.contains("compute error at node#5"), "{error}");
    assert!(error.contains("weight 1 must be nonnegative"), "{error}");
}
