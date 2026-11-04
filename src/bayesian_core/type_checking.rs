use super::distributions::DistributionKind;
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
                if *operation == crate::nodes::Operation::Index && inputs.len() == 2 {
                    if let ValueType::Vector { length, .. } = inputs[0] {
                        if let Some(Value::Integer(index)) = constants.get(&params[1].from_node) {
                            if *index < 0 || *index as usize >= length {
                                return Err(at("vector index is out of range".into()));
                            }
                        }
                        if let NodeIR::Random {
                            dist_type,
                            params: category_params,
                        } = &graph.nodes[&params[1].from_node]
                        {
                            if dist_type == "Categorical" {
                                if let ValueType::Vector {
                                    length: categories, ..
                                } = shapes[&category_params[0].from_node].value_type
                                {
                                    if categories > length {
                                        return Err(at(
                                            "category support exceeds the vector length".into(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
                for (index, actual) in inputs.iter().copied().enumerate() {
                    if *operation == crate::nodes::Operation::Collect
                        && !matches!(actual, ValueType::Scalar(_))
                    {
                        return Err(CompilationError {
                            parameter: Some(index),
                            expected: Some(ValueType::Scalar(actual.element())),
                            actual: Some(actual),
                            ..at(
                                "collect accepts scalar inputs; nested vectors are not supported"
                                    .into(),
                            )
                        });
                    }
                    if operation.requires_vector() && !matches!(actual, ValueType::Vector { .. }) {
                        return Err(CompilationError {
                            parameter: Some(index),
                            actual: Some(actual),
                            ..at(format!(
                                "{} requires a nonempty vector input",
                                operation.name()
                            ))
                        });
                    }
                }
                // Include expected/actual metadata for a length mismatch.
                if operation.is_binary()
                    && let Some((_, first)) = inputs
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
                    if !kind.valid_parameter(index, actual) {
                        return Err(CompilationError {
                            parameter: Some(index),
                            expected: match (dist_type.as_str(), index) {
                                ("Categorical", _) => None,
                                ("Binomial", 0) => Some(ValueType::INTEGER),
                                _ => Some(ValueType::REAL),
                            },
                            actual: Some(actual),
                            ..at(format!(
                                "{dist_type} parameter {} requires {}; got {actual}",
                                index + 1,
                                match (dist_type.as_str(), index) {
                                    ("Binomial", 0) => "an integer trial count",
                                    ("Categorical", _) =>
                                        "a probability vector with 1..200 elements",
                                    _ => "a real scalar",
                                }
                            ))
                        });
                    }
                }
                if dist_type == "Binomial" {
                    if let Some(n) = constants.get(&params[0].from_node) {
                        kind.create(&[n.clone(), Value::Real(0.5)]).map_err(at)?;
                    }
                    if let Some(p) = constants.get(&params[1].from_node) {
                        let p = p.real_scalar().map_err(at)?;
                        if !(0.0..=1.0).contains(&p) {
                            return Err(at("Binomial probability must be in [0,1]".into()));
                        }
                    }
                }
                if let Some(values) = params
                    .iter()
                    .map(|p| constants.get(&p.from_node))
                    .collect::<Option<Vec<_>>>()
                {
                    kind.create(&values.into_iter().cloned().collect::<Vec<_>>())
                        .map_err(at)?;
                }
                (kind.output_type(), None)
            }
        };
        if let Some(owner) = normalized.observation_owners.get(&id) {
            let plate = &graph.plates[owner];
            for (row, &raw) in plate.data[&plate.mapping[&id]].iter().enumerate() {
                if let NodeIR::Random { dist_type, params } = node {
                    if (dist_type == "Categorical" || dist_type == "Binomial") && raw < 0. {
                        return Err(at(format!(
                            "plate {owner} row {row}: counts/categories must be nonnegative"
                        )));
                    }
                    if dist_type == "Categorical" {
                        if let ValueType::Vector { length, .. } =
                            shapes[&params[0].from_node].value_type
                        {
                            if raw >= length as f64 {
                                return Err(at(format!(
                                    "plate {owner} row {row}: category must be in 0..{}",
                                    length - 1
                                )));
                            }
                        }
                    }
                    if dist_type == "Binomial" {
                        if let Some(Value::Integer(n)) = constants.get(&params[0].from_node) {
                            if raw > *n as f64 {
                                return Err(at(format!(
                                    "plate {owner} row {row}: count exceeds trials"
                                )));
                            }
                        }
                    }
                }
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
