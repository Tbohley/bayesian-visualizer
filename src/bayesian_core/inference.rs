#[cfg(test)]
use super::graph_checks::ModelResult;
#[cfg(test)]
use super::graph_checks::ModelValues;
use super::graph_checks::PosteriorValues;
use super::model_compilation::CompiledGraph;
use fugue::{
    Address, Choice, ChoiceValue, DiminishingAdaptation, Distribution, Handler, PriorHandler,
    Trace, adaptive_single_site_mh,
};
#[cfg(test)]
use fugue::{SafeReplayHandler, ScoreGivenTrace};
use rand::{SeedableRng, rngs::StdRng};
use std::collections::HashMap;
use std::sync::Arc;

/// Retained posterior draws and their shared plate dimensions.
///
/// Only latent random nodes are retained, in flat row-major arrays. Dimensions
/// are stored once; immutable draws are shared with live views without copying.
pub struct InferenceResult {
    pub seed: u64,
    pub n_samples: usize,
    pub n_warmup: usize,
    pub samples_by_node: HashMap<u32, Vec<Arc<[f64]>>>,
    pub node_extents: HashMap<u32, Vec<usize>>,
    pub node_shapes: HashMap<u32, super::NodeShape>,
    /// Full traces are used only by reference-execution tests.
    #[cfg(test)]
    pub traces: Vec<Trace>,
}

/// A completed or cooperatively cancelled inference run.
pub struct ControlledInferenceResult {
    pub result: InferenceResult,
    pub cancelled: bool,
}

/// A single observable boundary in a cooperatively driven inference run.
///
/// Each call to [`InferenceRunner::step`] advances a candidate, restarts one,
/// or returns a retained draw, allowing single-threaded hosts to return to
/// their event loop between transitions.
pub enum InferenceStep {
    Warmup {
        completed: usize,
    },
    WarmupComplete,
    Sample {
        draw_index: usize,
        values: PosteriorValues,
    },
    Complete(ControlledInferenceResult),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InferencePhase {
    Warmup,
    Sampling,
}

/// Stateful adaptive MCMC execution that can be advanced cooperatively.
pub struct InferenceRunner {
    compiled: CompiledGraph,
    chains: Vec<WarmupChain>,
    active_chain: usize,
    restarts: usize,
    seed: u64,
    requested_samples: usize,
    requested_warmup: usize,
    completed_warmup: usize,
    samples_by_node: HashMap<u32, Vec<Arc<[f64]>>>,
    completed_samples: usize,
    #[cfg(test)]
    traces: Vec<Trace>,
    phase: InferencePhase,
}

// Internal candidates provide initialization resilience, not combined-chain inference.
const NUM_CHAINS: usize = 3;
const MAX_RESTARTS: usize = 5 * NUM_CHAINS;
pub const WARMUP_FAILURE_WARNING: &str = "No chain completed warmup without a -infinity log probability. Check the model and observed data.";

struct WarmupChain {
    rng: StdRng,
    trace: Option<Trace>,
    adaptation: DiminishingAdaptation,
}
impl WarmupChain {
    fn new(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
            trace: None,
            adaptation: DiminishingAdaptation::new(0.44, 0.7),
        }
    }
}

fn has_negative_infinity(trace: &Trace) -> bool {
    [trace.log_prior, trace.log_likelihood, trace.log_factors].contains(&f64::NEG_INFINITY)
}

/// Checks each site's score before aggregation, without storing observations.
/// The marker in log_factors is used only to discard an invalid warmup state;
/// healthy traces keep their ordinary Fugue scores and sampled choices.
struct CheckedWarmupScore {
    base: Trace,
    trace: Trace,
    impossible: bool,
}
impl Handler for CheckedWarmupScore {
    fn on_sample_f64(&mut self, addr: &Address, dist: &dyn Distribution<f64>) -> f64 {
        let value = self
            .base
            .get_f64(addr)
            .expect("compiled warmup trace has every latent");
        let logp = dist.log_prob(&value);
        self.impossible |= logp == f64::NEG_INFINITY;
        self.trace.log_prior += logp;
        self.trace.choices.insert(
            addr.clone(),
            Choice {
                addr: addr.clone(),
                value: ChoiceValue::F64(value),
                logp,
            },
        );
        value
    }
    fn on_observe_f64(&mut self, _: &Address, dist: &dyn Distribution<f64>, value: f64) {
        let logp = dist.log_prob(&value);
        self.impossible |= logp == f64::NEG_INFINITY;
        self.trace.log_likelihood += logp;
    }
    fn on_factor(&mut self, logw: f64) {
        self.impossible |= logw == f64::NEG_INFINITY;
        self.trace.log_factors += logw;
    }
    fn on_sample_bool(&mut self, _: &Address, _: &dyn Distribution<bool>) -> bool {
        unreachable!("compiled graphs use f64")
    }
    fn on_sample_u64(&mut self, _: &Address, _: &dyn Distribution<u64>) -> u64 {
        unreachable!("compiled graphs use f64")
    }
    fn on_sample_usize(&mut self, _: &Address, _: &dyn Distribution<usize>) -> usize {
        unreachable!("compiled graphs use f64")
    }
    fn on_observe_bool(&mut self, _: &Address, _: &dyn Distribution<bool>, _: bool) {
        unreachable!("compiled graphs use f64")
    }
    fn on_observe_u64(&mut self, _: &Address, _: &dyn Distribution<u64>, _: u64) {
        unreachable!("compiled graphs use f64")
    }
    fn on_observe_usize(&mut self, _: &Address, _: &dyn Distribution<usize>, _: usize) {
        unreachable!("compiled graphs use f64")
    }
    fn finish(mut self) -> Trace {
        if self.impossible {
            self.trace.log_factors = f64::NEG_INFINITY;
        }
        self.trace
    }
}

fn score_warmup(compiled: &CompiledGraph, base: Trace) -> Result<Trace, String> {
    let (values, trace) = fugue::runtime::handler::run(
        CheckedWarmupScore {
            base,
            trace: Trace::default(),
            impossible: false,
        },
        compiled.inference_model(),
    );
    values.map_err(|error| format!("warmup scoring failed: {error}"))?;
    Ok(trace)
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
/// Unplated random nodes use an empty `indices` path. Plate values use paths such as
/// `[0]` or `[2, 4]`, preserving each concrete row independently.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeInstanceSamples {
    pub indices: Vec<usize>,
    pub samples: Vec<PosteriorSample>,
}

impl CompiledGraph {
    /// Starts an inference run without consuming the whole chain at once.
    pub fn inference_runner(
        self,
        seed: u64,
        n_samples: usize,
        n_warmup: usize,
    ) -> Result<InferenceRunner, String> {
        InferenceRunner::new(self, seed, n_samples, n_warmup)
    }

    #[cfg(test)]
    pub fn run_inference(
        &self,
        seed: u64,
        n_samples: usize,
        n_warmup: usize,
    ) -> Result<InferenceResult, String> {
        Ok(self
            .run_inference_controlled(seed, n_samples, n_warmup, || false, |_| {}, |_, _| {})?
            .result)
    }

    /// Warms up an internal candidate and restarts it on an impossible current
    /// state. Only the first successful candidate produces posterior draws.
    ///
    /// Cancellation is checked between MH steps. Warmup values are never sent
    /// to `on_sample`; only retained posterior draws are published.
    #[cfg(any(test, not(target_arch = "wasm32")))]
    pub fn run_inference_controlled(
        &self,
        seed: u64,
        n_samples: usize,
        n_warmup: usize,
        should_cancel: impl Fn() -> bool,
        mut on_warmup: impl FnMut(usize),
        mut on_sample: impl FnMut(usize, &PosteriorValues),
    ) -> Result<ControlledInferenceResult, String> {
        let mut runner = self.clone().inference_runner(seed, n_samples, n_warmup)?;
        loop {
            match runner.step(should_cancel())? {
                InferenceStep::Warmup { completed } => on_warmup(completed),
                InferenceStep::WarmupComplete => {}
                InferenceStep::Sample { draw_index, values } => on_sample(draw_index, &values),
                InferenceStep::Complete(outcome) => return Ok(outcome),
            }
        }
    }

    /// Replays one retained posterior trace while freshly sampling nodes that
    /// were observed during inference.
    #[cfg(test)]
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

impl InferenceRunner {
    fn new(
        compiled: CompiledGraph,
        seed: u64,
        requested_samples: usize,
        requested_warmup: usize,
    ) -> Result<Self, String> {
        if requested_samples == 0 {
            return Err("number of samples must be greater than zero".to_string());
        }

        let samples_by_node = compiled
            .posterior_node_extents()
            .keys()
            .map(|&id| (id, Vec::with_capacity(requested_samples)))
            .collect();
        Ok(Self {
            compiled,
            chains: (0..NUM_CHAINS)
                .map(|i| WarmupChain::new(seed.wrapping_add(i as u64)))
                .collect(),
            active_chain: 0,
            restarts: 0,
            seed,
            requested_samples,
            requested_warmup,
            completed_warmup: 0,
            samples_by_node,
            completed_samples: 0,
            #[cfg(test)]
            traces: Vec::with_capacity(requested_samples),
            phase: InferencePhase::Warmup,
        })
    }

    /// Advances one warmup transition, candidate restart, or retained draw.
    pub fn step(&mut self, cancel_requested: bool) -> Result<InferenceStep, String> {
        if cancel_requested {
            return Ok(self.finish(true));
        }

        if self.phase == InferencePhase::Warmup {
            return self.step_warmup();
        }

        if self.completed_samples == self.requested_samples {
            for &node_id in self.samples_by_node.keys() {
                let count = self.samples_by_node.get(&node_id).map_or(0, Vec::len);
                if count != self.requested_samples {
                    return Err(format!(
                        "inference retained {count} of {} draws for node {node_id}",
                        self.requested_samples
                    ));
                }
            }
            return Ok(self.finish(false));
        }

        let draw_index = self.completed_samples;
        let model_fn = || self.compiled.inference_model();
        let chain = &mut self.chains[0];
        let (values, trace) = adaptive_single_site_mh(
            &mut chain.rng,
            &model_fn,
            chain
                .trace
                .as_ref()
                .expect("selected chain has completed warmup"),
            &mut chain.adaptation,
        );
        chain.trace = Some(trace);
        let values: PosteriorValues = values
            .map_err(|error| format!("inference execution {} failed: {error}", draw_index + 1))?;
        for (&node_id, value) in &values {
            self.samples_by_node
                .entry(node_id)
                .or_default()
                .push(value.clone());
        }
        self.completed_samples += 1;
        #[cfg(test)]
        self.traces.push(chain.trace.as_ref().unwrap().clone());
        Ok(InferenceStep::Sample { draw_index, values })
    }

    fn step_warmup(&mut self) -> Result<InferenceStep, String> {
        let chain = &mut self.chains[self.active_chain];
        if chain.trace.is_none() {
            let (values, trace) = fugue::runtime::handler::run(
                PriorHandler {
                    rng: &mut chain.rng,
                    trace: Trace::default(),
                },
                self.compiled.inference_model(),
            );
            values.map_err(|error| format!("initial model execution failed: {error}"))?;
            chain.trace = Some(score_warmup(&self.compiled, trace)?);
        }
        // Prior scores and every stored warmup score are fresh. Inspect the
        // starting state even for zero warmup, before allowing MH to repair it.
        if has_negative_infinity(chain.trace.as_ref().unwrap()) {
            return self.restart_chain();
        }
        if self.completed_warmup == self.requested_warmup {
            let selected = self.chains.swap_remove(self.active_chain);
            self.chains.clear();
            self.chains.push(selected);
            self.active_chain = 0;
            self.phase = InferencePhase::Sampling;
            return Ok(InferenceStep::WarmupComplete);
        }
        let model_fn = || self.compiled.inference_model();
        let (values, selected) = adaptive_single_site_mh(
            &mut chain.rng,
            &model_fn,
            chain.trace.as_ref().unwrap(),
            &mut chain.adaptation,
        );
        values.map_err(|error| {
            format!(
                "warmup execution {} failed: {error}",
                self.completed_warmup + 1
            )
        })?;
        // Fugue 0.1 returns stale aggregate scores on its selected trace.
        // Rescore ONLY the returned (accepted/current) state: a proposal that
        // was rejected must not discard an otherwise healthy chain.
        let scored = score_warmup(&self.compiled, selected)?;
        chain.trace = Some(scored);
        if has_negative_infinity(chain.trace.as_ref().unwrap()) {
            return self.restart_chain();
        }
        self.completed_warmup += 1;
        Ok(InferenceStep::Warmup {
            completed: self.completed_warmup,
        })
    }

    fn restart_chain(&mut self) -> Result<InferenceStep, String> {
        self.restarts += 1;
        self.completed_warmup = 0;
        if self.restarts > MAX_RESTARTS {
            return Err(WARMUP_FAILURE_WARNING.to_string());
        }
        let seed = self
            .seed
            .wrapping_add((NUM_CHAINS + self.restarts - 1) as u64);
        self.chains[self.active_chain] = WarmupChain::new(seed);
        self.active_chain = (self.active_chain + 1) % NUM_CHAINS;
        Ok(InferenceStep::Warmup { completed: 0 })
    }

    fn finish(&mut self, cancelled: bool) -> InferenceStep {
        let retained = self.completed_samples;
        InferenceStep::Complete(ControlledInferenceResult {
            result: InferenceResult {
                seed: self.seed,
                n_samples: retained,
                n_warmup: self.completed_warmup,
                samples_by_node: std::mem::take(&mut self.samples_by_node),
                node_extents: self.compiled.posterior_node_extents(),
                node_shapes: self.compiled.node_shapes().clone(),
                #[cfg(test)]
                traces: std::mem::take(&mut self.traces),
            },
            cancelled,
        })
    }
}

impl InferenceResult {
    /// Returns draw-indexed values independently for every concrete node instance.
    pub fn samples_for_node(&self, node_id: u32) -> Result<Vec<NodeInstanceSamples>, String> {
        let draws = self
            .samples_by_node
            .get(&node_id)
            .ok_or_else(|| format!("inference results do not contain node {node_id}"))?;
        let extents = self.node_extents.get(&node_id).ok_or_else(|| {
            format!("inference results do not contain dimensions for node {node_id}")
        })?;
        let count = extents.iter().product::<usize>();
        if draws.is_empty() {
            return Err("cannot display an empty posterior".into());
        }
        if draws
            .iter()
            .any(|draw| draw.len() != count || draw.iter().any(|v| !v.is_finite()))
        {
            return Err(format!(
                "node {node_id} contains invalid or non-finite posterior values"
            ));
        }
        Ok((0..count)
            .map(|index| NodeInstanceSamples {
                indices: super::execution::unflatten(index, extents),
                samples: draws
                    .iter()
                    .enumerate()
                    .map(|(draw_index, draw)| PosteriorSample {
                        draw_index,
                        value: draw[index],
                    })
                    .collect(),
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "tests/inference.rs"]
mod tests;
