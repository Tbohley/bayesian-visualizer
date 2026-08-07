pub mod compilation;
use bevy::prelude::*;
use crate::bayesian_core::{CompiledGraph, InferenceResult};


#[derive(Resource)]
pub struct GraphIRResource(pub CompiledGraph);

#[derive(Resource)]
pub struct InferenceResultResource(pub InferenceResult);

#[derive(Event)]
pub struct TriggerCompilation;

#[derive(Event)]
pub struct SampleDisplay{
    pos: Vec2,
    val: String,
    console_output: Option<String>,
}

#[derive(Component)]
pub struct SamplePopup{
    pub timer: Timer,
    pub console_output: Option<String>,
}
