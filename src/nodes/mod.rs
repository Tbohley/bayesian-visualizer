pub mod random_var;
use fugue::Distribution;
pub use random_var::*;

use bevy::prelude::*;

//on all node entities
#[derive(Component)]
pub struct GraphNode(pub u32);

//on the text child entity of a named node
#[derive(Component)]
pub struct NamedNode(pub String);

//on the text child of a default node
#[derive(Component)]    
pub struct UnnamedNode;

#[derive(Debug)]
pub struct ParamValue (pub &'static str, pub f64);


pub trait DistributionDebug<T>: Distribution<T> + std::fmt::Debug {}
impl<T, D: Distribution<T> + std::fmt::Debug> DistributionDebug<T> for D {}

//on random variable nodes
#[derive(Component)]
pub struct RandomVar{
    pub dist_type: String,
    pub dist: Box<dyn DistributionDebug<f64>>,
    pub params: Vec<ParamValue>
}