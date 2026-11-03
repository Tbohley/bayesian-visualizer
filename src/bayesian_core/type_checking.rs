use super::execution::DistributionKind;
use super::plate_validation::NormalizedPlates;
use super::{GraphIR, NodeIR, Value, ValueType};
use std::{collections::HashMap, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeShape {
    pub value_type: ValueType,
    /// Plate axes describe independent instances, never vector components.
    pub plate_ids: Vec<u32>,
    pub plate_extents: Vec<usize>,
}

/// Parameter indices are zero-based, matching the node's parameter list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompilationError {
    pub node_ids: Vec<u32>,
    pub parameter: Option<usize>,
    pub expected: Option<ValueType>,
    pub actual: Option<ValueType>,
    pub message: String,
}

impl CompilationError {
    pub(crate) fn at(node_id: u32, message: impl Into<String>) -> Self {
        Self {
            node_ids: vec![node_id],
            parameter: None,
            expected: None,
            actual: None,
            message: message.into(),
        }
    }
}

impl From<String> for CompilationError {
    fn from(message: String) -> Self {
        Self {
            node_ids: Vec::new(),
            parameter: None,
            expected: None,
            actual: None,
            message,
        }
    }
}

impl fmt::Display for CompilationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(id) = self.node_ids.first() {
            write!(f, "node {id}")?;
            if let Some(parameter) = self.parameter {
                write!(f, " parameter {}", parameter + 1)?;
            }
            write!(f, ": ")?;
        }
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for CompilationError {}

pub(super) fn check_types(
    graph: &GraphIR,
    normalized: &NormalizedPlates,
    order: &[u32],
) -> Result<HashMap<u32, NodeShape>, CompilationError> {
    let mut shapes: HashMap<u32, NodeShape> = HashMap::new();
    let mut constants: HashMap<u32, Value> = HashMap::new();
    for &id in order {
        let at = |message| CompilationError::at(id, message);
        let node = &graph.nodes[&id];
        let (value_type, constant) = match node {
            NodeIR::Scalar { value } => {
                let value = Value::Real(*value);
                value.validate().map_err(at)?;
                (value.value_type(), Some(value))
            }
            NodeIR::Constant { value } => {
                value.validate().map_err(at)?;
                (value.value_type(), Some(value.clone()))
            }
            NodeIR::Compute { operation, params } => {
                let inputs: Vec<_> = params
                    .iter()
                    .map(|p| shapes[&p.from_node].value_type)
                    .collect();
                // Include expected/actual metadata for a length mismatch.
                if let Some((_, first)) = inputs
                    .iter()
                    .enumerate()
                    .find(|(_, ty)| matches!(ty, ValueType::Vector { .. }))
                {
                    for (index, actual) in inputs.iter().enumerate() {
                        if let (
                            ValueType::Vector { length: a, .. },
                            ValueType::Vector { length: b, .. },
                        ) = (first, actual)
                        {
                            if a != b {
                                return Err(CompilationError {
                                    parameter: Some(index),
                                    expected: Some(*first),
                                    actual: Some(*actual),
                                    ..at("elementwise operations require matching vector lengths"
                                        .into())
                                });
                            }
                        }
                    }
                }
                let output = operation.output_type(&inputs).map_err(at)?;
                let values = params
                    .iter()
                    .map(|p| constants.get(&p.from_node).cloned())
                    .collect::<Option<Vec<_>>>();
                let constant = values
                    .map(|values| operation.evaluate_values(&values))
                    .transpose()
                    .map_err(at)?;
                (output, constant)
            }
            NodeIR::Random { dist_type, params } => {
                let kind = DistributionKind::parse(dist_type, params.len()).map_err(at)?;
                for (index, param) in params.iter().enumerate() {
                    let actual = shapes[&param.from_node].value_type;
                    if !matches!(actual, ValueType::Scalar(_)) {
                        return Err(CompilationError {
                            parameter: Some(index),
                            expected: Some(ValueType::REAL),
                            actual: Some(actual),
                            ..at(format!("{dist_type} expects a real scalar, got {actual}"))
                        });
                    }
                }
                if let Some(values) = params
                    .iter()
                    .map(|p| constants.get(&p.from_node))
                    .collect::<Option<Vec<_>>>()
                {
                    let mut args = [0.0; 2];
                    for (index, value) in values.iter().enumerate() {
                        args[index] = value.real_scalar().map_err(at)?;
                    }
                    kind.create(args).map_err(at)?;
                }
                (ValueType::REAL, None)
            }
        };
        if let Some(owner) = normalized.observation_owners.get(&id) {
            let plate = &graph.plates[owner];
            for (row, &raw) in plate.data[&plate.mapping[&id]].iter().enumerate() {
                Value::observation(raw, value_type)
                    .map_err(|error| at(format!("plate {owner} row {row}: {error}")))?;
            }
            // Dataset values override literal values per instance.
        } else if let Some(value) = constant {
            constants.insert(id, value);
        }
        let plate_ids = normalized.node_paths[&id].clone();
        let plate_extents = plate_ids.iter().map(|p| normalized.extents[p]).collect();
        shapes.insert(
            id,
            NodeShape {
                value_type,
                plate_ids,
                plate_extents,
            },
        );
    }
    Ok(shapes)
}

#[cfg(test)]
#[path = "tests/type_checking.rs"]
mod tests;
