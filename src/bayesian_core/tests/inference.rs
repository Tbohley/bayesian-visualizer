use super::*;
use crate::bayesian_core::{GraphIR, NodeIR, ParamIR, PlateIR};
use std::cell::Cell;

fn simple_random_graph() -> CompiledGraph {
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
    graph.compile().unwrap()
}

#[test]
fn samples_preserve_plate_rows_and_draw_indices() {
    let result = InferenceResult {
        seed: 1,
        n_samples: 3,
        n_warmup: 0,
        samples_by_node: HashMap::from([(
            7,
            vec![
                Arc::from([1.0, 10.0]),
                Arc::from([2.0, 20.0]),
                Arc::from([3.0, 30.0]),
            ],
        )]),
        node_extents: HashMap::from([(7, vec![2])]),
        node_shapes: HashMap::new(),
        traces: Vec::new(),
    };

    let instances = result.samples_for_node(7).unwrap();
    assert_eq!(instances.len(), 2);
    assert_eq!(instances[0].indices, vec![0]);
    assert_eq!(
        instances[0]
            .samples
            .iter()
            .map(|sample| sample.value)
            .collect::<Vec<_>>(),
        vec![1.0, 2.0, 3.0]
    );
    assert_eq!(instances[0].samples[1].draw_index, 1);
    assert_eq!(instances[0].samples[1].value, 2.0);
    assert_eq!(instances[1].indices, vec![1]);
    assert_eq!(instances[1].samples[1].value, 20.0);
}

#[test]
fn inference_retains_only_latent_random_nodes() {
    let compiled = simple_random_graph();
    let result = compiled.run_inference(42, 8, 2).unwrap();

    assert_eq!(result.traces.len(), 8);
    assert!(!result.samples_by_node.contains_key(&1));
    assert!(!result.samples_by_node.contains_key(&2));
    assert_eq!(result.samples_by_node[&3].len(), 8);
    assert_eq!(result.samples_for_node(3).unwrap()[0].samples.len(), 8);
}

#[test]
fn published_flat_draws_share_storage_with_retained_history() {
    let published = std::cell::RefCell::new(Vec::new());
    let outcome = simple_random_graph()
        .run_inference_controlled(
            42,
            4,
            1,
            || false,
            |_| {},
            |_, values| published.borrow_mut().push(Arc::clone(&values[&3])),
        )
        .unwrap();
    for (published, retained) in published
        .borrow()
        .iter()
        .zip(&outcome.result.samples_by_node[&3])
    {
        assert!(Arc::ptr_eq(published, retained));
    }
}

#[test]
fn incremental_runner_stops_at_every_cooperative_boundary() {
    let mut runner = simple_random_graph().inference_runner(42, 2, 2).unwrap();

    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::Warmup { completed: 1 }
    ));
    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::Warmup { completed: 2 }
    ));
    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::WarmupComplete
    ));
    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::Sample { draw_index: 0, .. }
    ));
    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::Sample { draw_index: 1, .. }
    ));
    let InferenceStep::Complete(outcome) = runner.step(false).unwrap() else {
        panic!("runner should complete after the requested draws");
    };
    assert!(!outcome.cancelled);
    assert_eq!(outcome.result.n_warmup, 2);
    assert_eq!(outcome.result.n_samples, 2);
}

#[test]
fn controlled_inference_cancels_during_warmup_without_retaining_draws() {
    let compiled = simple_random_graph();
    let cancel = Cell::new(false);
    let published = Cell::new(0);
    let outcome = compiled
        .run_inference_controlled(
            42,
            8,
            5,
            || cancel.get(),
            |completed| {
                if completed == 2 {
                    cancel.set(true);
                }
            },
            |_, _| published.set(published.get() + 1),
        )
        .unwrap();

    assert!(outcome.cancelled);
    assert_eq!(outcome.result.n_warmup, 2);
    assert_eq!(outcome.result.n_samples, 0);
    assert_eq!(published.get(), 0);
}

#[test]
fn controlled_inference_keeps_draws_retained_before_cancellation() {
    let compiled = simple_random_graph();
    let cancel = Cell::new(false);
    let published = Cell::new(0);
    let outcome = compiled
        .run_inference_controlled(
            42,
            8,
            2,
            || cancel.get(),
            |_| {},
            |draw_index, _| {
                published.set(published.get() + 1);
                if draw_index == 2 {
                    cancel.set(true);
                }
            },
        )
        .unwrap();

    assert!(outcome.cancelled);
    assert_eq!(outcome.result.n_warmup, 2);
    assert_eq!(outcome.result.n_samples, 3);
    assert_eq!(outcome.result.traces.len(), 3);
    assert_eq!(outcome.result.samples_by_node[&3].len(), 3);
    assert_eq!(published.get(), 3);
}

#[test]
fn detects_negative_infinity_in_any_score_component() {
    let finite = Trace {
        log_prior: -2.0,
        log_likelihood: -3.0,
        ..Default::default()
    };
    assert!(!has_negative_infinity(&finite));
    for component in 0..3 {
        let mut trace = finite.clone();
        match component {
            0 => trace.log_prior = f64::NEG_INFINITY,
            1 => trace.log_likelihood = f64::NEG_INFINITY,
            _ => trace.log_factors = f64::NEG_INFINITY,
        }
        assert!(has_negative_infinity(&trace));
    }
}

#[test]
fn an_individual_impossible_score_cannot_be_hidden_by_positive_infinity() {
    #[derive(Clone)]
    struct FixedScore(f64);
    impl Distribution<f64> for FixedScore {
        fn sample(&self, _: &mut dyn rand::RngCore) -> f64 {
            0.0
        }
        fn log_prob(&self, _: &f64) -> f64 {
            self.0
        }
        fn clone_box(&self) -> Box<dyn Distribution<f64>> {
            Box::new(self.clone())
        }
    }
    let mut handler = CheckedWarmupScore {
        base: Trace::default(),
        trace: Trace::default(),
        impossible: false,
    };
    let address = Address("observed".into());
    handler.on_observe_f64(&address, &FixedScore(f64::INFINITY), 0.0);
    handler.on_observe_f64(&address, &FixedScore(f64::NEG_INFINITY), 0.0);
    let trace = handler.finish();
    assert!(trace.log_likelihood.is_nan());
    assert!(has_negative_infinity(&trace));
}

#[test]
fn impossible_observations_stop_after_the_restart_budget() {
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
    graph.nodes.insert(
        4,
        NodeIR::Random {
            dist_type: "Uniform".to_string(),
            params: vec![ParamIR { from_node: 1 }, ParamIR { from_node: 2 }],
        },
    );
    graph.plates.insert(
        10,
        PlateIR {
            id: 10,
            n: 1,
            nodes: vec![4],
            plates: Vec::new(),
            data: HashMap::from([("observed".to_string(), vec![2.0])]),
            mapping: HashMap::from([(4, "observed".to_string())]),
        },
    );

    let mut runner = graph.compile().unwrap().inference_runner(42, 1, 1).unwrap();
    for restart in 1..=MAX_RESTARTS {
        assert!(matches!(
            runner.step(false).unwrap(),
            InferenceStep::Warmup { completed: 0 }
        ));
        assert_eq!(runner.restarts, restart);
        assert!(runner.traces.is_empty());
    }
    assert_eq!(
        runner.step(false).err().as_deref(),
        Some(WARMUP_FAILURE_WARNING)
    );
    assert_eq!(runner.restarts, MAX_RESTARTS + 1);
    assert!(runner.traces.is_empty());
}

fn bounded_likelihood_graph() -> CompiledGraph {
    let mut graph = GraphIR::new();
    for (id, value) in [(1, -2.0), (2, 2.0), (4, 1.0)] {
        graph.nodes.insert(id, NodeIR::Scalar { value });
    }
    graph.nodes.insert(
        3,
        NodeIR::Random {
            dist_type: "Uniform".into(),
            params: vec![ParamIR { from_node: 1 }, ParamIR { from_node: 2 }],
        },
    );
    graph.nodes.insert(
        5,
        NodeIR::Compute {
            operation: crate::nodes::Operation::Add,
            params: vec![ParamIR { from_node: 3 }, ParamIR { from_node: 4 }],
        },
    );
    graph.nodes.insert(
        6,
        NodeIR::Random {
            dist_type: "Uniform".into(),
            params: vec![ParamIR { from_node: 3 }, ParamIR { from_node: 5 }],
        },
    );
    graph.plates.insert(
        10,
        PlateIR {
            id: 10,
            n: 1,
            nodes: vec![6],
            plates: vec![],
            data: HashMap::from([("y".into(), vec![0.0])]),
            mapping: HashMap::from([(6, "y".into())]),
        },
    );
    graph.compile().unwrap()
}

fn prior_is_valid(compiled: &CompiledGraph, seed: u64) -> bool {
    let mut rng = StdRng::seed_from_u64(seed);
    let (_, trace) = fugue::runtime::handler::run(
        PriorHandler {
            rng: &mut rng,
            trace: Trace::default(),
        },
        compiled.inference_model(),
    );
    !has_negative_infinity(&trace)
}

fn seed_with_bad_then_good_initialization(compiled: &CompiledGraph) -> u64 {
    (0..100)
        .find(|&seed| !prior_is_valid(compiled, seed) && prior_is_valid(compiled, seed + 1))
        .unwrap()
}

#[test]
fn a_bad_initialization_is_silently_replaced_and_only_one_chain_is_retained() {
    let compiled = bounded_likelihood_graph();
    let seed = seed_with_bad_then_good_initialization(&compiled);
    let mut runner = compiled.clone().inference_runner(seed, 4, 5).unwrap();
    assert_eq!(runner.chains.len(), NUM_CHAINS);
    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::Warmup { completed: 0 }
    ));
    assert_eq!(runner.restarts, 1);
    assert_eq!(runner.active_chain, 1);
    let result = loop {
        if let InferenceStep::Complete(outcome) = runner.step(false).unwrap() {
            break outcome.result;
        }
    };
    assert_eq!(runner.chains.len(), 1);
    assert_eq!(runner.restarts, 1);
    assert_eq!(result.n_warmup, 5);
    assert_eq!(result.n_samples, 4);
    for trace in result.traces {
        let (_, scored) = fugue::runtime::handler::run(
            ScoreGivenTrace {
                base: trace,
                trace: Trace::default(),
            },
            compiled.inference_model(),
        );
        assert!(scored.total_log_weight().is_finite());
    }
    let repeated = compiled.run_inference(seed, 4, 5).unwrap();
    assert_eq!(result.samples_by_node, repeated.samples_by_node);
}

#[test]
fn an_impossible_current_state_during_warmup_resets_all_warmup_progress() {
    let compiled = bounded_likelihood_graph();
    let seed = (0..100)
        .find(|&seed| prior_is_valid(&compiled, seed) && prior_is_valid(&compiled, seed + 1))
        .unwrap();
    let mut runner = compiled.inference_runner(seed, 1, 3).unwrap();
    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::Warmup { completed: 1 }
    ));
    // Inject a genuinely impossible current state after warmup has started.
    let trace = runner.chains[0].trace.as_mut().unwrap();
    trace
        .choices
        .get_mut(&fugue::Address("node#3".into()))
        .unwrap()
        .value = fugue::ChoiceValue::F64(1.5);
    trace.log_likelihood = f64::NEG_INFINITY;
    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::Warmup { completed: 0 }
    ));
    assert_eq!(runner.restarts, 1);
    assert!(runner.chains[0].trace.is_none());
    assert!(runner.chains[0].adaptation.total_counts.is_empty());
    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::Warmup { completed: 1 }
    ));
}

#[test]
fn impossible_rejected_proposals_do_not_restart_a_healthy_chain() {
    let mut graph = GraphIR::new();
    graph.nodes.insert(1, NodeIR::Scalar { value: -1.0 });
    graph.nodes.insert(2, NodeIR::Scalar { value: 1.0 });
    graph.nodes.insert(
        3,
        NodeIR::Random {
            dist_type: "Uniform".into(),
            params: vec![ParamIR { from_node: 1 }, ParamIR { from_node: 2 }],
        },
    );
    let mut runner = graph
        .compile()
        .unwrap()
        .inference_runner(42, 1, 100)
        .unwrap();
    for step in 1..=100 {
        if step > 1 {
            runner.chains[0]
                .adaptation
                .scales
                .insert(fugue::Address("node#3".into()), (100.0, 100.0_f64.ln()));
        }
        assert!(
            matches!(runner.step(false).unwrap(), InferenceStep::Warmup { completed } if completed == step)
        );
        assert_eq!(runner.restarts, 0);
    }
    let adaptation = &runner.chains[0].adaptation;
    assert!(
        adaptation.accept_counts.values().sum::<usize>()
            < adaptation.total_counts.values().sum::<usize>()
    );
    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::WarmupComplete
    ));
}

#[test]
fn zero_warmup_still_rejects_an_impossible_initial_state() {
    let compiled = bounded_likelihood_graph();
    let seed = seed_with_bad_then_good_initialization(&compiled);
    let mut runner = compiled.inference_runner(seed, 1, 0).unwrap();
    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::Warmup { completed: 0 }
    ));
    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::WarmupComplete
    ));
    assert_eq!(runner.restarts, 1);
    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::Sample { draw_index: 0, .. }
    ));
}

#[test]
fn cancellation_between_restarts_retains_no_failed_chain_draws() {
    let compiled = bounded_likelihood_graph();
    let seed = seed_with_bad_then_good_initialization(&compiled);
    let mut runner = compiled.inference_runner(seed, 5, 5).unwrap();
    assert!(matches!(
        runner.step(false).unwrap(),
        InferenceStep::Warmup { completed: 0 }
    ));
    let InferenceStep::Complete(outcome) = runner.step(true).unwrap() else {
        panic!("expected cancellation");
    };
    assert!(outcome.cancelled);
    assert_eq!(outcome.result.n_samples, 0);
    assert!(outcome.result.traces.is_empty());
}

#[test]
fn posterior_predictive_replays_latents_and_resamples_observations() {
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
    graph.nodes.insert(
        4,
        NodeIR::Random {
            dist_type: "Normal".to_string(),
            params: vec![ParamIR { from_node: 3 }, ParamIR { from_node: 2 }],
        },
    );
    graph.plates.insert(
        10,
        PlateIR {
            id: 10,
            n: 1,
            nodes: vec![4],
            plates: Vec::new(),
            data: HashMap::from([("y".to_string(), vec![12.0])]),
            mapping: HashMap::from([(4, "y".to_string())]),
        },
    );

    let compiled = graph.compile().unwrap();
    let inference = compiled.run_inference(42, 1, 0).unwrap();
    let predictive = compiled
        .posterior_predictive_sample(&inference.traces[0])
        .unwrap();

    assert_eq!(
        predictive[&3],
        ModelResult::Scalar(inference.samples_by_node[&3][0][0])
    );
    assert!(!inference.samples_by_node.contains_key(&4));
    assert_ne!(
        predictive[&4],
        ModelResult::Plate(vec![ModelResult::Scalar(12.0)])
    );
}
