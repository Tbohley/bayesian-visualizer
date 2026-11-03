use super::*;
use crate::bayesian_core::{CompiledGraph, PlateIR};
use fugue::{Choice, ChoiceValue, PriorHandler, ScoreGivenTrace, Trace};
use rand::{SeedableRng, rngs::StdRng};

fn scalar(graph: &mut GraphIR, id: u32, value: f64) {
    graph.nodes.insert(id, NodeIR::Scalar { value });
}
fn random(graph: &mut GraphIR, id: u32, distribution: &str, inputs: &[u32]) {
    graph.nodes.insert(
        id,
        NodeIR::Random {
            dist_type: distribution.into(),
            params: inputs
                .iter()
                .map(|&from_node| ParamIR { from_node })
                .collect(),
        },
    );
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
fn fixed_trace(values: &[(u32, f64)]) -> Trace {
    let mut trace = Trace::default();
    for &(id, value) in values {
        let address = Address(format!("node#{id}"));
        trace.choices.insert(
            address.clone(),
            Choice {
                addr: address,
                value: ChoiceValue::F64(value),
                logp: 0.0,
            },
        );
    }
    trace
}
fn score<A>(model: Model<A>, base: Trace) -> (A, Trace) {
    fugue::runtime::handler::run(
        ScoreGivenTrace {
            base,
            trace: Trace::default(),
        },
        model,
    )
}

pub(crate) fn regression(n: usize) -> CompiledGraph {
    let mut graph = GraphIR::new();
    scalar(&mut graph, 1, 0.0);
    scalar(&mut graph, 2, 1.0);
    random(&mut graph, 3, "Normal", &[1, 2]);
    scalar(&mut graph, 4, 0.0);
    compute(&mut graph, 5, Operation::Multiply, &[3, 4]);
    random(&mut graph, 7, "Normal", &[5, 2]);
    let mut rows = plate(100, n, vec![4, 5, 7]);
    rows.data
        .insert("x".into(), (0..n).map(|i| (i % 17) as f64 / 17.0).collect());
    rows.data.insert(
        "y".into(),
        (0..n)
            .map(|i| 0.7 * ((i % 17) as f64 / 17.0) + 0.1 * ((i % 5) as f64 - 2.0))
            .collect(),
    );
    rows.mapping = HashMap::from([(4, "x".into()), (7, "y".into())]);
    graph.plates.insert(100, rows);
    graph.compile().unwrap()
}

#[test]
fn folded_constants_and_unused_deterministic_branches_need_no_runtime_slots() {
    let mut graph = GraphIR::new();
    scalar(&mut graph, 1, 2.0);
    scalar(&mut graph, 2, 1.0);
    compute(&mut graph, 3, Operation::Multiply, &[1, 1]);
    random(&mut graph, 4, "Normal", &[3, 2]);
    scalar(&mut graph, 5, -1.0);
    compute(&mut graph, 6, Operation::Logarithm, &[5]); // unused, invalid branch
    let order = graph.topological_sort().unwrap();
    let normalized = graph.validated_plates().unwrap();
    let plan = Arc::new(
        ExecutionPlan::compile(&graph, &normalized, &order, Mode::Inference, false).unwrap(),
    );
    assert_eq!(plan.slot_count, 1);
    assert_eq!(plan.instructions.len(), 1);
    let (draw, trace) = score(plan.inference_model(), fixed_trace(&[(4, 4.0)]));
    assert_eq!(&*draw.unwrap()[&4], &[4.0]);
    assert_eq!(
        trace.log_prior,
        Normal::new(4.0, 1.0).unwrap().log_prob(&4.0)
    );
}

#[test]
fn parameter_projection_uses_plate_identity_across_overlapping_dimensions() {
    let mut graph = GraphIR::new();
    scalar(&mut graph, 1, 2.0);
    scalar(&mut graph, 2, 0.0);
    compute(&mut graph, 3, Operation::Multiply, &[1, 2]);
    graph.plates.insert(10, plate(10, 2, vec![3]));
    let mut producer = plate(20, 3, vec![2, 3]);
    producer.data.insert("x".into(), vec![1.0, 3.0, 7.0]);
    producer.mapping.insert(2, "x".into());
    graph.plates.insert(20, producer);
    let model = graph.compile().unwrap().model().unwrap();
    let (values, _) = score(model, Trace::default());
    assert_eq!(
        values.unwrap()[&3],
        ModelResult::Plate(vec![
            ModelResult::Plate(vec![
                ModelResult::Scalar(2.0),
                ModelResult::Scalar(6.0),
                ModelResult::Scalar(14.0)
            ]);
            2
        ])
    );
}

#[test]
fn flat_latent_draws_and_log_joint_match_full_execution_and_direct_density() {
    let compiled = regression(100);
    for theta in [-1.0, 0.0, 0.7, 2.0] {
        let base = fixed_trace(&[(3, theta)]);
        let (flat, trace) = score(compiled.inference_model(), base.clone());
        let (full, full_trace) = score(compiled.model().unwrap(), base);
        assert_eq!(flat.unwrap().len(), 1);
        assert_eq!(full.unwrap()[&3], ModelResult::Scalar(theta));
        assert_eq!(trace.total_log_weight(), full_trace.total_log_weight());
        let rows = &compiled.graph().plates[&100];
        let expected = Normal::new(0.0, 1.0).unwrap().log_prob(&theta)
            + rows.data["x"]
                .iter()
                .zip(&rows.data["y"])
                .map(|(&x, &y)| Normal::new(theta * x, 1.0).unwrap().log_prob(&y))
                .sum::<f64>();
        assert!((expected - trace.total_log_weight()).abs() < 1e-10);
    }
}

#[test]
fn prediction_recomputes_descendants_of_observed_random_nodes() {
    let mut graph = GraphIR::new();
    scalar(&mut graph, 1, 0.0);
    scalar(&mut graph, 2, 1.0);
    random(&mut graph, 3, "Normal", &[1, 2]);
    random(&mut graph, 4, "Normal", &[3, 2]);
    scalar(&mut graph, 5, 2.0);
    compute(&mut graph, 6, Operation::Multiply, &[4, 5]);
    random(&mut graph, 7, "Normal", &[6, 2]);
    let mut observed = plate(10, 1, vec![4, 6, 7]);
    observed.data = HashMap::from([("a".into(), vec![123.0]), ("b".into(), vec![246.0])]);
    observed.mapping = HashMap::from([(4, "a".into()), (7, "b".into())]);
    graph.plates.insert(10, observed);
    let compiled = graph.compile().unwrap();
    let mut base = fixed_trace(&[(3, 0.5)]);
    for (id, value) in [(4, 2.0), (7, 4.0)] {
        let addr = Address(format!("node#{id}/plate#10[0]"));
        base.choices.insert(
            addr.clone(),
            Choice {
                addr,
                value: ChoiceValue::F64(value),
                logp: 0.0,
            },
        );
    }
    let (flat, inference) = score(compiled.inference_model(), base.clone());
    assert_eq!(flat.unwrap().len(), 1);
    assert_eq!(inference.choices.len(), 1);
    let (values, predictive) = score(compiled.predictive_model().unwrap(), base);
    let values = values.unwrap();
    assert_eq!(values.len(), 3);
    assert_eq!(
        values[&4],
        ModelResult::Plate(vec![ModelResult::Scalar(2.0)])
    );
    let expected = Normal::new(0.0, 1.0).unwrap().log_prob(&0.5)
        + Normal::new(0.5, 1.0).unwrap().log_prob(&2.0)
        + Normal::new(4.0, 1.0).unwrap().log_prob(&4.0);
    assert_eq!(predictive.log_prior, expected);
}

#[test]
fn flat_multidimensional_latents_preserve_concrete_row_indices() {
    let mut graph = GraphIR::new();
    scalar(&mut graph, 1, 0.0);
    scalar(&mut graph, 2, 1.0);
    random(&mut graph, 3, "Normal", &[1, 2]);
    graph.plates.insert(10, plate(10, 2, vec![3]));
    graph.plates.insert(20, plate(20, 3, vec![3]));
    let compiled = graph.compile().unwrap();
    let result = compiled.run_inference(42, 4, 2).unwrap();
    assert_eq!(result.node_extents[&3], vec![2, 3]);
    let instances = result.samples_for_node(3).unwrap();
    assert_eq!(instances.len(), 6);
    assert_eq!(instances[5].indices, vec![1, 2]);
    for (index, instance) in instances.iter().enumerate() {
        for sample in &instance.samples {
            assert_eq!(
                sample.value,
                result.samples_by_node[&3][sample.draw_index][index]
            );
        }
    }
}

#[test]
fn dynamic_parameter_errors_keep_the_instance_address() {
    let mut graph = GraphIR::new();
    scalar(&mut graph, 1, -1.0);
    scalar(&mut graph, 2, 1.0);
    random(&mut graph, 3, "Uniform", &[1, 2]);
    compute(&mut graph, 4, Operation::Logarithm, &[3]);
    random(&mut graph, 5, "Normal", &[4, 2]);
    let compiled = graph.compile().unwrap();
    let (result, _) = score(compiled.inference_model(), fixed_trace(&[(3, -0.5)]));
    assert!(result.unwrap_err().contains("compute error at node#4"));
}

#[test]
fn all_supported_distributions_have_equivalent_prior_and_replay_scores() {
    for (name, params) in [
        ("Normal", vec![0.0, 1.0]),
        ("Uniform", vec![0.0, 1.0]),
        ("Beta", vec![2.0, 3.0]),
        ("Exponential", vec![2.0]),
        ("Gamma", vec![2.0, 3.0]),
        ("LogNormal", vec![0.0, 1.0]),
    ] {
        let mut graph = GraphIR::new();
        for (i, &value) in params.iter().enumerate() {
            scalar(&mut graph, i as u32, value);
        }
        random(
            &mut graph,
            10,
            name,
            &(0..params.len() as u32).collect::<Vec<_>>(),
        );
        let compiled = graph.compile().unwrap();
        let mut rng = StdRng::seed_from_u64(42);
        let (prior, trace) = fugue::runtime::handler::run(
            PriorHandler {
                rng: &mut rng,
                trace: Trace::default(),
            },
            compiled.inference_model(),
        );
        let (replayed, replay_trace) = score(compiled.inference_model(), trace.clone());
        assert_eq!(prior.unwrap(), replayed.unwrap());
        assert_eq!(
            trace.total_log_weight(),
            replay_trace.total_log_weight(),
            "{name}"
        );
    }
}

#[test]
#[ignore = "manual optimized inference throughput benchmark"]
fn compiled_regression_throughput() {
    for n in [100, 500, 1000] {
        let compiled = regression(n);
        let start = std::time::Instant::now();
        let result = compiled.run_inference(42, 1000, 200).unwrap();
        println!(
            "{n} observations: {:?} for {} samples + 200 warmup",
            start.elapsed(),
            result.n_samples
        );
    }
}
