use super::graph_checks::{ModelResult, ModelValues};
use super::model_compilation::CompiledGraph;
use fugue::{
    adaptive_single_site_mh, DiminishingAdaptation, PriorHandler, SafeReplayHandler, Trace,
};
use rand::{rngs::StdRng, SeedableRng};
use std::collections::{BTreeMap, HashMap};

/// All retained posterior executions and their Fugue traces.
///
/// Values stay in their original nested plate shape so future histogram views
/// can select a concrete row without rerunning inference or pooling data.
pub struct InferenceResult {
    pub seed: u64,
    pub n_samples: usize,
    pub n_warmup: usize,
    pub samples_by_node: HashMap<u32, Vec<ModelResult>>,
    pub traces: Vec<Trace>,
}

/// A completed or cooperatively cancelled inference run.
pub struct ControlledInferenceResult {
    pub result: InferenceResult,
    pub cancelled: bool,
}

/// One scalar posterior value and the retained draw that produced it.
///
/// `draw_index` is stable across nodes, so selections made in one variable's
/// histogram can be applied to every other variable from the same draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PosteriorSample {
    pub draw_index: usize,
    pub value: f64,
}

/// Posterior samples for one concrete scalar instance of a node.
///
/// Scalar nodes use an empty `indices` path. Plate values use paths such as
/// `[0]` or `[2, 4]`, preserving each concrete row independently.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeInstanceSamples {
    pub indices: Vec<usize>,
    pub samples: Vec<PosteriorSample>,
}

impl CompiledGraph {
    #[allow(dead_code)]
    pub fn run_inference(
        &self,
        seed: u64,
        n_samples: usize,
        n_warmup: usize,
    ) -> Result<InferenceResult, String> {
        Ok(self
            .run_inference_controlled(
                seed,
                n_samples,
                n_warmup,
                || false,
                |_| {},
                |_, _| {},
            )?
            .result)
    }

    /// Runs the same adaptive single-site chain as `adaptive_mcmc_chain`, while
    /// exposing safe boundaries for progress reporting and cancellation.
    ///
    /// Cancellation is checked between MH steps. Warmup values are never sent
    /// to `on_sample`; only retained posterior draws are published.
    pub fn run_inference_controlled(
        &self,
        seed: u64,
        n_samples: usize,
        n_warmup: usize,
        should_cancel: impl Fn() -> bool,
        mut on_warmup: impl FnMut(usize),
        mut on_sample: impl FnMut(usize, &ModelValues),
    ) -> Result<ControlledInferenceResult, String> {
        if n_samples == 0 {
            return Err("number of samples must be greater than zero".to_string());
        }

        // Surface deterministic model construction errors before starting.
        self.model()?;
        let model_fn = || {
            self.model()
                .expect("a validated compiled graph should always create a model")
        };
        let mut rng = StdRng::seed_from_u64(seed);
        let (_, mut current_trace) = fugue::runtime::handler::run(
            PriorHandler {
                rng: &mut rng,
                trace: Trace::default(),
            },
            model_fn(),
        );
        let mut adaptation = DiminishingAdaptation::new(0.44, 0.7);
        let mut completed_warmup = 0;

        for warmup_index in 0..n_warmup {
            if should_cancel() {
                return Ok(ControlledInferenceResult {
                    result: InferenceResult {
                        seed,
                        n_samples: 0,
                        n_warmup: completed_warmup,
                        samples_by_node: HashMap::new(),
                        traces: Vec::new(),
                    },
                    cancelled: true,
                });
            }
            let (_, trace) = adaptive_single_site_mh(
                &mut rng,
                &model_fn,
                &current_trace,
                &mut adaptation,
            );
            current_trace = trace;
            completed_warmup = warmup_index + 1;
            on_warmup(completed_warmup);
        }

        let mut samples_by_node = HashMap::<u32, Vec<ModelResult>>::new();
        let mut traces = Vec::with_capacity(n_samples);
        let mut cancelled = false;

        for draw_index in 0..n_samples {
            if should_cancel() {
                cancelled = true;
                break;
            }
            let (values, trace) = adaptive_single_site_mh(
                &mut rng,
                &model_fn,
                &current_trace,
                &mut adaptation,
            );
            current_trace = trace;
            let values: ModelValues = values.map_err(|error| {
                format!("inference execution {} failed: {error}", draw_index + 1)
            })?;
            for (&node_id, value) in &values {
                samples_by_node
                    .entry(node_id)
                    .or_default()
                    .push(value.clone());
            }
            traces.push(current_trace.clone());
            on_sample(draw_index, &values);
        }

        let retained = traces.len();
        if !cancelled {
            for &node_id in self.graph().nodes.keys() {
                let count = samples_by_node.get(&node_id).map_or(0, Vec::len);
                if count != n_samples {
                    return Err(format!(
                        "inference retained {count} of {n_samples} draws for node {node_id}"
                    ));
                }
            }
        }

        Ok(ControlledInferenceResult {
            result: InferenceResult {
                seed,
                n_samples: retained,
                n_warmup: completed_warmup,
                samples_by_node,
                traces,
            },
            cancelled,
        })
    }

    /// Replays one retained posterior trace while freshly sampling nodes that
    /// were observed during inference.
    pub fn posterior_predictive_sample(&self, posterior: &Trace) -> Result<ModelValues, String> {
        let model = self.predictive_model()?;
        let mut rng = rand::thread_rng();
        let (result, _) = fugue::runtime::handler::run(
            SafeReplayHandler {
                rng: &mut rng,
                base: posterior.clone(),
                trace: Trace::default(),
                warn_on_mismatch: true,
            },
            model,
        );
        result
    }
}

impl InferenceResult {
    /// Returns draw-indexed values independently for every concrete node instance.
    pub fn samples_for_node(&self, node_id: u32) -> Result<Vec<NodeInstanceSamples>, String> {
        let draws = self
            .samples_by_node
            .get(&node_id)
            .ok_or_else(|| format!("inference results do not contain node {node_id}"))?;
        let mut samples_by_instance = BTreeMap::<Vec<usize>, Vec<PosteriorSample>>::new();

        for (draw_index, draw) in draws.iter().enumerate() {
            flatten_result(draw, draw_index, &mut Vec::new(), &mut samples_by_instance);
        }

        samples_by_instance
            .into_iter()
            .map(|(indices, samples)| {
                if samples.is_empty() {
                    return Err("cannot display an empty posterior".to_string());
                }
                if samples.iter().any(|sample| !sample.value.is_finite()) {
                    return Err(format!(
                        "node instance {indices:?} contains non-finite values"
                    ));
                }
                Ok(NodeInstanceSamples { indices, samples })
            })
            .collect()
    }
}

fn flatten_result(
    value: &ModelResult,
    draw_index: usize,
    indices: &mut Vec<usize>,
    output: &mut BTreeMap<Vec<usize>, Vec<PosteriorSample>>,
) {
    match value {
        ModelResult::Scalar(value) => {
            output
                .entry(indices.clone())
                .or_default()
                .push(PosteriorSample {
                    draw_index,
                    value: *value,
                })
        }
        ModelResult::Plate(items) => {
            for (index, item) in items.iter().enumerate() {
                indices.push(index);
                flatten_result(item, draw_index, indices, output);
                indices.pop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bayesian_core::{GraphIR, NodeIR, ParamIR, PlateIR};
    use std::cell::Cell;

    fn simple_random_graph() -> CompiledGraph {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, NodeIR::Scalar { id: 1, value: 0.0 });
        graph.nodes.insert(2, NodeIR::Scalar { id: 2, value: 1.0 });
        graph.nodes.insert(
            3,
            NodeIR::Random {
                id: 3,
                label: Some("theta".to_string()),
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
                    ModelResult::Plate(vec![ModelResult::Scalar(1.0), ModelResult::Scalar(10.0)]),
                    ModelResult::Plate(vec![ModelResult::Scalar(2.0), ModelResult::Scalar(20.0)]),
                    ModelResult::Plate(vec![ModelResult::Scalar(3.0), ModelResult::Scalar(30.0)]),
                ],
            )]),
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
    fn inference_retains_every_nodes_complete_draws() {
        let compiled = simple_random_graph();
        let result = compiled.run_inference(42, 8, 2).unwrap();

        assert_eq!(result.traces.len(), 8);
        assert_eq!(result.samples_by_node[&1].len(), 8);
        assert_eq!(result.samples_by_node[&2].len(), 8);
        assert_eq!(result.samples_by_node[&3].len(), 8);
        assert_eq!(result.samples_for_node(3).unwrap()[0].samples.len(), 8);
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
    fn posterior_predictive_replays_latents_and_resamples_observations() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, NodeIR::Scalar { id: 1, value: 0.0 });
        graph.nodes.insert(2, NodeIR::Scalar { id: 2, value: 1.0 });
        graph.nodes.insert(
            3,
            NodeIR::Random {
                id: 3,
                label: Some("theta".to_string()),
                dist_type: "Normal".to_string(),
                params: vec![ParamIR { from_node: 1 }, ParamIR { from_node: 2 }],
            },
        );
        graph.nodes.insert(
            4,
            NodeIR::Random {
                id: 4,
                label: Some("y".to_string()),
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
                data: HashMap::from([("y".to_string(), vec![123.0])]),
                mapping: HashMap::from([(4, "y".to_string())]),
            },
        );

        let compiled = graph.compile().unwrap();
        let inference = compiled.run_inference(42, 1, 0).unwrap();
        let predictive = compiled
            .posterior_predictive_sample(&inference.traces[0])
            .unwrap();

        assert_eq!(predictive[&3], inference.samples_by_node[&3][0]);
        assert_ne!(
            predictive[&4],
            ModelResult::Plate(vec![ModelResult::Scalar(123.0)])
        );
    }
}
