//! A compiled instruction plan. Deterministic work runs in a loop; only a
//! probabilistic effect suspends execution and owns a continuation.
use super::graph_checks::{ModelResult, ModelValues, PosteriorValues};
use super::plate_validation::{NormalizedPlates, projection_positions};
use super::{GraphIR, NodeIR, ParamIR};
use crate::nodes::Operation;
use fugue::{
    Address, Beta, Distribution, Exponential, Gamma, LogNormal, Model, Normal, Uniform, pure,
};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(super) enum Mode {
    Inference,
    Predictive,
}

#[derive(Clone, Copy, Debug)]
enum Source {
    Constant(f64),
    Slot(usize),
}
impl Source {
    fn read(self, values: &[f64]) -> f64 {
        match self {
            Self::Constant(value) => value,
            Self::Slot(slot) => values[slot],
        }
    }
}

#[derive(Clone, Copy)]
struct Arguments {
    values: [Source; 2],
    len: usize,
}
impl Arguments {
    fn resolve(self, slots: &[f64]) -> [f64; 2] {
        self.values.map(|source| source.read(slots))
    }
    fn constant(self) -> bool {
        self.values[..self.len]
            .iter()
            .all(|source| matches!(source, Source::Constant(_)))
    }
}

#[derive(Clone, Copy)]
enum DistributionKind {
    Normal,
    Uniform,
    Beta,
    Exponential,
    Gamma,
    LogNormal,
}
impl DistributionKind {
    fn parse(name: &str, arity: usize) -> Result<Self, String> {
        let kind = match (name, arity) {
            ("Normal", 2) => Self::Normal,
            ("Uniform", 2) => Self::Uniform,
            ("Beta", 2) => Self::Beta,
            ("Exponential", 1) => Self::Exponential,
            ("Gamma", 2) => Self::Gamma,
            ("LogNormal", 2) => Self::LogNormal,
            _ => {
                return Err(format!(
                    "wrong number of parameters or unsupported distribution {name}"
                ));
            }
        };
        Ok(kind)
    }
    fn name(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Uniform => "Uniform",
            Self::Beta => "Beta",
            Self::Exponential => "Exponential",
            Self::Gamma => "Gamma",
            Self::LogNormal => "LogNormal",
        }
    }
    fn create(self, p: [f64; 2]) -> Result<Box<dyn Distribution<f64>>, String> {
        macro_rules! boxed {
            ($e:expr) => {
                $e.map(|d| Box::new(d) as Box<dyn Distribution<f64>>)
                    .map_err(|e| e.to_string())
            };
        }
        match self {
            Self::Normal => boxed!(Normal::new(p[0], p[1])),
            Self::Uniform => boxed!(Uniform::new(p[0], p[1])),
            Self::Beta => boxed!(Beta::new(p[0], p[1])),
            Self::Exponential => boxed!(Exponential::new(p[0])),
            Self::Gamma => boxed!(Gamma::new(p[0], p[1])),
            Self::LogNormal => boxed!(LogNormal::new(p[0], p[1])),
        }
    }
}

enum DistributionSpec {
    Constant {
        distribution: Box<dyn Distribution<f64>>,
        parameters: [f64; 2],
        len: usize,
    },
    Dynamic(DistributionKind, Arguments),
}
impl DistributionSpec {
    fn create(&self, slots: &[f64]) -> Result<Box<dyn Distribution<f64>>, String> {
        match self {
            Self::Constant { distribution, .. } => Ok(distribution.clone_box()),
            Self::Dynamic(kind, args) => kind.create(args.resolve(slots)),
        }
    }
}

enum Instruction {
    Compute {
        operation: Operation,
        args: Arguments,
        output: usize,
        address: String,
    },
    Random {
        distribution: DistributionSpec,
        kind: DistributionKind,
        address: Address,
        output: Option<usize>,
        observed: Option<f64>,
    },
}

struct NodeOutput {
    node_id: u32,
    extents: Vec<usize>,
    sources: Vec<Source>,
    latent: bool,
}

pub(super) struct ExecutionPlan {
    instructions: Vec<Instruction>,
    slot_count: usize,
    outputs: Vec<NodeOutput>,
}

impl ExecutionPlan {
    /// `all_nodes` is reserved for one-off full graph inspection. Inference
    /// and prediction keep all random sites and only their deterministic ancestors.
    pub(super) fn compile(
        graph: &GraphIR,
        normalized: &NormalizedPlates,
        order: &[u32],
        mode: Mode,
        all_nodes: bool,
    ) -> Result<Self, String> {
        let mut needed = HashSet::new();
        for &id in order.iter().rev() {
            if all_nodes
                || matches!(graph.nodes[&id], NodeIR::Random { .. })
                || needed.contains(&id)
            {
                needed.insert(id);
                for param in node_params(&graph.nodes[&id]) {
                    needed.insert(param.from_node);
                }
            }
        }
        let mut plan = Self {
            instructions: Vec::new(),
            slot_count: 0,
            outputs: Vec::new(),
        };
        let mut sources: HashMap<u32, Vec<Source>> = HashMap::new();
        for &id in order {
            if !needed.contains(&id) {
                continue;
            }
            let node = &graph.nodes[&id];
            let dimensions = &normalized.node_paths[&id];
            let extents: Vec<usize> = dimensions.iter().map(|p| normalized.extents[p]).collect();
            let count = extents
                .iter()
                .try_fold(1usize, |count, extent| count.checked_mul(*extent))
                .ok_or_else(|| format!("plate instance count overflows for node {id}"))?;
            let params = node_params(node);
            if params.len() > 2 {
                return Err(format!("too many parameters for node {id}"));
            }
            let positions = params
                .iter()
                .map(|p| projection_positions(&normalized.node_paths[&p.from_node], dimensions))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| format!("incompatible plate dimensions at node {id}"))?;
            let mut node_sources = Vec::with_capacity(count);
            for flat_index in 0..count {
                let indices = unflatten(flat_index, &extents);
                let mut args = Arguments {
                    values: [Source::Constant(0.0); 2],
                    len: params.len(),
                };
                for (i, param) in params.iter().enumerate() {
                    let producer_dims = &normalized.node_paths[&param.from_node];
                    let producer_index =
                        positions[i]
                            .iter()
                            .enumerate()
                            .fold(0, |offset, (axis, &position)| {
                                offset * normalized.extents[&producer_dims[axis]]
                                    + indices[position]
                            });
                    args.values[i] = sources[&param.from_node][producer_index];
                }
                let observed = if let Some(owner) = normalized.observation_owners.get(&id) {
                    let plate = &graph.plates[owner];
                    let axis = dimensions
                        .binary_search(owner)
                        .expect("validated observation dimension");
                    Some(plate.data[&plate.mapping[&id]][indices[axis]])
                } else {
                    None
                };
                let source = match node {
                    NodeIR::Scalar { value, .. } => Source::Constant(observed.unwrap_or(*value)),
                    NodeIR::Compute { operation, .. } => {
                        let address =
                            super::model_compilation::instance_address(id, dimensions, &indices);
                        if args.constant() {
                            let p = args.resolve(&[]);
                            Source::Constant(
                                operation
                                    .evaluate(&p[..args.len])
                                    .map_err(|e| format!("compute error at {address}: {e}"))?,
                            )
                        } else {
                            let output = plan.allocate_slot();
                            plan.instructions.push(Instruction::Compute {
                                operation: *operation,
                                args,
                                output,
                                address,
                            });
                            Source::Slot(output)
                        }
                    }
                    NodeIR::Random { dist_type, .. } => {
                        let address = Address(super::model_compilation::instance_address(
                            id, dimensions, &indices,
                        ));
                        let kind = DistributionKind::parse(dist_type, params.len())
                            .map_err(|e| format!("{e} at {}", address.0))?;
                        let distribution = if args.constant() {
                            let parameters = args.resolve(&[]);
                            let distribution = kind.create(parameters).map_err(|e| {
                                format!("invalid {dist_type} parameters at {}: {e}", address.0)
                            })?;
                            DistributionSpec::Constant {
                                distribution,
                                parameters,
                                len: args.len,
                            }
                        } else {
                            DistributionSpec::Dynamic(kind, args)
                        };
                        let observed = match mode {
                            Mode::Inference => observed,
                            Mode::Predictive => None,
                        };
                        let output = if observed.is_some() {
                            None
                        } else {
                            Some(plan.allocate_slot())
                        };
                        plan.instructions.push(Instruction::Random {
                            distribution,
                            kind,
                            address,
                            output,
                            observed,
                        });
                        match (observed, output) {
                            (Some(value), _) => Source::Constant(value),
                            (_, Some(slot)) => Source::Slot(slot),
                            _ => unreachable!(),
                        }
                    }
                };
                node_sources.push(source);
            }
            let latent = matches!(node, NodeIR::Random { .. })
                && !normalized.observation_owners.contains_key(&id);
            if all_nodes || matches!(node, NodeIR::Random { .. }) {
                plan.outputs.push(NodeOutput {
                    node_id: id,
                    extents,
                    sources: node_sources.clone(),
                    latent,
                });
            }
            sources.insert(id, node_sources);
        }
        Ok(plan)
    }

    /// Shows the actual optimized instructions, including folded literals and slots.
    /// Limit console output for large plates; the header counts the complete plan.
    pub(super) fn debug_string(&self) -> String {
        fn source(s: Source) -> String {
            match s {
                Source::Constant(v) => format!("{v:?}"),
                Source::Slot(i) => format!("values[{i}]"),
            }
        }
        fn args(a: Arguments) -> String {
            a.values[..a.len]
                .iter()
                .map(|s| source(*s))
                .collect::<Vec<_>>()
                .join(", ")
        }
        let effects = self
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::Random { .. }))
            .count();
        let mut text = format!(
            "Fugue execution plan: {effects} effects, {} computations, {} dynamic slots\n",
            self.instructions.len() - effects,
            self.slot_count
        );
        for instruction in self.instructions.iter().take(64) {
            match instruction {
                Instruction::Compute {
                    operation,
                    args: a,
                    output,
                    address,
                } => {
                    writeln!(
                        text,
                        "{address}: values[{output}] = {operation:?}({})",
                        args(*a)
                    )
                    .unwrap();
                }
                Instruction::Random {
                    distribution,
                    kind,
                    address,
                    output,
                    observed,
                } => {
                    let parameters = match distribution {
                        DistributionSpec::Constant {
                            parameters, len, ..
                        } => parameters[..*len]
                            .iter()
                            .map(|v| format!("{v:?}"))
                            .collect::<Vec<_>>()
                            .join(", "),
                        DistributionSpec::Dynamic(_, a) => args(*a),
                    };
                    if let Some(value) = observed {
                        writeln!(
                            text,
                            "observe({:?}, {}({parameters}), {value:?})",
                            address.0,
                            kind.name()
                        )
                        .unwrap();
                    } else {
                        writeln!(
                            text,
                            "sample({:?}, {}({parameters})) -> values[{}]",
                            address.0,
                            kind.name(),
                            output.unwrap()
                        )
                        .unwrap();
                    }
                }
            }
        }
        if self.instructions.len() > 64 {
            writeln!(
                text,
                "... {} more instructions",
                self.instructions.len() - 64
            )
            .unwrap();
        }
        text.push_str("Retained latent outputs:\n");
        for output in self.outputs.iter().filter(|o| o.latent) {
            writeln!(
                text,
                "  node#{} shape {:?}: {} values",
                output.node_id,
                output.extents,
                output.sources.len()
            )
            .unwrap();
        }
        text
    }

    fn allocate_slot(&mut self) -> usize {
        let slot = self.slot_count;
        self.slot_count += 1;
        slot
    }

    pub(super) fn latent_extents(&self) -> HashMap<u32, Vec<usize>> {
        self.outputs
            .iter()
            .filter(|o| o.latent)
            .map(|o| (o.node_id, o.extents.clone()))
            .collect()
    }

    pub(super) fn model(self: &Arc<Self>) -> Model<Result<ModelValues, String>> {
        self.start(materialize)
    }
    pub(super) fn inference_model(self: &Arc<Self>) -> Model<Result<PosteriorValues, String>> {
        self.start(|plan, values| {
            Ok(plan
                .outputs
                .iter()
                .filter(|o| o.latent)
                .map(|o| {
                    (
                        o.node_id,
                        o.sources
                            .iter()
                            .map(|s| s.read(values))
                            .collect::<Vec<_>>()
                            .into(),
                    )
                })
                .collect())
        })
    }
    fn start<R: Send + 'static>(self: &Arc<Self>, finish: Finish<R>) -> Model<Result<R, String>> {
        drive(
            Arc::clone(self),
            State {
                next: 0,
                values: vec![0.0; self.slot_count],
            },
            finish,
        )
    }
}

type Finish<R> = fn(&ExecutionPlan, &[f64]) -> Result<R, String>;
struct State {
    next: usize,
    values: Vec<f64>,
}

fn drive<R: Send + 'static>(
    plan: Arc<ExecutionPlan>,
    mut state: State,
    finish: Finish<R>,
) -> Model<Result<R, String>> {
    loop {
        let Some(instruction) = plan.instructions.get(state.next) else {
            return pure(finish(&plan, &state.values));
        };
        state.next += 1;
        match instruction {
            Instruction::Compute {
                operation,
                args,
                output,
                address,
            } => {
                let p = args.resolve(&state.values);
                match operation.evaluate(&p[..args.len]) {
                    Ok(value) => state.values[*output] = value,
                    Err(error) => return pure(Err(format!("compute error at {address}: {error}"))),
                }
            }
            Instruction::Random {
                distribution,
                kind,
                address,
                output,
                observed,
            } => {
                let dist = match distribution.create(&state.values) {
                    Ok(dist) => dist,
                    Err(error) => {
                        return pure(Err(format!(
                            "invalid {} parameters at {}: {error}",
                            kind.name(),
                            address.0
                        )));
                    }
                };
                let addr = address.clone();
                let output = *output;
                if let Some(value) = *observed {
                    return Model::ObserveF64 {
                        addr,
                        dist,
                        value,
                        k: Box::new(move |_| drive(plan, state, finish)),
                    };
                }
                return Model::SampleF64 {
                    addr,
                    dist,
                    k: Box::new(move |value| {
                        state.values[output.expect("sample instruction has a slot")] = value;
                        drive(plan, state, finish)
                    }),
                };
            }
        }
    }
}

fn node_params(node: &NodeIR) -> &[ParamIR] {
    match node {
        NodeIR::Scalar { .. } => &[],
        NodeIR::Compute { params, .. } | NodeIR::Random { params, .. } => params,
    }
}

pub(super) fn unflatten(mut index: usize, extents: &[usize]) -> Vec<usize> {
    let mut indices = vec![0; extents.len()];
    for axis in (0..extents.len()).rev() {
        indices[axis] = index % extents[axis];
        index /= extents[axis];
    }
    indices
}

fn materialize(plan: &ExecutionPlan, values: &[f64]) -> Result<ModelValues, String> {
    fn nest(output: &NodeOutput, values: &[f64], depth: usize, offset: &mut usize) -> ModelResult {
        if depth == output.extents.len() {
            let value = output.sources[*offset].read(values);
            *offset += 1;
            ModelResult::Scalar(value)
        } else {
            ModelResult::Plate(
                (0..output.extents[depth])
                    .map(|_| nest(output, values, depth + 1, offset))
                    .collect(),
            )
        }
    }
    Ok(plan
        .outputs
        .iter()
        .map(|o| (o.node_id, nest(o, values, 0, &mut 0)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bayesian_core::{CompiledGraph, PlateIR};
    use fugue::{Choice, ChoiceValue, PriorHandler, ScoreGivenTrace, Trace};
    use rand::{SeedableRng, rngs::StdRng};

    fn scalar(graph: &mut GraphIR, id: u32, value: f64) {
        graph.nodes.insert(id, NodeIR::Scalar { id, value });
    }
    fn random(graph: &mut GraphIR, id: u32, distribution: &str, inputs: &[u32]) {
        graph.nodes.insert(
            id,
            NodeIR::Random {
                id,
                label: None,
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
                id,
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
        assert!(compiled.graph().format_model_values(&values).is_ok());
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
}
