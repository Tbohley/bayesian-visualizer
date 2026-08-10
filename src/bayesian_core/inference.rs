use super::graph_checks::{ModelResult, ModelValues};
use super::model_compilation::CompiledGraph;
use fugue::{adaptive_mcmc_chain, SafeReplayHandler, Trace};
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

impl NodeInstanceSamples {
    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn mean(&self) -> f64 {
        self.samples.iter().map(|sample| sample.value).sum::<f64>() / self.count() as f64
    }

    pub fn standard_deviation(&self) -> f64 {
        if self.count() <= 1 {
            return 0.0;
        }
        let mean = self.mean();
        let variance = self
            .samples
            .iter()
            .map(|sample| (sample.value - mean).powi(2))
            .sum::<f64>()
            / (self.count() - 1) as f64;
        variance.sqrt()
    }

    pub fn median(&self) -> f64 {
        self.quantile(0.5)
    }

    pub fn lower_95(&self) -> f64 {
        self.quantile(0.025)
    }

    pub fn upper_95(&self) -> f64 {
        self.quantile(0.975)
    }

    fn quantile(&self, probability: f64) -> f64 {
        let mut values = self
            .samples
            .iter()
            .map(|sample| sample.value)
            .collect::<Vec<_>>();
        values.sort_by(f64::total_cmp);
        quantile(&values, probability)
    }
}

impl CompiledGraph {
    pub fn run_inference(
        &self,
        seed: u64,
        n_samples: usize,
        n_warmup: usize,
    ) -> Result<InferenceResult, String> {
        if n_samples == 0 {
            return Err("number of samples must be greater than zero".to_string());
        }

        // Check the factory once before entering Fugue's infallible callback.
        self.model()?;
        let mut rng = StdRng::seed_from_u64(seed);
        let draws = adaptive_mcmc_chain(
            &mut rng,
            || {
                self.model()
                    .expect("a validated compiled graph should always create a model")
            },
            n_samples,
            n_warmup,
        );

        let mut samples_by_node = HashMap::<u32, Vec<ModelResult>>::new();
        let mut traces = Vec::with_capacity(draws.len());

        for (draw_index, (values, trace)) in draws.into_iter().enumerate() {
            let values: ModelValues = values.map_err(|error| {
                format!("inference execution {} failed: {error}", draw_index + 1)
            })?;
            for (&node_id, value) in &values {
                samples_by_node
                    .entry(node_id)
                    .or_default()
                    .push(value.clone());
            }
            traces.push(trace);
        }

        for &node_id in self.graph().nodes.keys() {
            let count = samples_by_node.get(&node_id).map_or(0, Vec::len);
            if count != n_samples {
                return Err(format!(
                    "inference retained {count} of {n_samples} draws for node {node_id}"
                ));
            }
        }

        Ok(InferenceResult {
            seed,
            n_samples,
            n_warmup,
            samples_by_node,
            traces,
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
            flatten_result(
                draw,
                draw_index,
                &mut Vec::new(),
                &mut samples_by_instance,
            );
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
        ModelResult::Scalar(value) => output
            .entry(indices.clone())
            .or_default()
            .push(PosteriorSample {
                draw_index,
                value: *value,
            }),
        ModelResult::Plate(items) => {
            for (index, item) in items.iter().enumerate() {
                indices.push(index);
                flatten_result(item, draw_index, indices, output);
                indices.pop();
            }
        }
    }
}

fn quantile(sorted: &[f64], probability: f64) -> f64 {
    let position = probability * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let fraction = position - lower as f64;
    sorted[lower] + (sorted[upper] - sorted[lower]) * fraction
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bayesian_core::{GraphIR, NodeIR, ParamIR, PlateIR};

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
        assert_eq!(instances[0].mean(), 2.0);
        assert_eq!(instances[0].samples[1].draw_index, 1);
        assert_eq!(instances[0].samples[1].value, 2.0);
        assert_eq!(instances[1].indices, vec![1]);
        assert_eq!(instances[1].mean(), 20.0);
    }

    #[test]
    fn inference_retains_every_nodes_complete_draws() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, NodeIR::Scalar { id: 1, value: 0.0 });
        graph.nodes.insert(2, NodeIR::Scalar { id: 2, value: 1.0 });
        graph.nodes.insert(
            3,
            NodeIR::Random {
                id: 3,
                label: Some("theta".to_string()),
                dist_type: "Normal".to_string(),
                params: vec![
                    ParamIR {
                        from_node: 1,
                        param_name: None,
                    },
                    ParamIR {
                        from_node: 2,
                        param_name: None,
                    },
                ],
            },
        );

        let compiled = graph.compile().unwrap();
        let result = compiled.run_inference(42, 8, 2).unwrap();

        assert_eq!(result.traces.len(), 8);
        assert_eq!(result.samples_by_node[&1].len(), 8);
        assert_eq!(result.samples_by_node[&2].len(), 8);
        assert_eq!(result.samples_by_node[&3].len(), 8);
        assert_eq!(result.samples_for_node(3).unwrap()[0].count(), 8);
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
                params: vec![
                    ParamIR {
                        from_node: 1,
                        param_name: None,
                    },
                    ParamIR {
                        from_node: 2,
                        param_name: None,
                    },
                ],
            },
        );
        graph.nodes.insert(
            4,
            NodeIR::Random {
                id: 4,
                label: Some("y".to_string()),
                dist_type: "Normal".to_string(),
                params: vec![
                    ParamIR {
                        from_node: 3,
                        param_name: None,
                    },
                    ParamIR {
                        from_node: 2,
                        param_name: None,
                    },
                ],
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
