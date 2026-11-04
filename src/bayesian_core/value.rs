//! A node's value shape is independent of the dimensions introduced by plates.
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
            Self::RealVector(v) => ValueType::Vector {
                element: ScalarType::Real,
                length: v.len(),
            },
            Self::IntegerVector(v) => ValueType::Vector {
                element: ScalarType::Integer,
                length: v.len(),
            },
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Real(v) if !v.is_finite() => Err("value must be finite".into()),
            Self::RealVector(v) if v.is_empty() => {
                Err("vector must contain at least one element".into())
            }
            Self::IntegerVector(v) if v.is_empty() => {
                Err("vector must contain at least one element".into())
            }
            Self::RealVector(v) if v.iter().any(|x| !x.is_finite()) => {
                Err("vector elements must be finite".into())
            }
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
                if value.is_finite()
                    && value.fract() == 0.0
                    && value.abs() <= super::distributions::MAX_COUNT as f64 =>
            {
                Self::Integer(value as i64)
            }
            _ => return Err(format!("dataset value {value} cannot supply {expected}")),
        };
        result.validate()?;
        Ok(result)
    }
}
