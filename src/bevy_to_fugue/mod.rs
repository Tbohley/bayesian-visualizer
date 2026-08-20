pub mod compilation;
use bevy::{prelude::*, tasks::Task};
use crate::bayesian_core::{
    graph_checks::ModelValues, CompiledGraph, ControlledInferenceResult, InferenceResult,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize},
};


#[derive(Resource)]
pub struct GraphIRResource(pub CompiledGraph);

#[derive(Resource)]
pub struct InferenceResultResource(pub InferenceResult);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InferenceResultState {
    Running,
    Complete,
    Cancelled,
    Failed,
}

/// Presentation metadata that remains available after a partial run is kept.
#[derive(Resource)]
pub struct InferenceStatusResource {
    pub state: InferenceResultState,
    pub requested_samples: usize,
}

pub struct InferenceControl {
    pub cancel_requested: AtomicBool,
    pub discard_result: AtomicBool,
    pub warmup_completed: AtomicUsize,
    pub samples_completed: AtomicUsize,
    pub warmup_diagnostic_ready: AtomicBool,
    pub warmup_negative_infinity: AtomicBool,
    pub warmup_warning_emitted: AtomicBool,
    pub pending_draws: Mutex<Vec<ModelValues>>,
}

impl InferenceControl {
    pub fn new() -> Self {
        Self {
            cancel_requested: AtomicBool::new(false),
            discard_result: AtomicBool::new(false),
            warmup_completed: AtomicUsize::new(0),
            samples_completed: AtomicUsize::new(0),
            warmup_diagnostic_ready: AtomicBool::new(false),
            warmup_negative_infinity: AtomicBool::new(false),
            warmup_warning_emitted: AtomicBool::new(false),
            pending_draws: Mutex::new(Vec::new()),
        }
    }
}

#[derive(Resource)]
pub struct InferenceJob {
    pub task: Task<Result<ControlledInferenceResult, String>>,
    pub control: Arc<InferenceControl>,
    pub seed: u64,
    pub requested_samples: usize,
    pub requested_warmup: usize,
}

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
