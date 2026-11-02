//! A node's value shape is independent of the dimensions introduced by plates.
use crate::nodes::Operation;
use std::{fmt, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarType {
    Real,
    Integer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueType {
    Scalar(ScalarType),
    Vector { element: ScalarType, length: usize },
}

impl ValueType {
    pub const REAL: Self = Self::Scalar(ScalarType::Real);
    pub const INTEGER: Self = Self::Scalar(ScalarType::Integer);

    pub fn element(self) -> ScalarType {
        match self {
            Self::Scalar(element) | Self::Vector { element, .. } => element,
        }
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let element = match self.element() {
            ScalarType::Real => "real",
            ScalarType::Integer => "integer",
        };
        match self {
            Self::Scalar(_) => write!(f, "{element} scalar"),
            Self::Vector { length, .. } => write!(f, "{element} vector[{length}]"),
        }
    }
}

/// Nested vectors cannot be represented. Integers remain exact until widened
/// for a real-valued operation (which uses ordinary IEEE-754 conversion).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Real(f64),
    Integer(i64),
    RealVector(Arc<[f64]>),
    IntegerVector(Arc<[i64]>),
}

impl Value {
    pub fn value_type(&self) -> ValueType {
        match self {
            Self::Real(_) => ValueType::REAL,
            Self::Integer(_) => ValueType::INTEGER,
            Self::RealVector(v) => ValueType::Vector { element: ScalarType::Real, length: v.len() },
            Self::IntegerVector(v) => ValueType::Vector { element: ScalarType::Integer, length: v.len() },
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Real(v) if !v.is_finite() => Err("value must be finite".into()),
            Self::RealVector(v) if v.is_empty() => Err("vector must contain at least one element".into()),
            Self::IntegerVector(v) if v.is_empty() => Err("vector must contain at least one element".into()),
            Self::RealVector(v) if v.iter().any(|x| !x.is_finite()) => Err("vector elements must be finite".into()),
            _ => Ok(()),
        }
    }

    /// Only scalar values can be widened for the existing scalar distributions.
    pub fn real_scalar(&self) -> Result<f64, String> {
        match self {
            Self::Real(v) => Ok(*v),
            Self::Integer(v) => Ok(*v as f64),
            _ => Err(format!("expected real scalar, got {}", self.value_type())),
        }
    }

    /// Dataset columns are scalar f64s today. Never silently truncate a value
    /// when a column supplies an explicitly integer node.
    pub(crate) fn observation(value: f64, expected: ValueType) -> Result<Self, String> {
        let result = match expected {
            ValueType::Scalar(ScalarType::Real) => Self::Real(value),
            ValueType::Scalar(ScalarType::Integer)
                if value.is_finite() && value.fract() == 0.0
                    && value >= i64::MIN as f64 && value < -(i64::MIN as f64) => Self::Integer(value as i64),
            _ => return Err(format!("dataset value {value} cannot supply {expected}")),
        };
        result.validate()?;
        Ok(result)
    }
}

impl Operation {
    pub(crate) fn output_type(&self, inputs: &[ValueType]) -> Result<ValueType, String> {
        let arity = match self {
            Self::Exponential | Self::Logarithm => 1,
            Self::Sum | Self::Product => return Err("vector reductions are not implemented yet".into()),
            _ => 2,
        };
        if inputs.len() != arity {
            return Err(format!("operation expected {arity} parameters, got {}", inputs.len()));
        }
        let mut length = None;
        for input in inputs {
            if let ValueType::Vector { length: n, .. } = *input {
                if n == 0 { return Err("vector must contain at least one element".into()); }
                if length.is_some_and(|previous| previous != n) {
                    return Err("elementwise operations require matching vector lengths".into());
                }
                length = Some(n);
            }
        }
        let element = if matches!(self, Self::Add | Self::Subtract | Self::Multiply)
            && inputs.iter().all(|input| input.element() == ScalarType::Integer) {
            ScalarType::Integer
        } else {
            ScalarType::Real
        };
        Ok(match length {
            Some(length) => ValueType::Vector { element, length },
            None => ValueType::Scalar(element),
        })
    }

    pub(crate) fn evaluate_values(&self, inputs: &[Value]) -> Result<Value, String> {
        let output = self.output_type(&inputs.iter().map(Value::value_type).collect::<Vec<_>>())?;
        for input in inputs { input.validate()?; }
        let integer_at = |value: &Value, index| match value {
            Value::Integer(v) => *v,
            Value::IntegerVector(v) => v[index],
            _ => unreachable!("integer output requires integer inputs"),
        };
        let real_at = |value: &Value, index| match value {
            Value::Real(v) => *v,
            Value::Integer(v) => *v as f64,
            Value::RealVector(v) => v[index],
            Value::IntegerVector(v) => v[index] as f64,
        };
        let length = match output { ValueType::Scalar(_) => 1, ValueType::Vector { length, .. } => length };
        let result = if output.element() == ScalarType::Integer {
            let values = (0..length).map(|index| {
                let a = integer_at(&inputs[0], index);
                let b = integer_at(&inputs[1], index);
                match self {
                    Self::Add => a.checked_add(b),
                    Self::Subtract => a.checked_sub(b),
                    Self::Multiply => a.checked_mul(b),
                    _ => unreachable!(),
                }.ok_or_else(|| "integer arithmetic overflow".to_string())
            }).collect::<Result<Vec<_>, _>>()?;
            match output {
                ValueType::Scalar(_) => Value::Integer(values[0]),
                _ => Value::IntegerVector(values.into()),
            }
        } else {
            let values = (0..length).map(|index| {
                self.evaluate(&inputs.iter().map(|value| real_at(value, index)).collect::<Vec<_>>())
            }).collect::<Result<Vec<_>, _>>()?;
            match output {
                ValueType::Scalar(_) => Value::Real(values[0]),
                _ => Value::RealVector(values.into()),
            }
        };
        result.validate()?;
        Ok(result)
    }
}
