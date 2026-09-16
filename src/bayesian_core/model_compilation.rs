use super::graph_checks::{GraphModel, ModelResult, ModelValues};
use super::plate_validation::{NormalizedPlates, projection_positions};
use super::{GraphIR, NodeIR, ParamIR};
use crate::nodes::Operation;
use fugue::{
    Address, Beta, Distribution, Exponential, FugueResult, Gamma, LogNormal, Model, ModelExt,
    Normal, Uniform, pure,
};
use std::collections::HashMap;
use std::fmt::Write as _;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
/// Flat runtime identity for one concrete node instance at specific plate indices.
struct InstanceKey {
    node_id: u32,
    indices: Vec<usize>,
}

#[derive(Default)]
/// Flat collection of scalar values produced during one model execution.
struct ExecutionState {
    values: HashMap<InstanceKey, f64>,
}

type ExecutionModel = Model<Result<ExecutionState, String>>;
type DynDistribution = Box<dyn Distribution<f64>>;

#[derive(Clone, Copy)]
enum ExecutionMode {
    Inference,
    Predictive,
}

/// Preprocessed parameter metadata used to locate the applicable producer instance.
#[derive(Clone)]
struct CompiledParam {
    from_node: u32,
    consumer_positions: Vec<usize>,
    producer_plate_ids: Vec<u32>,
}

/// Plate path and extents needed to reconstruct one node's public result shape.
#[derive(Clone)]
struct NodeShape {
    node_id: u32,
    plate_ids: Vec<u32>,
    extents: Vec<usize>,
}

/// Structurally validated graph data that can cheaply create fresh Fugue models.
///
/// Fugue consumes a model on every execution, so the bind tree is rebuilt for
/// each proposal. Cycle checks, topological sorting, plate normalization, and
/// result-shape derivation are retained here and performed only once.
pub struct CompiledGraph {
    graph: GraphIR,
    normalized: NormalizedPlates,
    order: Vec<u32>,
    shapes: Vec<NodeShape>,
}

impl CompiledGraph {
    pub(crate) fn new(graph: GraphIR) -> Result<Self, String> {
        if let Err(cycle) = graph.check_cycles() {
            return Err(format!(
                "graph contains a cycle including node IDs: {cycle:?}"
            ));
        }

        let order = graph.topological_sort()?;
        if order.len() != graph.nodes.len() {
            return Err("graph contains a cycle".to_string());
        }

        let normalized = graph.validated_plates()?;
        let shapes = node_shapes(&graph, &normalized)?;
        let compiled = Self {
            graph,
            normalized,
            order,
            shapes,
        };

        // Surface deterministic model-construction errors at compile time.
        compiled.model()?;
        Ok(compiled)
    }

    pub fn graph(&self) -> &GraphIR {
        &self.graph
    }

    pub fn node_plate_path(&self, node_id: u32) -> Option<&[u32]> {
        self.normalized.node_paths.get(&node_id).map(Vec::as_slice)
    }

    pub fn model(&self) -> Result<GraphModel, String> {
        self.model_for(ExecutionMode::Inference)
    }

    pub(crate) fn predictive_model(&self) -> Result<GraphModel, String> {
        self.model_for(ExecutionMode::Predictive)
    }

    fn model_for(&self, mode: ExecutionMode) -> Result<GraphModel, String> {
        let mut model = pure(Ok(ExecutionState::default()));
        for &node_id in &self.order {
            let node = self.graph.nodes[&node_id].clone();
            let plate_ids = self.normalized.node_paths[&node_id].clone();
            let extents = plate_ids
                .iter()
                .map(|plate| self.normalized.extents[plate])
                .collect::<Vec<_>>();
            let params =
                compiled_params(node_params(&node), &plate_ids, &self.normalized.node_paths)?;
            for indices in cartesian_indices(&extents) {
                let data_value = observation_value(
            &self.graph,
            &self.normalized,
                    node_id,
                    &plate_ids,
                    &indices,
                )?;
                model = compile_node_instance(
                    node.clone(),
                    params.clone(),
                    plate_ids.clone(),
                    indices,
                    data_value,
            mode,
            model,
                );
            }
        }

        let shapes = self.shapes.clone();
        Ok(model.bind(move |result: Result<ExecutionState, String>| {
            pure(result.and_then(|state| materialize_values(&state, &shapes)))
        }))
    }
}

/// Validates and compiles a graph into an executable hierarchical Fugue model.
pub(crate) fn create_model(graph: &GraphIR) -> Result<GraphModel, String> {
    CompiledGraph::new(graph.clone())?.model()
}

fn observation_value(
    graph: &GraphIR,
    normalized: &NormalizedPlates,
    node_id: u32,
    plate_ids: &[u32],
    indices: &[usize],
) -> Result<Option<f64>, String> {
    let Some(&owner_id) = normalized.observation_owners.get(&node_id) else {
        return Ok(None);
    };
    let owner = &graph.plates[&owner_id];
    let column = &owner.mapping[&node_id];
    let position = plate_ids.binary_search(&owner_id).map_err(|_| {
        format!("observation owner plate {owner_id} is not a dimension of node {node_id}")
    })?;
    let row = indices[position];
    owner
                        .data
                        .get(column)
                        .and_then(|values| values.get(row))
        .copied()
        .map(Some)
        .ok_or_else(|| format!("plate {owner_id} column {column:?} has no value at row {row}"))
    }

fn cartesian_indices(extents: &[usize]) -> Vec<Vec<usize>> {
    let mut paths = vec![Vec::new()];
    for &extent in extents {
        paths = paths
            .into_iter()
            .flat_map(|path| {
                (0..extent).map(move |index| {
                    let mut next = path.clone();
                    next.push(index);
                    next
                })
            })
            .collect();
    }
    paths
}

/// Extends the execution model with one node instance at its plate indices.
fn compile_node_instance(
    node: NodeIR,
    params: Vec<CompiledParam>,
    plate_ids: Vec<u32>,
    indices: Vec<usize>,
    data_value: Option<f64>,
    mode: ExecutionMode,
    model: ExecutionModel,
) -> ExecutionModel {
    let node_id = node_id(&node);
    let address = instance_address(node_id, &plate_ids, &indices);
    let key = InstanceKey {
        node_id,
        indices: indices.clone(),
    };

    model.bind(move |result| {
        let mut state = match result {
            Ok(state) => state,
            Err(error) => return pure(Err(error)),
        };

        match node {
            NodeIR::Scalar { value, .. } => {
                state.values.insert(key, data_value.unwrap_or(value));
                pure(Ok(state))
            }
            NodeIR::Compute { operation, .. } => {
                let values = match resolve_params(&params, &indices, &state) {
                    Ok(values) => values,
                    Err(error) => return pure(Err(format!("{error} at {address}"))),
                };

                match operation.evaluate(&values) {
                    Ok(value) => {
                        state.values.insert(key, value);
                        pure(Ok(state))
                    }
                    Err(error) => pure(Err(format!("compute error at {address}: {error}"))),
                }
            }
            NodeIR::Random {
                dist_type,
                params: _,
                ..
            } => {
                let values = match resolve_params(&params, &indices, &state) {
                    Ok(values) => values,
                    Err(error) => return pure(Err(format!("{error} at {address}"))),
                };
                let distribution = match create_distribution(&dist_type, &values) {
                    Ok(distribution) => distribution,
                    Err(error) => {
                        return pure(Err(format!(
                            "invalid {dist_type} parameters at {address}: {error}"
                        )));
                    }
                };

                if let (ExecutionMode::Inference, Some(value)) = (mode, data_value) {
                    Model::ObserveF64 {
                        addr: Address(address),
                        dist: distribution,
                        value,
                        k: Box::new(pure),
                    }
                    .bind(move |_| {
                        state.values.insert(key, value);
                        pure(Ok(state))
                    })
                } else {
                    Model::SampleF64 {
                        addr: Address(address),
                        dist: distribution,
                        k: Box::new(pure),
                    }
                    .bind(move |value| {
                        state.values.insert(key, value);
                        pure(Ok(state))
                    })
                }
            }
        }
    })
}

/// Records each parameter producer's plate depth so instances can be resolved correctly.
fn compiled_params(
    params: &[ParamIR],
    consumer_plate_ids: &[u32],
    node_paths: &HashMap<u32, Vec<u32>>,
) -> Result<Vec<CompiledParam>, String> {
    params
        .iter()
        .map(|param| {
            let producer_plate_ids = node_paths
                .get(&param.from_node)
                .ok_or_else(|| format!("parameter references missing node {}", param.from_node))?
                .clone();
            let consumer_positions = projection_positions(&producer_plate_ids, consumer_plate_ids)
                .map_err(|_| {
                    format!("node {} has incompatible plate dimensions", param.from_node)
                })?;
            Ok(CompiledParam {
                from_node: param.from_node,
                consumer_positions,
                producer_plate_ids,
            })
        })
        .collect()
}

/// Resolves each parameter by projecting the consumer coordinates onto producer dimensions.
fn resolve_params(
    params: &[CompiledParam],
    consumer_indices: &[usize],
    state: &ExecutionState,
) -> Result<Vec<f64>, String> {
    params
        .iter()
        .map(|param| {
            let producer_indices = param
                .consumer_positions
                .iter()
                .map(|&position| consumer_indices[position])
                .collect::<Vec<_>>();
            let key = InstanceKey {
                node_id: param.from_node,
                indices: producer_indices,
            };
            state.values.get(&key).copied().ok_or_else(|| {
                format!(
                    "parameter references unavailable node instance {}",
                    instance_address(param.from_node, &param.producer_plate_ids, &key.indices)
                )
            })
        })
        .collect()
}

/// Derives the plate dimensions needed to reconstruct each node's hierarchical value.
fn node_shapes(graph: &GraphIR, normalized: &NormalizedPlates) -> Result<Vec<NodeShape>, String> {
    let mut node_ids = graph.nodes.keys().copied().collect::<Vec<_>>();
    node_ids.sort_unstable();

    node_ids
        .into_iter()
        .map(|node_id| {
            let plate_ids = normalized
                .node_paths
                .get(&node_id)
                .cloned()
                .ok_or_else(|| format!("node {node_id} has no normalized plate path"))?;
            let extents = plate_ids
                .iter()
                .map(|plate_id| {
                    graph
                        .plates
                        .get(plate_id)
                        .map(|plate| plate.n)
                        .ok_or_else(|| {
                            format!("node {node_id} references missing plate {plate_id}")
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(NodeShape {
                node_id,
                plate_ids,
                extents,
            })
        })
        .collect()
}

/// Rebuilds the flat execution-state values into results keyed by graph node ID.
fn materialize_values(state: &ExecutionState, shapes: &[NodeShape]) -> Result<ModelValues, String> {
    let mut values = HashMap::with_capacity(shapes.len());
    for shape in shapes {
        let value = materialize_node(state, shape, 0, &mut Vec::new())?;
        values.insert(shape.node_id, value);
    }
    Ok(values)
}

/// Recursively materializes a scalar node instance or its nested plate values.
fn materialize_node(
    state: &ExecutionState,
    shape: &NodeShape,
    depth: usize,
    indices: &mut Vec<usize>,
) -> Result<ModelResult, String> {
    if depth == shape.extents.len() {
        let key = InstanceKey {
            node_id: shape.node_id,
            indices: indices.clone(),
        };
        return state
            .values
            .get(&key)
            .copied()
            .map(ModelResult::Scalar)
            .ok_or_else(|| {
                format!(
                    "model did not produce {}",
                    instance_address(shape.node_id, &shape.plate_ids, indices)
                )
            });
    }

    let mut items = Vec::with_capacity(shape.extents[depth]);
    for index in 0..shape.extents[depth] {
        indices.push(index);
        items.push(materialize_node(state, shape, depth + 1, indices)?);
        indices.pop();
    }
    Ok(ModelResult::Plate(items))
}

impl GraphIR {
    /// Renders the inference model as concise, source-like Fugue code.
    ///
    /// This is a presentation-oriented explanation of the compiled model, not
    /// an exact expansion of the engine's internal execution-state machinery.
    pub fn bind_debug_string(&self) -> Result<String, String> {
        let compiled = CompiledGraph::new(self.clone())?;
        compiled.bind_debug_string()
    }

    // pub fn ancestral_sample_debug(&self) -> Result<String, String> {
    //     let values = self.ancestral_sample()?;
    //     self.format_model_values(&values)
    // }

    /// Formats every sampled node value as human-readable instance lines.
    pub fn format_model_values(&self, values: &ModelValues) -> Result<String, String> {
        let normalized = self.validated_plates()?;
        let mut node_ids = self.nodes.keys().copied().collect::<Vec<_>>();
        node_ids.sort_unstable();
        let mut lines = Vec::new();

        for node_id in node_ids {
            let value = values
                .get(&node_id)
                .ok_or_else(|| format!("sample results are missing node {node_id}"))?;
            format_node_instances(self, &normalized, node_id, value, &mut lines)?;
        }

        Ok(lines.join("\n"))
    }

    /// Formats one node's scalar or plated result as human-readable instance lines.
    pub fn format_node_value(&self, node_id: u32, value: &ModelResult) -> Result<String, String> {
        let normalized = self.validated_plates()?;
        let mut lines = Vec::new();
        format_node_instances(self, &normalized, node_id, value, &mut lines)?;

        if lines.is_empty() {
            let node = self
                .nodes
                .get(&node_id)
                .ok_or_else(|| format!("graph is missing node {node_id}"))?;
            return Ok(format!(
                "{} @ node#{node_id} = {value:?}",
                node_display_name(node)
            ));
        }

        Ok(lines.join("\n"))
    }
}

impl CompiledGraph {
    /// Renders a compact `prob!`/`plate!` view of the compiled inference model.
    pub fn bind_debug_string(&self) -> Result<String, String> {
        let mut output = String::new();

        let mut observed_nodes = self
            .normalized
            .observation_owners
            .iter()
            .map(|(&node_id, &plate_id)| (node_id, plate_id))
            .collect::<Vec<_>>();
        observed_nodes.sort_unstable();
        for (node_id, plate_id) in observed_nodes {
            let column = &self.graph.plates[&plate_id].mapping[&node_id];
            writeln!(
                output,
                "let observed_node_{node_id} = data.column({column:?}); // plate {plate_id}"
            )
            .expect("writing to a String cannot fail");
        }
        if !self.normalized.observation_owners.is_empty() {
            output.push('\n');
        }

        output.push_str("let model = prob! {\n");

        for &node_id in &self.order {
            let node = &self.graph.nodes[&node_id];
            let plate_ids = &self.normalized.node_paths[&node_id];
            if let NodeIR::Random {
                label: Some(label), ..
            } = node
            {
                writeln!(output, "    // {label}").expect("writing to a String cannot fail");
            }

            if plate_ids.is_empty() {
                render_root_node(&mut output, node, &self.normalized.node_paths);
            } else {
                render_plated_node(
                    &mut output,
                    node,
                    plate_ids,
                    &self.normalized,
                    &self.normalized.node_paths,
                );
            }
        }

        let results = self
            .order
            .iter()
            .map(|node_id| format!("node_{node_id}"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(output, "    pure(({results}))").expect("writing to a String cannot fail");
        output.push_str("};\n");

        Ok(output)
    }
}

/// Renders a node with no repeated dimensions directly inside `prob!`.
fn render_root_node(output: &mut String, node: &NodeIR, node_paths: &HashMap<u32, Vec<u32>>) {
    let node_id = node_id(node);
    match node {
        NodeIR::Scalar { value, .. } => {
            writeln!(output, "    let node_{node_id} = {value:?};")
                .expect("writing to a String cannot fail");
        }
        NodeIR::Compute {
            operation, params, ..
        } => {
            let params = debug_param_expressions(params, node_paths);
            let expression = debug_compute_expression(*operation, &params);
            writeln!(output, "    let node_{node_id} = {expression};")
                .expect("writing to a String cannot fail");
        }
        NodeIR::Random {
            dist_type, params, ..
        } => {
            let params = debug_param_expressions(params, node_paths);
            let distribution = debug_distribution(dist_type, &params);
            writeln!(
                output,
                "    let node_{node_id} <- sample(addr!(\"node_{node_id}\"), {distribution});"
            )
            .expect("writing to a String cannot fail");
        }
    }
}

/// Renders a node as one or more nested Fugue plates.
fn render_plated_node(
    output: &mut String,
    node: &NodeIR,
    plate_ids: &[u32],
    normalized: &NormalizedPlates,
    node_paths: &HashMap<u32, Vec<u32>>,
) {
    let node_id = node_id(node);
    output.push_str(&format!("    let node_{node_id} <- "));
    for (depth, plate_id) in plate_ids.iter().enumerate() {
        if depth > 0 {
            output.push_str(&"    ".repeat(depth + 1));
        }
        writeln!(
            output,
            "plate!(i_{plate_id} in 0..{} => {{",
            normalized.extents[plate_id]
        )
        .expect("writing to a String cannot fail");
    }

    let indent = "    ".repeat(plate_ids.len() + 1);
    let expression = debug_plated_node_expression(node, plate_ids, normalized, node_paths);
    writeln!(output, "{indent}{expression}").expect("writing to a String cannot fail");

    for depth in (0..plate_ids.len()).rev() {
        output.push_str(&"    ".repeat(depth + 1));
        output.push_str("})");
        if depth == 0 {
            output.push_str(";\n");
        } else {
            output.push('\n');
        }
    }
}

/// Produces the model expression at the innermost level of a rendered plate.
fn debug_plated_node_expression(
    node: &NodeIR,
    plate_ids: &[u32],
    normalized: &NormalizedPlates,
    node_paths: &HashMap<u32, Vec<u32>>,
) -> String {
    let node_id = node_id(node);
    let observed = normalized
        .observation_owners
        .get(&node_id)
        .map(|plate_id| format!("observed_node_{node_id}[i_{plate_id}]"));

    match node {
        NodeIR::Scalar { value, .. } => {
            format!("pure({})", observed.unwrap_or_else(|| format!("{value:?}")))
        }
        NodeIR::Compute {
            operation, params, ..
        } => {
            let params = debug_param_expressions(params, node_paths);
            format!("pure({})", debug_compute_expression(*operation, &params))
        }
        NodeIR::Random {
            dist_type, params, ..
        } => {
            let params = debug_param_expressions(params, node_paths);
            let distribution = debug_distribution(dist_type, &params);
            let address = debug_address(node_id, plate_ids);
            match observed {
                Some(value) => {
                    format!("observe({address}, {distribution}, {value}).map(move |_| {value})")
                }
                None => format!("sample({address}, {distribution})"),
            }
        }
    }
}

/// Expresses parameter projections as direct indexing into upstream plate results.
fn debug_param_expressions(params: &[ParamIR], node_paths: &HashMap<u32, Vec<u32>>) -> Vec<String> {
    params
        .iter()
        .map(|param| {
            let mut expression = format!("node_{}", param.from_node);
            for plate_id in &node_paths[&param.from_node] {
                expression.push_str(&format!("[i_{plate_id}]"));
            }
            expression
        })
        .collect()
}

/// Uses ordinary mathematical notation for deterministic graph nodes.
fn debug_compute_expression(operation: Operation, params: &[String]) -> String {
    match (operation, params) {
        (Operation::Add, [a, b]) => format!("({a} + {b})"),
        (Operation::Subtract, [a, b]) => format!("({a} - {b})"),
        (Operation::Multiply, [a, b]) => format!("({a} * {b})"),
        (Operation::Divide, [a, b]) => format!("({a} / {b})"),
        (Operation::Power, [base, exponent]) => format!("{base}.powf({exponent})"),
        (Operation::Exponential, [value]) => format!("{value}.exp()"),
        (Operation::Logarithm, [value]) => format!("{value}.ln()"),
        (Operation::Sum, [values]) => format!("{values}.iter().sum()"),
        (Operation::Product, [values]) => format!("{values}.iter().product()"),
        _ => format!(
            "Operation::{operation:?}.evaluate(&[{}]).unwrap()",
            params.join(", ")
        ),
    }
}

/// Constructs a concrete Fugue distribution expression.
fn debug_distribution(dist_type: &str, params: &[String]) -> String {
    format!("{dist_type}::new({}).unwrap()", params.join(", "))
}

/// Gives each displayed random variable a compact indexed Fugue address.
fn debug_address(node_id: u32, plate_ids: &[u32]) -> String {
    if plate_ids.is_empty() {
        return format!("addr!(\"node_{node_id}\")");
    }

    let format_string = std::iter::repeat_n("{}", plate_ids.len())
        .collect::<Vec<_>>()
        .join("_");
    let indices = plate_ids
        .iter()
        .map(|plate_id| format!("i_{plate_id}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("scoped_addr!(\"node\", \"{node_id}\", \"{format_string}\", {indices})")
}

/// Formats all instances of a node according to its normalized plate path.
fn format_node_instances(
    graph: &GraphIR,
    normalized: &NormalizedPlates,
    node_id: u32,
    value: &ModelResult,
    lines: &mut Vec<String>,
) -> Result<(), String> {
    let plate_ids = normalized
        .node_paths
        .get(&node_id)
        .ok_or_else(|| format!("node {node_id} has no normalized plate path"))?;
    let display_name = node_display_name(
        graph
            .nodes
            .get(&node_id)
            .ok_or_else(|| format!("graph is missing node {node_id}"))?,
    );
    format_instances(
        node_id,
        &display_name,
        plate_ids,
        value,
        0,
        &mut Vec::new(),
        lines,
    )
}

/// Walks a nested model result and appends one display line for each scalar instance.
fn format_instances(
    node_id: u32,
    display_name: &str,
    plate_ids: &[u32],
    value: &ModelResult,
    depth: usize,
    indices: &mut Vec<usize>,
    lines: &mut Vec<String>,
) -> Result<(), String> {
    match (depth == plate_ids.len(), value) {
        (true, ModelResult::Scalar(value)) => {
            lines.push(format!(
                "{display_name} @ {} = {value:?}",
                instance_address(node_id, plate_ids, indices)
            ));
            Ok(())
        }
        (false, ModelResult::Plate(items)) => {
            for (index, item) in items.iter().enumerate() {
                indices.push(index);
                format_instances(
                    node_id,
                    display_name,
                    plate_ids,
                    item,
                    depth + 1,
                    indices,
                    lines,
                )?;
                indices.pop();
            }
            Ok(())
        }
        (true, ModelResult::Plate(_)) => Err(format!(
            "node {node_id} has an unexpected extra plate dimension"
        )),
        (false, ModelResult::Scalar(_)) => Err(format!(
            "node {node_id} is missing plate dimension {}",
            plate_ids[depth]
        )),
    }
}

/// Builds the stable textual address for a node instance and its plate indices.
fn instance_address(node_id: u32, plate_ids: &[u32], indices: &[usize]) -> String {
    let mut address = format!("node#{node_id}");
    for (&plate_id, &index) in plate_ids.iter().zip(indices) {
        address.push_str(&format!("/plate#{plate_id}[{index}]"));
    }
    address
}

/// Returns a node's label when present, or a fallback name based on its ID.
fn node_display_name(node: &NodeIR) -> String {
    match node {
        NodeIR::Random {
            id,
            label: Some(label),
            ..
        } => format!("{label} (node {id})"),
        _ => format!("node {}", node_id(node)),
    }
}

/// Extracts the ID stored by any graph node variant.
fn node_id(node: &NodeIR) -> u32 {
    match node {
        NodeIR::Random { id, .. } | NodeIR::Scalar { id, .. } | NodeIR::Compute { id, .. } => *id,
    }
}

/// Returns a node's upstream parameters, or an empty slice for scalar nodes.
fn node_params(node: &NodeIR) -> &[ParamIR] {
    match node {
        NodeIR::Random { params, .. } | NodeIR::Compute { params, .. } => params,
        NodeIR::Scalar { .. } => &[],
    }
}

/// Boxes a concrete Fugue distribution while converting construction errors to strings.
fn boxed<D: Distribution<f64> + 'static>(
    result: FugueResult<D>,
) -> Result<DynDistribution, String> {
    result
        .map(|dist| Box::new(dist) as DynDistribution)
        .map_err(|error| error.to_string())
}

/// Constructs the requested distribution after checking its expected parameter arity.
fn create_distribution(dist_type: &str, params: &[f64]) -> Result<DynDistribution, String> {
    match (dist_type, params) {
        ("Normal", &[mu, sigma]) => boxed(Normal::new(mu, sigma)),
        ("Uniform", &[low, high]) => boxed(Uniform::new(low, high)),
        ("Beta", &[alpha, beta]) => boxed(Beta::new(alpha, beta)),
        ("Exponential", &[rate]) => boxed(Exponential::new(rate)),
        ("Gamma", &[shape, rate]) => boxed(Gamma::new(shape, rate)),
        ("LogNormal", &[mu, sigma]) => boxed(LogNormal::new(mu, sigma)),
        (name, _) => Err(format!("wrong number of parameters for {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bayesian_core::PlateIR;

    fn empty_plate(id: u32, n: usize, nodes: Vec<u32>) -> PlateIR {
        PlateIR {
            id,
            n,
            nodes,
            plates: vec![],
            data: HashMap::new(),
            mapping: HashMap::new(),
        }
    }

    #[test]
    /// Verifies mapped data columns observe random nodes and override scalar literals.
    fn linked_column_observes_random_nodes_and_replaces_scalar_literals() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, NodeIR::Scalar { id: 1, value: 0.0 });
        graph.nodes.insert(2, NodeIR::Scalar { id: 2, value: 1.0 });
        graph.nodes.insert(
            3,
            NodeIR::Random {
                id: 3,
                label: None,
                dist_type: "Normal".to_string(),
                params: vec![ParamIR { from_node: 1 }, ParamIR { from_node: 2 }],
            },
        );
        graph.nodes.insert(
            4,
            NodeIR::Scalar {
                id: 4,
                value: 999.0,
            },
        );
        graph.plates.insert(
            10,
            PlateIR {
                id: 10,
                n: 2,
                nodes: vec![3, 4],
                plates: Vec::new(),
                data: HashMap::from([("x".to_string(), vec![1.25, -0.5])]),
                mapping: HashMap::from([(3, "x".to_string()), (4, "x".to_string())]),
            },
        );

        let values = graph.ancestral_sample().unwrap();
        let expected =
            ModelResult::Plate(vec![ModelResult::Scalar(1.25), ModelResult::Scalar(-0.5)]);

        assert_eq!(values[&3], expected);
        assert_eq!(values[&4], expected);
    }

    #[test]
    fn bind_debug_string_presents_plated_observations_as_fugue_code() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, NodeIR::Scalar { id: 1, value: 0.0 });
        graph.nodes.insert(2, NodeIR::Scalar { id: 2, value: 1.0 });
        graph.nodes.insert(
            3,
            NodeIR::Random {
                id: 3,
                label: Some("x".into()),
                dist_type: "Normal".into(),
                params: vec![ParamIR { from_node: 1 }, ParamIR { from_node: 2 }],
            },
        );
        graph.plates.insert(
            10,
            PlateIR {
                id: 10,
                n: 2,
                nodes: vec![3],
                plates: Vec::new(),
                data: HashMap::from([("x".to_string(), vec![1.25, -0.5])]),
                mapping: HashMap::from([(3, "x".to_string())]),
            },
        );

        let code = graph.bind_debug_string().unwrap();

        assert!(code.contains("let observed_node_3 = data.column(\"x\"); // plate 10"));
        assert!(code.contains("let model = prob! {"));
        assert!(code.contains("let node_3 <- plate!(i_10 in 0..2 => {"));
        assert!(code.contains("observe("));
        assert!(code.contains("Normal::new(node_1, node_2).unwrap()"));
        assert!(code.contains("observed_node_3[i_10]"));
        assert!(code.contains("pure((node_1, node_2, node_3))"));
    }

    #[test]
    fn one_two_and_three_dimensional_shapes_are_cartesian_products() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, NodeIR::Scalar { id: 1, value: 7.0 });
        graph.nodes.insert(2, NodeIR::Scalar { id: 2, value: 8.0 });
        graph.nodes.insert(3, NodeIR::Scalar { id: 3, value: 9.0 });
        graph.plates.insert(10, empty_plate(10, 2, vec![1, 2, 3]));
        graph.plates.insert(20, empty_plate(20, 3, vec![2, 3]));
        graph.plates.insert(30, empty_plate(30, 4, vec![3]));
        let values = graph.ancestral_sample().unwrap();

        fn scalars(value: &ModelResult) -> usize {
            match value {
                ModelResult::Scalar(_) => 1,
                ModelResult::Plate(items) => items.iter().map(scalars).sum(),
            }
        }
        assert_eq!(scalars(&values[&1]), 2);
        assert_eq!(scalars(&values[&2]), 6);
        assert_eq!(scalars(&values[&3]), 24);
        assert_eq!(
            instance_address(3, &[10, 20, 30], &[1, 2, 3]),
            "node#3/plate#10[1]/plate#20[2]/plate#30[3]"
        );
    }

    #[test]
    fn observation_owner_broadcasts_across_additional_dimensions() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, NodeIR::Scalar { id: 1, value: 99.0 });
        let mut owner = empty_plate(10, 2, vec![1]);
        owner.data.insert("x".into(), vec![4.0, 5.0]);
        owner.mapping.insert(1, "x".into());
        graph.plates.insert(10, owner);
        graph.plates.insert(20, empty_plate(20, 3, vec![1]));

        let values = graph.ancestral_sample().unwrap();
        assert_eq!(
            values[&1],
            ModelResult::Plate(vec![
                ModelResult::Plate(vec![ModelResult::Scalar(4.0); 3]),
                ModelResult::Plate(vec![ModelResult::Scalar(5.0); 3]),
            ])
        );
    }

    #[test]
    fn parameter_projection_uses_plate_identity_not_prefix_depth() {
        let param = CompiledParam {
            from_node: 1,
            consumer_positions: vec![1],
            producer_plate_ids: vec![20],
        };
        let state = ExecutionState {
            values: HashMap::from([(
                InstanceKey {
                    node_id: 1,
                    indices: vec![2],
                },
                42.0,
            )]),
        };
        assert_eq!(
            resolve_params(&[param], &[7, 2], &state).unwrap(),
            vec![42.0]
        );
    }
}
