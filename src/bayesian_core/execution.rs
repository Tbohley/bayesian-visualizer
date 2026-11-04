//! A compiled instruction plan. Deterministic work runs in a loop; only a
//! probabilistic effect suspends execution and owns a continuation.
use super::distributions::{DistributionKind, RuntimeDistribution};
use super::graph_checks::PosteriorValues;
#[cfg(test)]
use super::graph_checks::{ModelResult, ModelValues};
use super::plate_validation::{NormalizedPlates, projection_positions};
use super::{CompilationError, GraphIR, NodeIR, ParamIR, Value};
use crate::nodes::Operation;
use fugue::{Address, Model, pure};
#[cfg(test)]
use fugue::{Distribution, Normal};
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
    values: Vec<Source>,
}
impl Arguments {
    fn resolve(&self, slots: &[Value]) -> Vec<Value> {
        self.values
            .iter()
            .map(|source| source.read(slots))
            .collect()
    }
    fn constant(&self) -> bool {
        self.values
            .iter()
            .all(|source| matches!(source, Source::Constant(_)))
    }
}

enum DistributionSpec {
    Constant {
        distribution: RuntimeDistribution,
        parameters: Vec<Value>,
    },
    Dynamic(DistributionKind, Arguments),
}
impl DistributionSpec {
    fn create(&self, slots: &[Value]) -> Result<RuntimeDistribution, String> {
        match self {
            Self::Constant { distribution, .. } => Ok(distribution.clone()),
            Self::Dynamic(kind, args) => kind.create(&args.resolve(slots)),
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
        observed: Option<Value>,
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
            let positions = params
                .iter()
                .map(|p| projection_positions(&normalized.node_paths[&p.from_node], dimensions))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| CompilationError::at(id, "incompatible plate dimensions"))?;
            let mut node_sources = Vec::with_capacity(count);
            for flat_index in 0..count {
                let indices = unflatten(flat_index, &extents);
                let mut args = Arguments {
                    values: Vec::with_capacity(params.len()),
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
                    args.values
                        .push(sources[&param.from_node][producer_index].clone());
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
                            let parameters = args.resolve(&[]);
                            let distribution = kind.create(&parameters).map_err(|e| {
                                CompilationError::at(
                                    id,
                                    format!("invalid {dist_type} parameters at {}: {e}", address.0),
                                )
                            })?;
                            DistributionSpec::Constant {
                                distribution,
                                parameters,
                            }
                        } else {
                            DistributionSpec::Dynamic(kind, args)
                        };
                        let observed = observed
                            .map(|raw| Value::observation(raw, kind.output_type()))
                            .transpose()
                            .map_err(|e| CompilationError::at(id, e))?;
                        if let (Some(value), DistributionSpec::Constant { distribution, .. }) =
                            (&observed, &distribution)
                        {
                            if kind.output_type() == super::ValueType::INTEGER
                                && !distribution.accepts(value)
                            {
                                return Err(CompilationError::at(
                                    id,
                                    "observation is outside the distribution support (or has zero probability)",
                                ));
                            }
                        }
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
                            observed: observed.clone(),
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
        fn value_text(v: &Value) -> String {
            match v {
                Value::Real(v) => format!("{v:?}"),
                Value::Integer(v) => format!("{v}"),
                _ => format!("{v:?}"),
            }
        }
        fn source(s: &Source) -> String {
            match s {
                Source::Constant(v) => format!("{v:?}"),
                Source::Slot(i) => format!("values[{i}]"),
            }
        }
        fn args(a: &Arguments) -> String {
            a.values
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
                        DistributionSpec::Constant { parameters, .. } => parameters
                            .iter()
                            .map(value_text)
                            .collect::<Vec<_>>()
                            .join(", "),
                        DistributionSpec::Dynamic(_, a) => args(a),
                    };
                    if let Some(value) = observed {
                        let value = value_text(value);
                        writeln!(
                            text,
                            "observe({:?}, {}({parameters}), {value})",
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
                match (dist, observed.clone()) {
                    (RuntimeDistribution::Real(dist), Some(Value::Real(value))) => {
                        return Model::ObserveF64 {
                            addr,
                            dist,
                            value,
                            k: Box::new(move |_| drive(plan, state, finish)),
                        };
                    }
                    (RuntimeDistribution::Count(dist), Some(Value::Integer(value))) => {
                        return Model::ObserveU64 {
                            addr,
                            dist,
                            value: value as u64,
                            k: Box::new(move |_| drive(plan, state, finish)),
                        };
                    }
                    (RuntimeDistribution::Category(dist), Some(Value::Integer(value))) => {
                        return Model::ObserveUsize {
                            addr,
                            dist,
                            value: value as usize,
                            k: Box::new(move |_| drive(plan, state, finish)),
                        };
                    }
                    (RuntimeDistribution::Real(dist), None) => {
                        return Model::SampleF64 {
                            addr,
                            dist,
                            k: Box::new(move |value| {
                                state.values[output.unwrap()] = Value::Real(value);
                                drive(plan, state, finish)
                            }),
                        };
                    }
                    (RuntimeDistribution::Count(dist), None) => {
                        return Model::SampleU64 {
                            addr,
                            dist,
                            k: Box::new(move |value| {
                                state.values[output.unwrap()] = Value::Integer(value as i64);
                                drive(plan, state, finish)
                            }),
                        };
                    }
                    (RuntimeDistribution::Category(dist), None) => {
                        return Model::SampleUsize {
                            addr,
                            dist,
                            k: Box::new(move |value| {
                                state.values[output.unwrap()] = Value::Integer(value as i64);
                                drive(plan, state, finish)
                            }),
                        };
                    }
                    _ => unreachable!("observation type checked at compilation"),
                }
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
