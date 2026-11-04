use super::inference::InferenceStep;
use super::*;
use crate::nodes::Operation;
use fugue::ChoiceValue;
use std::collections::HashMap;
fn c(g: &mut GraphIR, id: u32, value: Value) {
    g.nodes.insert(id, NodeIR::Constant { value });
}
fn op(g: &mut GraphIR, id: u32, operation: Operation, inputs: &[u32]) {
    g.nodes.insert(
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
fn rv(g: &mut GraphIR, id: u32, dist: &str, inputs: &[u32]) {
    g.nodes.insert(
        id,
        NodeIR::Random {
            dist_type: dist.into(),
            params: inputs
                .iter()
                .map(|&from_node| ParamIR { from_node })
                .collect(),
        },
    );
}
fn observe(g: &mut GraphIR, id: u32, value: f64) {
    g.plates.insert(
        100,
        PlateIR {
            id: 100,
            n: 1,
            nodes: vec![id],
            plates: vec![],
            data: HashMap::from([("x".into(), vec![value])]),
            mapping: HashMap::from([(id, "x".into())]),
        },
    );
}
#[test]
fn binomial_boundaries_have_finite_correct_scores() {
    use super::distributions::{DistributionKind, RuntimeDistribution};
    for (n, p, k, expected) in [
        (0, 0., 0, 0.),
        (0, 1., 0, 0.),
        (10, 0., 0, 0.),
        (10, 1., 10, 0.),
        (10, 0., 1, f64::NEG_INFINITY),
        (10, 1., 9, f64::NEG_INFINITY),
    ] {
        let RuntimeDistribution::Count(d) = DistributionKind::Binomial
            .create(&[Value::Integer(n), Value::Real(p)])
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(d.log_prob(&k), expected);
    }
}
#[test]
fn invalid_types_probabilities_and_observations_fail_compilation() {
    for probabilities in [vec![], vec![0., 0.], vec![-0.1, 1.1], vec![f64::NAN, 1.]] {
        let mut g = GraphIR::new();
        c(&mut g, 1, Value::RealVector(probabilities.into()));
        rv(&mut g, 2, "Categorical", &[1]);
        assert!(g.compile().is_err());
    }
    for observation in [-1., 0.5, 2.] {
        let mut g = GraphIR::new();
        c(&mut g, 1, Value::RealVector(vec![0.5, 0.5].into()));
        rv(&mut g, 2, "Categorical", &[1]);
        observe(&mut g, 2, observation);
        assert!(g.compile().is_err());
    }
    for (trials, p, observed) in [
        (Value::Real(10.), 0.5, 1.),
        (Value::Integer(-1), 0.5, 0.),
        (Value::Integer(2), 1.2, 1.),
        (Value::Integer(2), 0.5, 3.),
        (Value::Integer(2), 0.5, 0.5),
    ] {
        let mut g = GraphIR::new();
        c(&mut g, 1, trials);
        c(&mut g, 2, Value::Real(p));
        rv(&mut g, 3, "Binomial", &[1, 2]);
        observe(&mut g, 3, observed);
        assert!(g.compile().is_err());
    }
}
#[test]
fn categorical_prior_inference_uses_every_valid_category_and_preserves_integer_trace_types() {
    let mut g = GraphIR::new();
    c(&mut g, 1, Value::RealVector(vec![0., 0.1, 0.2, 0.7].into()));
    rv(&mut g, 2, "Categorical", &[1]);
    let g = g.compile().unwrap();
    let result = g.run_inference(123, 6000, 100).unwrap();
    let mut counts = [0; 4];
    for draw in &result.samples_by_node[&2] {
        let v = draw[0];
        assert_eq!(v.fract(), 0.);
        assert!((0. ..4.).contains(&v));
        counts[v as usize] += 1;
    }
    assert_eq!(counts[0], 0);
    for (i, p) in [0., 0.1, 0.2, 0.7].iter().enumerate() {
        assert!((counts[i] as f64 / 6000. - p).abs() < 0.025, "{counts:?}");
    }
    assert!(matches!(
        result.traces[0].choices.values().next().unwrap().value,
        ChoiceValue::Usize(_)
    ));
}
#[test]
fn beta_binomial_posterior_matches_conjugate_answer_and_predictive_counts() {
    let mut g = GraphIR::new();
    c(&mut g, 1, Value::Real(1.));
    c(&mut g, 2, Value::Integer(20));
    rv(&mut g, 3, "Beta", &[1, 1]);
    rv(&mut g, 4, "Binomial", &[2, 3]);
    observe(&mut g, 4, 14.);
    rv(&mut g, 5, "Binomial", &[2, 3]);
    let result = g.compile().unwrap().run_inference(42, 16000, 2000).unwrap();
    let mean = result.samples_by_node[&3].iter().map(|d| d[0]).sum::<f64>() / 16000.;
    let predictive = result.samples_by_node[&5]
        .iter()
        .map(|d| {
            assert!(d[0].fract() == 0. && (0. ..=20.).contains(&d[0]));
            d[0]
        })
        .sum::<f64>()
        / 16000.;
    assert!((mean - 15. / 22.).abs() < 0.025, "bias mean {mean}");
    assert!(
        (predictive - 20. * 15. / 22.).abs() < 0.5,
        "predictive {predictive}"
    );
    assert!(
        result
            .traces
            .iter()
            .all(|t| t.total_log_weight().is_finite())
    );
    assert!(
        result.traces[0]
            .choices
            .values()
            .any(|c| matches!(c.value, ChoiceValue::U64(_)))
    );
}
#[test]
fn categorical_mixture_posterior_matches_exact_enumeration() {
    let mut g = GraphIR::new();
    c(&mut g, 1, Value::RealVector(vec![0.25, 0.75].into()));
    c(&mut g, 2, Value::RealVector(vec![0., 2.].into()));
    c(&mut g, 3, Value::Real(1.));
    rv(&mut g, 4, "Categorical", &[1]);
    op(&mut g, 5, Operation::Index, &[2, 4]);
    rv(&mut g, 6, "Normal", &[5, 3]);
    observe(&mut g, 6, 0.);
    let g = g.compile().unwrap();
    let result = g.run_inference(7, 12000, 300).unwrap();
    let expected = 0.75 * (-2.0_f64).exp() / (0.25 + 0.75 * (-2.0_f64).exp());
    let observed = result.samples_by_node[&4].iter().map(|d| d[0]).sum::<f64>() / 12000.;
    assert!(
        (observed - expected).abs() < 0.025,
        "{observed} vs {expected}"
    );
    let values = g.posterior_predictive_sample(&result.traces[0]).unwrap();
    assert!(matches!(values[&4], graph_checks::ModelResult::Integer(_)));
    assert!(matches!(values[&6], graph_checks::ModelResult::Plate(_)));
}
#[test]
fn zero_warmup_cancellation_and_reproducibility_work_for_discrete_models() {
    let mut g = GraphIR::new();
    c(&mut g, 1, Value::RealVector(vec![0.4, 0.6].into()));
    rv(&mut g, 2, "Categorical", &[1]);
    let g = g.compile().unwrap();
    let a = g.run_inference(11, 20, 0).unwrap();
    let b = g.run_inference(11, 20, 0).unwrap();
    assert_eq!(a.samples_by_node, b.samples_by_node);
    let mut runner = g.inference_runner(1, 20, 10).unwrap();
    let InferenceStep::Complete(result) = runner.step(true).unwrap() else {
        panic!()
    };
    assert!(result.cancelled);
    assert_eq!(result.result.n_samples, 0);
}

#[test]
fn observed_categories_learn_their_probability_and_integer_predictions_resample() {
    let mut g = GraphIR::new();
    c(&mut g, 1, Value::Real(1.));
    rv(&mut g, 2, "Beta", &[1, 1]);
    op(&mut g, 3, Operation::Subtract, &[1, 2]);
    op(&mut g, 4, Operation::Collect, &[2, 3]);
    rv(&mut g, 5, "Categorical", &[4]);
    g.plates.insert(
        100,
        PlateIR {
            id: 100,
            n: 10,
            nodes: vec![5],
            plates: vec![],
            data: HashMap::from([(
                "category".into(),
                vec![0., 0., 0., 0., 0., 0., 1., 1., 1., 1.],
            )]),
            mapping: HashMap::from([(5, "category".into())]),
        },
    );
    let compiled = g.compile().unwrap();
    let result = compiled.run_inference(12, 10000, 1000).unwrap();
    let mean = result.samples_by_node[&2].iter().map(|d| d[0]).sum::<f64>() / 10000.;
    assert!((mean - 7. / 12.).abs() < 0.035, "{mean}");
    let prediction = compiled
        .posterior_predictive_sample(&result.traces[0])
        .unwrap();
    let graph_checks::ModelResult::Plate(values) = &prediction[&5] else {
        panic!()
    };
    assert_eq!(values.len(), 10);
    assert!(
        values
            .iter()
            .all(|v| matches!(v, graph_checks::ModelResult::Integer(0 | 1)))
    );
}
#[test]
fn cancelling_a_discrete_run_keeps_only_retained_integer_draws() {
    use std::cell::Cell;
    let mut g = GraphIR::new();
    c(&mut g, 1, Value::Integer(5));
    c(&mut g, 2, Value::Real(0.5));
    rv(&mut g, 3, "Binomial", &[1, 2]);
    let compiled = g.compile().unwrap();
    let stop = Cell::new(false);
    let result = compiled
        .run_inference_controlled(
            42,
            100,
            10,
            || stop.get(),
            |_| {},
            |index, _| {
                if index == 6 {
                    stop.set(true);
                }
            },
        )
        .unwrap();
    assert!(result.cancelled);
    assert_eq!(result.result.n_samples, 7);
    assert_eq!(result.result.samples_by_node[&3].len(), 7);
    assert!(
        result.result.samples_by_node[&3]
            .iter()
            .all(|d| d[0].fract() == 0. && (0. ..=5.).contains(&d[0]))
    );
}

#[test]
fn categorical_normalizes_weights_without_changing_the_input() {
    use super::distributions::{DistributionKind, RuntimeDistribution};
    for weights in [
        vec![0., 2., 6.],
        vec![0., 0.2, 0.6],
        vec![0., f64::MAX / 3., f64::MAX],
    ] {
        let input = Value::RealVector(weights.clone().into());
        let RuntimeDistribution::Category(d) = DistributionKind::Categorical
            .create(&[input.clone()])
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(d.log_prob(&0), f64::NEG_INFINITY);
        assert!((d.log_prob(&1).exp() - 0.25).abs() < 1e-12);
        assert!((d.log_prob(&2).exp() - 0.75).abs() < 1e-12);
        assert_eq!(input, Value::RealVector(weights.into()));
    }
    let mut g = GraphIR::new();
    c(&mut g, 1, Value::IntegerVector(vec![1, 3].into()));
    rv(&mut g, 2, "Categorical", &[1]);
    let result = g.compile().unwrap().run_inference(123, 2000, 10).unwrap();
    let frequency = result.samples_by_node[&2]
        .iter()
        .filter(|draw| draw[0] == 1.)
        .count() as f64
        / 2000.;
    assert!((frequency - 0.75).abs() < 0.04);
}
