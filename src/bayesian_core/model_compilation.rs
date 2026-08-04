use super::graph_checks::{GraphModel, ModelResult, ModelValues};
use super::plate_validation::{NormalizedPlates, NormalizedScope};
use super::{GraphIR, NodeIR, ParamIR};
use fugue::{
    Address, Beta, Distribution, Exponential, FugueResult, Gamma, LogNormal, Model, ModelExt,
    Normal, Uniform, pure,
};
use std::collections::HashMap;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct InstanceKey {
    node_id: u32,
    indices: Vec<usize>,
}

#[derive(Default)]
struct ExecutionState {
    values: HashMap<InstanceKey, f64>,
}

type ExecutionModel = Model<Result<ExecutionState, String>>;
type DynDistribution = Box<dyn Distribution<f64>>;

struct CompiledParam {
    from_node: u32,
    producer_depth: usize,
    producer_plate_ids: Vec<u32>,
}

struct NodeShape {
    node_id: u32,
    plate_ids: Vec<u32>,
    extents: Vec<usize>,
}

pub(crate) fn create_model(graph: &GraphIR) -> Result<GraphModel, String> {
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
    let mut model = pure(Ok(ExecutionState::default()));
    model = compile_scope(graph, &normalized, &normalized.root, &order, &[], model)?;

    let shapes = node_shapes(graph, &normalized)?;
    Ok(
        model
            .bind(move |result| pure(result.and_then(|state| materialize_values(&state, &shapes)))),
    )
}

//recursively compile the current plate's scope into a Fugue model (i.e. start at root, recurse on each nested plate)
fn compile_scope(
    graph: &GraphIR,
    normalized: &NormalizedPlates,
    scope: &NormalizedScope,
    order: &[u32],
    indices: &[usize],
    mut model: ExecutionModel,
) -> Result<ExecutionModel, String> {
    for &node_id in order {
        if scope.nodes.binary_search(&node_id).is_err() {
            continue;
        }

        let node = graph
            .nodes
            .get(&node_id)
            .cloned()
            .ok_or_else(|| format!("normalized scope references missing node {node_id}"))?;
        let plate_ids = normalized
            .node_paths
            .get(&node_id)
            .cloned()
            .ok_or_else(|| format!("node {node_id} has no normalized plate path"))?;
        let params = compiled_params(node_params(&node), &normalized.node_paths)?;
        let data_value = if let Some(plate) = scope.plate {
            let plate_ir = graph
                .plates
                .get(&plate.id)
                .ok_or_else(|| format!("normalized scope references missing plate {}", plate.id))?;

            if let Some(column) = plate_ir.mapping.get(&node_id) {
                let row = *indices
                    .last()
                    .ok_or_else(|| format!("plate {} node {node_id} has no row index", plate.id))?;
                Some(
                    *plate_ir
                        .data
                        .get(column)
                        .and_then(|values| values.get(row))
                        .ok_or_else(|| {
                            format!(
                                "plate {} column {column:?} has no value at row {row}",
                                plate.id
                            )
                        })?,
                )
            } else {
                None
            }
        } else {
            None
        };

        model = compile_node_instance(node, params, plate_ids, indices.to_vec(), data_value, model);
    }

    for child in &scope.children {
        let plate = child
            .plate
            .ok_or_else(|| "normalized child scope is missing its plate".to_string())?;

        for index in 0..plate.n {
            let mut child_indices = indices.to_vec();
            child_indices.push(index);
            model = compile_scope(graph, normalized, child, order, &child_indices, model)?;
        }
    }

    Ok(model)
}

//actually extend the model to include next dependent node by bind()ing
//creates distributions, compiles compute nodes, and places scalar values into model
fn compile_node_instance(
    node: NodeIR,
    params: Vec<CompiledParam>,
    plate_ids: Vec<u32>,
    indices: Vec<usize>,
    data_value: Option<f64>,
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

                if let Some(value) = data_value {
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

//formats node parameters that come from within a plate, i.e. must be duplicated
fn compiled_params(
    params: &[ParamIR],
    node_paths: &HashMap<u32, Vec<u32>>,
) -> Result<Vec<CompiledParam>, String> {
    params
        .iter()
        .map(|param| {
            let producer_depth = node_paths
                .get(&param.from_node)
                .ok_or_else(|| format!("parameter references missing node {}", param.from_node))?
                .len();
            Ok(CompiledParam {
                from_node: param.from_node,
                producer_depth,
                producer_plate_ids: node_paths[&param.from_node].clone(),
            })
        })
        .collect()
}

//actually handles plated parameters
fn resolve_params(
    params: &[CompiledParam],
    consumer_indices: &[usize],
    state: &ExecutionState,
) -> Result<Vec<f64>, String> {
    params
        .iter()
        .map(|param| {
            let producer_indices = consumer_indices
                .get(..param.producer_depth)
                .ok_or_else(|| {
                    format!(
                        "node {} requires a deeper plate scope than its consumer",
                        param.from_node
                    )
                })?
                .to_vec();
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

fn materialize_values(state: &ExecutionState, shapes: &[NodeShape]) -> Result<ModelValues, String> {
    let mut values = HashMap::with_capacity(shapes.len());
    for shape in shapes {
        let value = materialize_node(state, shape, 0, &mut Vec::new())?;
        values.insert(shape.node_id, value);
    }
    Ok(values)
}

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
    // pub fn ancestral_sample_debug(&self) -> Result<String, String> {
    //     let values = self.ancestral_sample()?;
    //     self.format_model_values(&values)
    // }

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

fn instance_address(node_id: u32, plate_ids: &[u32], indices: &[usize]) -> String {
    let mut address = format!("node#{node_id}");
    for (&plate_id, &index) in plate_ids.iter().zip(indices) {
        address.push_str(&format!("/plate#{plate_id}[{index}]"));
    }
    address
}

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

fn node_id(node: &NodeIR) -> u32 {
    match node {
        NodeIR::Random { id, .. } | NodeIR::Scalar { id, .. } | NodeIR::Compute { id, .. } => *id,
    }
}

fn node_params(node: &NodeIR) -> &[ParamIR] {
    match node {
        NodeIR::Random { params, .. } | NodeIR::Compute { params, .. } => params,
        NodeIR::Scalar { .. } => &[],
    }
}

fn boxed<D: Distribution<f64> + 'static>(
    result: FugueResult<D>,
) -> Result<DynDistribution, String> {
    result
        .map(|dist| Box::new(dist) as DynDistribution)
        .map_err(|error| error.to_string())
}

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

    #[test]
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
}
