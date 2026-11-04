//! Typed distribution construction and support checks shared by compilation and execution.
use super::{Value, ValueType};
use fugue::{
    Beta, Binomial, Categorical, Distribution, Exponential, Gamma, LogNormal, Normal, Uniform,
};
pub const MAX_COUNT: i64 = 9_007_199_254_740_991;
pub const MAX_CATEGORIES: usize = 200;

pub(super) enum RuntimeDistribution {
    Real(Box<dyn Distribution<f64>>),
    Count(Box<dyn Distribution<u64>>),
    Category(Box<dyn Distribution<usize>>),
}
impl Clone for RuntimeDistribution {
    fn clone(&self) -> Self {
        match self {
            Self::Real(d) => Self::Real(d.clone_box()),
            Self::Count(d) => Self::Count(d.clone_box()),
            Self::Category(d) => Self::Category(d.clone_box()),
        }
    }
}
impl RuntimeDistribution {
    pub fn accepts(&self, value: &Value) -> bool {
        match (self, value) {
            (Self::Real(_), Value::Real(_)) => true,
            (Self::Count(d), Value::Integer(v)) if *v >= 0 => d.log_prob(&(*v as u64)).is_finite(),
            (Self::Category(d), Value::Integer(v)) if *v >= 0 => {
                d.log_prob(&(*v as usize)).is_finite()
            }
            _ => false,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) enum DistributionKind {
    Normal,
    Uniform,
    Beta,
    Exponential,
    Gamma,
    LogNormal,
    Binomial,
    Categorical,
}
impl DistributionKind {
    pub fn parse(name: &str, arity: usize) -> Result<Self, String> {
        match (name, arity) {
            ("Normal", 2) => Ok(Self::Normal),
            ("Uniform", 2) => Ok(Self::Uniform),
            ("Beta", 2) => Ok(Self::Beta),
            ("Exponential", 1) => Ok(Self::Exponential),
            ("Gamma", 2) => Ok(Self::Gamma),
            ("LogNormal", 2) => Ok(Self::LogNormal),
            ("Binomial", 2) => Ok(Self::Binomial),
            ("Categorical", 1) => Ok(Self::Categorical),
            _ => Err(format!(
                "wrong number of parameters or unsupported distribution {name}"
            )),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Uniform => "Uniform",
            Self::Beta => "Beta",
            Self::Exponential => "Exponential",
            Self::Gamma => "Gamma",
            Self::LogNormal => "LogNormal",
            Self::Binomial => "Binomial",
            Self::Categorical => "Categorical",
        }
    }
    pub fn output_type(self) -> ValueType {
        match self {
            Self::Binomial | Self::Categorical => ValueType::INTEGER,
            _ => ValueType::REAL,
        }
    }
    pub fn valid_parameter(self, index: usize, ty: ValueType) -> bool {
        match (self, index) {
            (Self::Binomial, 0) => ty == ValueType::INTEGER,
            (Self::Categorical, 0) => matches!(
                ty,
                ValueType::Vector {
                    length: 1..=MAX_CATEGORIES,
                    ..
                }
            ),
            _ => matches!(ty, ValueType::Scalar(_)),
        }
    }
    pub fn create(self, values: &[Value]) -> Result<RuntimeDistribution, String> {
        for value in values {
            value.validate()?;
        }
        if let Self::Categorical = self {
            let probabilities = match &values[0] {
                Value::RealVector(v) => v.to_vec(),
                Value::IntegerVector(v) => v.iter().map(|x| *x as f64).collect(),
                _ => return Err("Categorical requires a probability vector".into()),
            };
            if probabilities.is_empty()
                || probabilities.len() > MAX_CATEGORIES
                || probabilities.iter().any(|p| *p < 0.)
            {
                return Err(
                    "Categorical needs 1..200 finite nonnegative weights with a positive total"
                        .into(),
                );
            }
            // Scale before summing so even very large finite weights normalize safely.
            let max = probabilities.iter().copied().fold(0., f64::max);
            if max == 0. {
                return Err("Categorical weights must have a positive total".into());
            }
            let scaled_sum: f64 = probabilities.iter().map(|p| p / max).sum();
            return Categorical::new(
                probabilities
                    .iter()
                    .map(|p| (p / max) / scaled_sum)
                    .collect(),
            )
            .map(|d| RuntimeDistribution::Category(Box::new(d)))
            .map_err(|e| e.to_string());
        }
        if let Self::Binomial = self {
            let Value::Integer(n) = values[0] else {
                return Err("Binomial trials must be an integer scalar".into());
            };
            if !(0..=1_000_000).contains(&n) {
                return Err("Binomial trials must be in 0..=1000000".to_string());
            }
            let p = values[1].real_scalar()?;
            let inner = Binomial::new(n as u64, p).map_err(|e| e.to_string())?;
            return Ok(RuntimeDistribution::Count(Box::new(StableBinomial {
                inner,
                n: n as u64,
                p,
            })));
        }
        let a = values[0].real_scalar()?;
        let b = values
            .get(1)
            .map(Value::real_scalar)
            .transpose()?
            .unwrap_or(0.);
        macro_rules! real {
            ($e:expr) => {
                $e.map(|d| RuntimeDistribution::Real(Box::new(d)))
                    .map_err(|e| e.to_string())
            };
        }
        match self {
            Self::Normal => real!(Normal::new(a, b)),
            Self::Uniform => real!(Uniform::new(a, b)),
            Self::Beta => real!(Beta::new(a, b)),
            Self::Exponential => real!(Exponential::new(a)),
            Self::Gamma => real!(Gamma::new(a, b)),
            Self::LogNormal => real!(LogNormal::new(a, b)),
            _ => unreachable!(),
        }
    }
}
#[derive(Clone)]
struct StableBinomial {
    inner: Binomial,
    n: u64,
    p: f64,
}
impl Distribution<u64> for StableBinomial {
    fn sample(&self, rng: &mut dyn rand::RngCore) -> u64 {
        self.inner.sample(rng)
    }
    fn log_prob(&self, k: &u64) -> f64 {
        if *k > self.n {
            return f64::NEG_INFINITY;
        }
        if self.n == 0 || self.p == 0. {
            return if *k == 0 { 0. } else { f64::NEG_INFINITY };
        }
        if self.p == 1. {
            return if *k == self.n { 0. } else { f64::NEG_INFINITY };
        }
        self.inner.log_prob(k)
    }
    fn clone_box(&self) -> Box<dyn Distribution<u64>> {
        Box::new(self.clone())
    }
}
