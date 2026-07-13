pub mod random_var;
use fugue::Distribution;
pub use random_var::*;

use bevy::prelude::*;

//on all node entities
#[derive(Component)]
pub struct GraphNode(pub u32);

#[derive(Component)]
pub struct NodeLabel(String);

pub enum Operation{
    Add,
    Sub,
    Mul,
    Div,
    Exp,
    Log,
    Pow,
    Sum,
    Product
}

#[derive(Debug)]
pub struct ParamValue (pub &'static str, pub f64);          //TODO: change from f64 to GraphLink

pub trait DistributionDebug<T>: Distribution<T> + std::fmt::Debug {}
impl<T, D: Distribution<T> + std::fmt::Debug> DistributionDebug<T> for D {}

pub trait NodeDisplay{
    fn label(&self) -> String;
}

//on random variable nodes
#[derive(Component)]
pub struct RandomNode{
    pub name: Option<String>,
    pub dist_type: String,
    pub dist: Box<dyn DistributionDebug<f64>>,
    pub params: Vec<ParamValue>
}
impl NodeDisplay for RandomNode{
    fn label(&self) -> String{
        format!["{}{}", match self.name.clone() {
            Some(n) => n + " ~ ",
            None => "var ~ ".to_string()
        }, self.dist_type]
    }
}

#[derive(Component)]
pub struct ComputeNode{
    pub operation: Operation,
    pub params: Vec<ParamValue>
}

impl NodeDisplay for ComputeNode{
    fn label(&self) -> String{
        match self.operation{
            Operation::Add => "+".to_string(),
            Operation::Sub => "-".to_string(),
            Operation::Mul => "*".to_string(),
            Operation::Div => "/".to_string(),
            Operation::Exp => "exp".to_string(),
            Operation::Log => "log".to_string(),
            Operation::Pow => "^".to_string(),
            Operation::Sum => "∑".to_string(),
            Operation::Product => "prod".to_string()
        }
    }
}

#[derive(Component)]
pub struct ScalarNode{
    pub val: f64
}

impl NodeDisplay for ScalarNode{
    fn label(&self) -> String{
        format!["{:.2}", self.val]
    }
}