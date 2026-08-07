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

#[derive(Clone, Debug, PartialEq)]
pub struct NodeInstanceSummary {
    pub indices: Vec<usize>,
    pub count: usize,
    pub mean: f64,
    pub standard_deviation: f64,
    pub median: f64,
    pub lower_95: f64,
    pub upper_95: f64,
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
    /// Computes summaries independently for every concrete plate-row instance.
    pub fn summaries_for_node(&self, node_id: u32) -> Result<Vec<NodeInstanceSummary>, String> {
        let draws = self
            .samples_by_node
            .get(&node_id)
            .ok_or_else(|| format!("inference results do not contain node {node_id}"))?;
        let mut values_by_instance = BTreeMap::<Vec<usize>, Vec<f64>>::new();

        for draw in draws {
            flatten_result(draw, &mut Vec::new(), &mut values_by_instance);
        }

        values_by_instance
            .into_iter()
            .map(|(indices, values)| summarize(indices, values))
            .collect()
    }
}

fn flatten_result(
    value: &ModelResult,
    indices: &mut Vec<usize>,
    output: &mut BTreeMap<Vec<usize>, Vec<f64>>,
) {
    match value {
        ModelResult::Scalar(value) => output.entry(indices.clone()).or_default().push(*value),
        ModelResult::Plate(items) => {
            for (index, item) in items.iter().enumerate() {
                indices.push(index);
                flatten_result(item, indices, output);
                indices.pop();
            }
        }
    }
}

fn summarize(indices: Vec<usize>, mut values: Vec<f64>) -> Result<NodeInstanceSummary, String> {
    if values.is_empty() {
        return Err("cannot summarize an empty posterior".to_string());
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(format!(
            "node instance {indices:?} contains non-finite values"
        ));
    }

    values.sort_by(f64::total_cmp);
    let count = values.len();
    let mean = values.iter().sum::<f64>() / count as f64;
    let variance = if count > 1 {
        values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / (count - 1) as f64
    } else {
        0.0
    };

    Ok(NodeInstanceSummary {
        indices,
        count,
        mean,
        standard_deviation: variance.sqrt(),
        median: quantile(&values, 0.5),
        lower_95: quantile(&values, 0.025),
        upper_95: quantile(&values, 0.975),
    })
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
    fn summaries_preserve_plate_rows() {
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

        let summaries = result.summaries_for_node(7).unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].indices, vec![0]);
        assert_eq!(summaries[0].mean, 2.0);
        assert_eq!(summaries[1].indices, vec![1]);
        assert_eq!(summaries[1].mean, 20.0);
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
        assert_eq!(result.summaries_for_node(3).unwrap()[0].count, 8);
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
