//! Arithmetic operation metadata shared by node rendering and the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Exponential,
    Logarithm,
    Power,
    Sum,
    Product,
    Logit,
    InverseLogit,
    Normalize,
    Softmax,
    Collect,
    Index,
}

impl Operation {
    pub const ALL: &[Self] = &[
        Self::Add,
        Self::Subtract,
        Self::Multiply,
        Self::Divide,
        Self::Power,
        Self::Exponential,
        Self::Logarithm,
        Self::Logit,
        Self::InverseLogit,
        Self::Sum,
        Self::Product,
        Self::Normalize,
        Self::Softmax,
        Self::Collect,
        Self::Index,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::Multiply => "Multiply",
            Self::Divide => "Divide",
            Self::Power => "Power",
            Self::Exponential => "Exp",
            Self::Logarithm => "Log",
            Self::Logit => "Logit",
            Self::InverseLogit => "Inverse logit",
            Self::Sum => "Sum",
            Self::Product => "Product",
            Self::Normalize => "Normalize weights",
            Self::Softmax => "Softmax",
            Self::Collect => "Collect vector",
            Self::Index => "Get element",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Power => "^",
            Self::Exponential => "exp",
            Self::Logarithm => "log",
            Self::Logit => "logit",
            Self::InverseLogit => "sigm",
            Self::Sum => "sum",
            Self::Product => "prod",
            Self::Normalize => "norm",
            Self::Softmax => "softm",
            Self::Collect => "vec",
            Self::Index => "at",
        }
    }

    pub fn is_binary(self) -> bool {
        matches!(
            self,
            Self::Add | Self::Subtract | Self::Multiply | Self::Divide | Self::Power
        )
    }

    pub fn requires_vector(self) -> bool {
        matches!(
            self,
            Self::Sum | Self::Product | Self::Normalize | Self::Softmax
        )
    }
}
