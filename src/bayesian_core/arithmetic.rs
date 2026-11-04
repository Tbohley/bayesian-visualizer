//! Shape rules and evaluation for deterministic scalar/vector arithmetic.
use super::{ScalarType, Value, ValueType};
use crate::nodes::Operation;

impl Operation {
    pub(crate) fn output_type(&self, inputs: &[ValueType]) -> Result<ValueType, String> {
        if *self == Self::Index {
            return match inputs {
                [
                    ValueType::Vector { element, length },
                    ValueType::Scalar(ScalarType::Integer),
                ] if *length > 0 => Ok(ValueType::Scalar(*element)),
                _ => Err("Get element requires a vector and an integer scalar index".into()),
            };
        }
        if *self == Self::Collect {
            if inputs.is_empty() {
                return Err("collect requires at least one scalar input".into());
            }
            if inputs
                .iter()
                .any(|input| !matches!(input, ValueType::Scalar(_)))
            {
                return Err(
                    "collect accepts scalar inputs; nested vectors are not supported".into(),
                );
            }
            return Ok(ValueType::Vector {
                element: if inputs.iter().all(|input| *input == ValueType::INTEGER) {
                    ScalarType::Integer
                } else {
                    ScalarType::Real
                },
                length: inputs.len(),
            });
        }
        let arity = if self.is_binary() { 2 } else { 1 };
        if inputs.len() != arity {
            return Err(format!(
                "operation expected {arity} parameters, got {}",
                inputs.len()
            ));
        }
        if self.requires_vector() {
            return match inputs[0] {
                ValueType::Vector { element, length } if length > 0 => Ok(match self {
                    Self::Sum | Self::Product => ValueType::Scalar(element),
                    _ => ValueType::Vector {
                        element: ScalarType::Real,
                        length,
                    },
                }),
                _ => Err(format!("{} requires a nonempty vector input", self.name())),
            };
        }
        let mut length = None;
        for input in inputs {
            if let ValueType::Vector { length: n, .. } = *input {
                if n == 0 {
                    return Err("vector must contain at least one element".into());
                }
                if length.is_some_and(|previous| previous != n) {
                    return Err("elementwise operations require matching vector lengths".into());
                }
                length = Some(n);
            }
        }
        let element = if matches!(self, Self::Add | Self::Subtract | Self::Multiply)
            && inputs
                .iter()
                .all(|input| input.element() == ScalarType::Integer)
        {
            ScalarType::Integer
        } else {
            ScalarType::Real
        };
        Ok(match length {
            Some(length) => ValueType::Vector { element, length },
            None => ValueType::Scalar(element),
        })
    }

    /// Scalar kernel for elementwise operations. Vector-wide operations are
    /// evaluated separately so they never reduce or normalize plate axes.
    pub fn evaluate(&self, params: &[f64]) -> Result<f64, String> {
        let arity = if self.is_binary() { 2 } else { 1 };
        if params.len() != arity {
            return Err(format!("operation expected {arity} parameters"));
        }
        if params.iter().any(|value| !value.is_finite()) {
            return Err("operation inputs must be finite".into());
        }
        let a = params[0];
        let result = match self {
            Self::Add => a + params[1],
            Self::Subtract => a - params[1],
            Self::Multiply => a * params[1],
            Self::Divide if params[1] == 0.0 => return Err("division by zero".into()),
            Self::Divide => a / params[1],
            Self::Power => a.powf(params[1]),
            Self::Exponential => a.exp(),
            Self::Logarithm if a <= 0.0 => return Err("logarithm input must be positive".into()),
            Self::Logarithm => a.ln(),
            Self::Logit if a <= 0.0 || a >= 1.0 => {
                return Err("logit input must be strictly between 0 and 1".into());
            }
            Self::Logit => a.ln() - (-a).ln_1p(),
            Self::InverseLogit if a >= 0.0 => 1.0 / (1.0 + (-a).exp()),
            Self::InverseLogit => {
                let e = a.exp();
                e / (1.0 + e)
            }
            _ => return Err("operation requires vector evaluation".into()),
        };
        if !result.is_finite() {
            return Err(format!("{} produced a non-finite result", self.name()));
        }
        Ok(result)
    }

    pub(crate) fn evaluate_values(&self, inputs: &[Value]) -> Result<Value, String> {
        let output = self.output_type(&inputs.iter().map(Value::value_type).collect::<Vec<_>>())?;
        for input in inputs {
            input.validate()?;
        }
        let result = match self {
            Self::Index => {
                let Value::Integer(index) = inputs[1] else {
                    unreachable!()
                };
                if index < 0 {
                    return Err("vector index must be nonnegative".into());
                }
                match &inputs[0] {
                    Value::RealVector(v) => v.get(index as usize).copied().map(Value::Real),
                    Value::IntegerVector(v) => v.get(index as usize).copied().map(Value::Integer),
                    _ => unreachable!(),
                }
                .ok_or_else(|| "vector index is out of range".to_string())?
            }
            Self::Collect if output.element() == ScalarType::Integer => Value::IntegerVector(
                inputs
                    .iter()
                    .map(|value| match value {
                        Value::Integer(value) => *value,
                        _ => unreachable!("integer scalar inputs were validated"),
                    })
                    .collect::<Vec<_>>()
                    .into(),
            ),
            Self::Collect => Value::RealVector(
                inputs
                    .iter()
                    .map(Value::real_scalar)
                    .collect::<Result<Vec<_>, _>>()?
                    .into(),
            ),
            Self::Sum | Self::Product => reduce(*self, &inputs[0])?,
            Self::Normalize | Self::Softmax => probabilities(*self, &inputs[0])?,
            _ => elementwise(*self, inputs, output)?,
        };
        result.validate()?;
        Ok(result)
    }
}

fn reduce(operation: Operation, input: &Value) -> Result<Value, String> {
    match input {
        Value::IntegerVector(values) => {
            let initial = if operation == Operation::Sum {
                0_i64
            } else {
                1_i64
            };
            values
                .iter()
                .try_fold(initial, |acc, value| {
                    if operation == Operation::Sum {
                        acc.checked_add(*value)
                    } else {
                        acc.checked_mul(*value)
                    }
                    .ok_or_else(|| format!("integer {} overflow", operation.name().to_lowercase()))
                })
                .map(Value::Integer)
        }
        Value::RealVector(values) => Ok(Value::Real(if operation == Operation::Sum {
            values.iter().sum()
        } else {
            values.iter().product()
        })),
        _ => unreachable!("vector input was validated"),
    }
}

fn probabilities(operation: Operation, input: &Value) -> Result<Value, String> {
    let values = match input {
        Value::RealVector(values) => values.to_vec(),
        Value::IntegerVector(values) => values.iter().map(|value| *value as f64).collect(),
        _ => unreachable!("vector input was validated"),
    };
    if operation == Operation::Normalize {
        if let Some(index) = values.iter().position(|value| *value < 0.0) {
            return Err(format!("weight {} must be nonnegative", index + 1));
        }
    }
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if operation == Operation::Normalize && max == 0.0 {
        return Err("weights must have a positive sum".into());
    }
    // Scaling weights by their maximum avoids overflowing their sum. Softmax
    // subtracts the maximum before exponentiation for the same reason.
    let scaled: Vec<_> = values
        .iter()
        .map(|value| {
            if operation == Operation::Normalize {
                value / max
            } else {
                (value - max).exp()
            }
        })
        .collect();
    let sum: f64 = scaled.iter().sum();
    Ok(Value::RealVector(
        scaled
            .into_iter()
            .map(|value| value / sum)
            .collect::<Vec<_>>()
            .into(),
    ))
}

fn elementwise(operation: Operation, inputs: &[Value], output: ValueType) -> Result<Value, String> {
    let length = match output {
        ValueType::Scalar(_) => 1,
        ValueType::Vector { length, .. } => length,
    };
    if output.element() == ScalarType::Integer {
        let integer_at = |value: &Value, index| match value {
            Value::Integer(value) => *value,
            Value::IntegerVector(values) => values[index],
            _ => unreachable!("integer inputs were validated"),
        };
        let values = (0..length)
            .map(|index| {
                let a = integer_at(&inputs[0], index);
                let b = integer_at(&inputs[1], index);
                match operation {
                    Operation::Add => a.checked_add(b),
                    Operation::Subtract => a.checked_sub(b),
                    Operation::Multiply => a.checked_mul(b),
                    _ => unreachable!(),
                }
                .ok_or_else(|| "integer arithmetic overflow".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(match output {
            ValueType::Scalar(_) => Value::Integer(values[0]),
            _ => Value::IntegerVector(values.into()),
        })
    } else {
        let real_at = |value: &Value, index| match value {
            Value::Real(value) => *value,
            Value::Integer(value) => *value as f64,
            Value::RealVector(values) => values[index],
            Value::IntegerVector(values) => values[index] as f64,
        };
        let values = (0..length)
            .map(|index| {
                let mut parameters = [0.0; 2];
                for (slot, input) in parameters.iter_mut().zip(inputs) {
                    *slot = real_at(input, index);
                }
                operation.evaluate(&parameters[..inputs.len()])
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(match output {
            ValueType::Scalar(_) => Value::Real(values[0]),
            _ => Value::RealVector(values.into()),
        })
    }
}

#[cfg(test)]
#[path = "tests/arithmetic.rs"]
mod tests;
