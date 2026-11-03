//! A compiled instruction plan. Deterministic work runs in a loop; only a
//! probabilistic effect suspends execution and owns a continuation.
use super::graph_checks::PosteriorValues;
#[cfg(test)]
use super::graph_checks::{ModelResult, ModelValues};
use super::plate_validation::{NormalizedPlates, projection_positions};
use super::{CompilationError, GraphIR, NodeIR, ParamIR, Value};
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
    #[cfg(test)]
    Predictive,
}

#[derive(Clone, Debug)]
enum Source {
    Constant(Value),
    Slot(usize),
}
impl Source {
    fn read(&self, values: &[Value]) -> Value {
        match self {
            Self::Constant(value) => value.clone(),
            Self::Slot(slot) => values[*slot].clone(),
        }
    }
}

#[derive(Clone)]
struct Arguments {
    values: [Source; 2],
    len: usize,
}
impl Arguments {
    fn resolve(&self, slots: &[Value]) -> Vec<Value> {
        self.values[..self.len]
            .iter()
            .map(|source| source.read(slots))
            .collect()
    }
    fn real_parameters(&self, slots: &[Value]) -> Result<[f64; 2], String> {
        let mut result = [0.0; 2];
        for (index, value) in self.resolve(slots).iter().enumerate() {
            result[index] = value.real_scalar()?;
        }
        Ok(result)
    }
    fn constant(&self) -> bool {
        self.values[..self.len]
            .iter()
            .all(|source| matches!(source, Source::Constant(_)))
    }
}

#[derive(Clone, Copy)]
pub(super) enum DistributionKind {
    Normal,
    Uniform,
    Beta,
    Exponential,
    Gamma,
    LogNormal,
}
impl DistributionKind {
    pub(super) fn parse(name: &str, arity: usize) -> Result<Self, String> {
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
    pub(super) fn create(self, p: [f64; 2]) -> Result<Box<dyn Distribution<f64>>, String> {
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
    fn create(&self, slots: &[Value]) -> Result<Box<dyn Distribution<f64>>, String> {
        match self {
            Self::Constant { distribution, .. } => Ok(distribution.clone_box()),
            Self::Dynamic(kind, args) => kind.create(args.real_parameters(slots)?),
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
    ) -> Result<Self, CompilationError> {
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
                .ok_or_else(|| CompilationError::at(id, "plate instance count overflows"))?;
            let params = node_params(node);
            if params.len() > 2 {
                return Err(CompilationError::at(id, "too many parameters"));
            }
            let positions = params
                .iter()
                .map(|p| projection_positions(&normalized.node_paths[&p.from_node], dimensions))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| CompilationError::at(id, "incompatible plate dimensions"))?;
            let mut node_sources = Vec::with_capacity(count);
            for flat_index in 0..count {
                let indices = unflatten(flat_index, &extents);
                let mut args = Arguments {
                    values: std::array::from_fn(|_| Source::Constant(Value::Real(0.0))),
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
                    args.values[i] = sources[&param.from_node][producer_index].clone();
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
                    NodeIR::Scalar { value, .. } => {
                        Source::Constant(Value::Real(observed.unwrap_or(*value)))
                    }
                    NodeIR::Constant { value } => Source::Constant(match observed {
                        Some(raw) => Value::observation(raw, value.value_type())
                            .map_err(|error| CompilationError::at(id, error))?,
                        None => value.clone(),
                    }),
                    NodeIR::Compute { operation, .. } => {
                        let address =
                            super::model_compilation::instance_address(id, dimensions, &indices);
                        if args.constant() {
                            let p = args.resolve(&[]);
                            Source::Constant(operation.evaluate_values(&p).map_err(|e| {
                                CompilationError::at(id, format!("compute error at {address}: {e}"))
                            })?)
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
                        let kind =
                            DistributionKind::parse(dist_type, params.len()).map_err(|e| {
                                CompilationError::at(id, format!("{e} at {}", address.0))
                            })?;
                        let distribution = if args.constant() {
                            let parameters = args
                                .real_parameters(&[])
                                .map_err(|error| CompilationError::at(id, error))?;
                            let distribution = kind.create(parameters).map_err(|e| {
                                CompilationError::at(
                                    id,
                                    format!("invalid {dist_type} parameters at {}: {e}", address.0),
                                )
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
                            #[cfg(test)]
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
                            (Some(value), _) => Source::Constant(Value::Real(value)),
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
        fn source(s: &Source) -> String {
            match s {
                Source::Constant(v) => format!("{v:?}"),
                Source::Slot(i) => format!("values[{i}]"),
            }
        }
        fn args(a: &Arguments) -> String {
            a.values[..a.len]
                .iter()
                .map(|s| source(s))
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
                        args(a)
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
                        DistributionSpec::Dynamic(_, a) => args(a),
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

    #[cfg(test)]
    pub(super) fn model(self: &Arc<Self>) -> Model<Result<ModelValues, String>> {
        self.start(materialize)
    }
    pub(super) fn inference_model(self: &Arc<Self>) -> Model<Result<PosteriorValues, String>> {
        self.start(|plan, values| {
            plan.outputs
                .iter()
                .filter(|o| o.latent)
                .map(|o| {
                    let values = o
                        .sources
                        .iter()
                        .map(|s| s.read(values).real_scalar())
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok((o.node_id, values.into()))
                })
                .collect()
        })
    }
    fn start<R: Send + 'static>(self: &Arc<Self>, finish: Finish<R>) -> Model<Result<R, String>> {
        drive(
            Arc::clone(self),
            State {
                next: 0,
                values: vec![Value::Real(0.0); self.slot_count],
            },
            finish,
        )
    }
}

type Finish<R> = fn(&ExecutionPlan, &[Value]) -> Result<R, String>;
struct State {
    next: usize,
    values: Vec<Value>,
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
                match operation.evaluate_values(&p) {
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
                        state.values[output.expect("sample instruction has a slot")] =
                            Value::Real(value);
                        drive(plan, state, finish)
                    }),
                };
            }
        }
    }
}

fn node_params(node: &NodeIR) -> &[ParamIR] {
    match node {
        NodeIR::Scalar { .. } | NodeIR::Constant { .. } => &[],
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

#[cfg(test)]
fn materialize(plan: &ExecutionPlan, values: &[Value]) -> Result<ModelValues, String> {
    fn nest(
        output: &NodeOutput,
        values: &[Value],
        depth: usize,
        offset: &mut usize,
    ) -> ModelResult {
        if depth == output.extents.len() {
            let value = output.sources[*offset].read(values);
            *offset += 1;
            match value {
                Value::Real(value) => ModelResult::Scalar(value),
                Value::Integer(value) => ModelResult::Integer(value),
                vector => ModelResult::Vector(vector),
            }
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
#[path = "tests/execution.rs"]
mod tests;
