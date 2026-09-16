use super::*;
use crate::bayesian_core::graph_checks::ModelResult;
use crate::bayesian_core::*;
use crate::constants::*;
use crate::data_vis::{
    CloseHistogramPanel, HistogramSubject, HistogramView,
    JointDistributionView, OpenHistogramPanel, OpenJointDistributionView, PlateIndexScopes,
    SampleSelections,
};
use crate::graph::*;
use crate::nodes::*;
use crate::sidebar::SetInferenceControlsEnabled;
use crate::sidebar::SetPosteriorSampleEnabled;
use crate::sidebar::link_params::format_number;
use crate::sidebar::{
    InferenceProgressContainer, InferenceProgressFill, InferenceProgressLabel,
    InferenceRunButtonLabel, NumberOfSamplesTextbox, NumberOfWarmupTextbox, RandomSeedTextbox,
};
use crate::ui::{ErrorToast, ShowCompilationErrorMarkers};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, futures::check_ready};
use bevy::text::EditableText;
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, atomic::Ordering};

fn node_ids_in_compilation_error(error: &str) -> Vec<u32> {
    let mut ids = HashSet::new();
    for prefix in ["node#", "node "] {
        for (start, _) in error.match_indices(prefix) {
            let digits = error[start + prefix.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>();
            if let Ok(id) = digits.parse() {
                ids.insert(id);
            }
        }
    }
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort_unstable();
    ids
}

fn report_compilation_error(commands: &mut Commands, text: String, mut node_ids: Vec<u32>) {
    node_ids.extend(node_ids_in_compilation_error(&text));
    node_ids.sort_unstable();
    node_ids.dedup();
    commands.trigger(ErrorToast {
        color: ERR_COLOR,
        text,
    });
    if !node_ids.is_empty() {
        commands.trigger(ShowCompilationErrorMarkers { node_ids });
    }
}

pub fn compile(
    _event: On<TriggerCompilation>,
    mut commands: Commands,
    inference_job: Option<Res<InferenceJob>>,
    rand_nodes: Query<(Entity, &RandomNode), (Without<ComputeNode>, Without<ScalarNode>)>,
    compute_nodes: Query<(Entity, &ComputeNode), (Without<RandomNode>, Without<ScalarNode>)>,
    scalar_nodes: Query<(Entity, &ScalarNode), (Without<RandomNode>, Without<ComputeNode>)>,
    node_ids: Query<(Entity, &GraphNode)>,
    node_positions: Query<(&GraphNode, &Transform), Without<Plate>>,
    plates: Query<(&GraphNode, &Plate)>,
) {
    if let Some(job) = inference_job {
        job.control.discard_result.store(true, Ordering::Relaxed);
        job.control.cancel_requested.store(true, Ordering::Relaxed);
    }
    print_graph_preset(
        &rand_nodes,
        &compute_nodes,
        &scalar_nodes,
        &node_ids,
        &node_positions,
        &plates,
    );

    // Any compile attempt supersedes posterior results from the previous graph.
    commands.remove_resource::<InferenceResultResource>();
    commands.remove_resource::<InferenceStatusResource>();
    commands.remove_resource::<SampleSelections>();
    commands.remove_resource::<PlateIndexScopes>();
    commands.trigger(CloseHistogramPanel);
    commands.trigger(SetPosteriorSampleEnabled(false));
    let graph = compile_ir(
        &rand_nodes,
        &compute_nodes,
        &scalar_nodes,
        &node_ids,
        &node_positions,
        &plates,
    );

    match graph {
        Ok(g) => {
            if let Err(error) = g.validate_plate_semantics() {
                report_compilation_error(&mut commands, error.clone(), Vec::new());
                println!("{error}");
                commands.remove_resource::<GraphIRResource>();
                commands.trigger(SetInferenceControlsEnabled(false));
                return;
            }

            match g.check_cycles() {
                Ok(()) => {
                    commands.trigger(ErrorToast {
                        color: SAMPLE_COLOR,
                        text: String::from(
                            "Graph successfully compiled. No errors detected... yet.",
                        ),
                    });
                    //println!("Compiled plates: {:#?}", g.plates);
                    //save graph for other functions
                    match g.compile() {
                        Ok(compiled) => {
                            match compiled.bind_debug_string() {
                                Ok(code) => println!("Generated Fugue model:\n{code}"),
                                Err(error) => {
                                    println!("Could not render Fugue bind model: {error}")
                                }
                            }
                            commands.insert_resource(GraphIRResource(compiled));
                            commands.remove_resource::<InferenceResultResource>();
                            commands.trigger(SetInferenceControlsEnabled(true));
                        }
                        Err(error) => {
                            report_compilation_error(
                                &mut commands,
                                format!("Compilation error: {error}"),
                                Vec::new(),
                            );
                            commands.remove_resource::<GraphIRResource>();
                            commands.remove_resource::<InferenceResultResource>();
                            commands.trigger(SetInferenceControlsEnabled(false));
                        }
                    }
                }
                Err(node_ids) => {
                    report_compilation_error(
                        &mut commands,
                        format!("Graph contains a cycle including node IDs: {:?}", node_ids),
                        node_ids,
                    );
                    commands.remove_resource::<GraphIRResource>();
                    commands.trigger(SetInferenceControlsEnabled(false));
                }
            }
        }
        Err(error) => {
            report_compilation_error(&mut commands, error.clone(), Vec::new());
            println!("{error}");
            commands.remove_resource::<GraphIRResource>();
            commands.trigger(SetInferenceControlsEnabled(false));
        }
    };
}

pub fn global_sample(
    _event: On<Pointer<Click>>,
    mut commands: Commands,
    node_ids: Query<(Entity, &GraphNode, &Transform)>,
    graph_resource: Option<Res<GraphIRResource>>,
    old_samples: Query<(Entity, &SamplePopup)>,
){
    for samp in old_samples.iter(){
        commands.entity(samp.0).despawn();
    }
    let Some(compiled) = graph_resource else {
        commands.trigger(ErrorToast{
            text: "Graph not compiled.".to_string(),
            color: ERR_COLOR,
        });
        return;
    };
    let g = compiled.0.graph();
    let sample_res = g.ancestral_sample();

    let vals = match sample_res {
        Ok(values) => values,
        Err(error) => {
            commands.trigger(ErrorToast{
                text: format!("Sampling error: {error}"),
                color: ERR_COLOR,
            });
            return;
        }
    };

    display_sample(&mut commands, g, &vals, &node_ids, "Basic sample");
}

pub fn posterior_sample(
    _event: On<Pointer<Click>>,
    mut commands: Commands,
    graph_resource: Option<Res<GraphIRResource>>,
    inference_results: Option<Res<InferenceResultResource>>,
    node_ids: Query<(Entity, &GraphNode, &Transform)>,
    old_samples: Query<(Entity, &SamplePopup)>,
) {
    for (entity, _) in &old_samples {
        commands.entity(entity).despawn();
    }
    let (Some(compiled), Some(results)) = (graph_resource, inference_results) else {
        commands.trigger(ErrorToast {
            text: "Run inference before taking a posterior sample.".to_string(),
            color: ERR_COLOR,
        });
        return;
    };
    if results.0.traces.is_empty() {
        commands.trigger(ErrorToast {
            text: "Inference produced no posterior traces.".to_string(),
            color: ERR_COLOR,
        });
        return;
    }

    let mut rng = rand::thread_rng();
    let draw_index = rng.gen_range(0..results.0.traces.len());
    let values = match compiled
        .0
        .posterior_predictive_sample(&results.0.traces[draw_index])
    {
        Ok(values) => values,
        Err(error) => {
            commands.trigger(ErrorToast {
                text: format!("Posterior sampling error: {error}"),
                color: ERR_COLOR,
            });
            return;
        }
    };
    display_sample(
        &mut commands,
        compiled.0.graph(),
        &values,
        &node_ids,
        &format!("Posterior predictive sample from draw {}", draw_index + 1),
    );
}

fn display_sample(
    commands: &mut Commands,
    graph: &GraphIR,
    values: &HashMap<u32, ModelResult>,
    node_ids: &Query<(Entity, &GraphNode, &Transform)>,
    title: &str,
) {
    match graph.format_model_values(values) {
        Ok(output) => println!("{title}:\n{output}"),
        Err(error) => println!("Could not format sample: {error}"),
    }
    let order = graph
        .topological_sort()
        .expect("topological ordering should be validated by compilation");

    for node_id in order {
        let (_, _, transform) = node_ids
            .iter()
        .find(|(_, node, _)| node.0 == node_id)
        .expect("node not found");
        let value = values
            .get(&node_id)
            .expect("sampled node val doesn't exist");
        let console_output = match value {
            ModelResult::Scalar(_) => None,
            ModelResult::Plate(_) => Some(
                graph
                    .format_node_value(node_id, value)
                    .unwrap_or_else(|error| format!("Could not format node {node_id}: {error}")),
            ),
        };

        commands.trigger(SampleDisplay{
            pos: Vec2 {
                x: transform.translation.x,
                y: transform.translation.y,
            },
            val: first_scalar(value)
                .map(format_number)
                .unwrap_or_else(|| "empty".to_string()),
            console_output,
        })
    }
}

pub fn run_inference(
    _event: On<Pointer<Click>>,
    mut commands: Commands,
    inference_job: Option<Res<InferenceJob>>,
    graph_resource: Option<Res<GraphIRResource>>,
    seed_text: Single<&EditableText, With<RandomSeedTextbox>>,
    sample_text: Single<&EditableText, With<NumberOfSamplesTextbox>>,
    warmup_text: Single<&EditableText, With<NumberOfWarmupTextbox>>,
) {
    if let Some(job) = inference_job {
        if !job.control.cancel_requested.swap(true, Ordering::Relaxed) {
            commands.trigger(ErrorToast {
                text: "Stopping inference after the current MCMC step...".to_string(),
                color: SAMPLE_COLOR,
            });
        }
        return;
    }

    // Never leave a visualization of superseded posterior draws on screen.
    commands.trigger(CloseHistogramPanel);
    let Some(compiled) = graph_resource else {
        commands.trigger(ErrorToast {
            text: "Graph not compiled.".to_string(),
            color: ERR_COLOR,
        });
        return;
    };

    let seed_string = seed_text.value().to_string();
    let seed = if seed_string.trim().is_empty() {
        rand::random::<u64>()
    } else {
        match seed_string.trim().parse::<u64>() {
            Ok(seed) => seed,
            Err(_) => {
                commands.trigger(ErrorToast {
                    text: "Random seed must be a non-negative whole number or blank.".to_string(),
                    color: ERR_COLOR,
                });
                return;
            }
        }
    };

    let n_samples = match parse_positive_count(&sample_text, "number of samples") {
        Ok(value) => value,
        Err(error) => {
            commands.trigger(ErrorToast {
                text: error,
                color: ERR_COLOR,
            });
            return;
        }
    };
    let n_warmup = match parse_count(&warmup_text, "number of rounds") {
        Ok(value) => value,
        Err(error) => {
            commands.trigger(ErrorToast {
                text: error,
                color: ERR_COLOR,
            });
            return;
        }
    };

    println!("Running inference: seed={seed}, samples={n_samples}, warmup={n_warmup}");
    commands.remove_resource::<InferenceResultResource>();
    commands.remove_resource::<SampleSelections>();
    commands.insert_resource(InferenceStatusResource {
        state: InferenceResultState::Running,
        requested_samples: n_samples,
    });
    commands.trigger(SetPosteriorSampleEnabled(false));

    let graph = compiled.0.graph().clone();
    let control = Arc::new(InferenceControl::new());
    let worker_control = Arc::clone(&control);
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let compiled = graph.compile()?;
        let cancel_control = Arc::clone(&worker_control);
        let warmup_control = Arc::clone(&worker_control);
        let diagnostic_control = Arc::clone(&worker_control);
        let sample_control = Arc::clone(&worker_control);

        compiled.run_inference_controlled(
            seed,
            n_samples,
            n_warmup,
            move || cancel_control.cancel_requested.load(Ordering::Relaxed),
            move |completed| {
                warmup_control
                    .warmup_completed
                    .store(completed, Ordering::Relaxed);
            },
            move |variables| {
                diagnostic_control
                    .warmup_negative_infinity
                    .store(!variables.is_empty(), Ordering::Relaxed);
                *diagnostic_control
                    .warmup_negative_infinity_variables
                    .lock()
                    .expect("warmup diagnostic should not be poisoned") = variables;
                diagnostic_control
                    .warmup_diagnostic_ready
                    .store(true, Ordering::Release);
            },
            move |draw_index, values| {
                sample_control
                    .samples_completed
                    .store(draw_index + 1, Ordering::Relaxed);
                sample_control
                    .pending_draws
                    .lock()
                    .expect("inference draw queue should not be poisoned")
                    .push(values.clone());
            },
        )
    });

    commands.insert_resource(InferenceJob {
        task,
        control,
        seed,
        requested_samples: n_samples,
        requested_warmup: n_warmup,
    });
}

fn append_live_draws(result: &mut InferenceResult, draws: Vec<ModelValues>) {
    for values in draws {
        for (node_id, value) in values {
            result
                .samples_by_node
                .entry(node_id)
                .or_default()
                .push(value);
        }
        result.n_samples += 1;
    }
}

fn reopen_selected_histogram(
    commands: &mut Commands,
    selected: &Query<&GraphNode, With<Selected>>,
    view: Option<&HistogramView>,
) {
    if let Some(view) = view {
        commands.trigger(OpenHistogramPanel {
            subject: view.subject,
            clear_toasts: false,
        });
        return;
    }
    let Ok(node) = selected.single() else {
        return;
    };
    commands.trigger(OpenHistogramPanel {
        subject: HistogramSubject::Node(node.0),
        clear_toasts: false,
    });
}

fn reopen_joint_distribution(commands: &mut Commands, joint_view: Option<JointDistributionView>) {
    if let Some(joint) = joint_view {
        commands.trigger(OpenJointDistributionView {
            x_node_id: joint.x_node_id,
            y_node_id: joint.y_node_id,
        });
    }
}

/// Drains live posterior draws and completes the background task without ever
/// waiting on it from Bevy's main thread.
pub fn poll_inference_job(
    mut commands: Commands,
    mut job: Option<ResMut<InferenceJob>>,
    mut live_results: Option<ResMut<InferenceResultResource>>,
    selected: Query<&GraphNode, With<Selected>>,
    view: Option<Single<&HistogramView>>,
    joint_view: Option<Single<&JointDistributionView>>,
) {
    let Some(job) = job.as_mut() else {
        return;
    };
    let joint_view = joint_view.as_ref().map(|joint| JointDistributionView {
        x_node_id: joint.x_node_id,
        y_node_id: joint.y_node_id,
    });
    let discard = job.control.discard_result.load(Ordering::Relaxed);
    if !discard
        && job.control.warmup_diagnostic_ready.load(Ordering::Acquire)
        && job.control.warmup_negative_infinity.load(Ordering::Relaxed)
        && !job
            .control
            .warmup_warning_emitted
            .swap(true, Ordering::Relaxed)
    {
        let variables = job
            .control
            .warmup_negative_infinity_variables
            .lock()
            .expect("warmup diagnostic should not be poisoned")
            .join(", ");
        commands.trigger(ErrorToast {
            text: format!(
                "Warning: warmup ended with a -infinity log probability for {variables}. The posterior may be unreliable; check that variable's observed data and distribution parameters."
            ),
            color: Color::srgb(0.55, 0.30, 0.03),
        });
    }
    let pending = {
        let mut pending = job
            .control
            .pending_draws
            .lock()
            .expect("inference draw queue should not be poisoned");
        std::mem::take(&mut *pending)
    };
    let received_draws = pending.len();

    if !discard && received_draws > 0 {
        if let Some(results) = live_results.as_mut() {
            append_live_draws(&mut results.0, pending);
        } else {
            let mut results = InferenceResult {
                seed: job.seed,
                n_samples: 0,
                n_warmup: job.requested_warmup,
                samples_by_node: HashMap::new(),
                traces: Vec::new(),
            };
            append_live_draws(&mut results, pending);
            commands.insert_resource(InferenceResultResource(results));
        }
        reopen_selected_histogram(&mut commands, &selected, view.as_ref().map(|view| **view));
        reopen_joint_distribution(&mut commands, joint_view);
    }

    let Some(outcome) = check_ready(&mut job.task) else {
        return;
    };
    let discard = job.control.discard_result.load(Ordering::Relaxed);
    commands.remove_resource::<InferenceJob>();

    if discard {
        commands.remove_resource::<InferenceResultResource>();
        commands.remove_resource::<InferenceStatusResource>();
        commands.remove_resource::<SampleSelections>();
        commands.trigger(CloseHistogramPanel);
        commands.trigger(SetPosteriorSampleEnabled(false));
        return;
    }

    match outcome {
        Ok(outcome) if outcome.result.n_samples > 0 => {
            let retained = outcome.result.n_samples;
            let completed_warmup = outcome.result.n_warmup;
            let completed_seed = outcome.result.seed;
            let state = if outcome.cancelled {
                InferenceResultState::Cancelled
            } else {
                InferenceResultState::Complete
            };
            commands.insert_resource(InferenceResultResource(outcome.result));
            commands.insert_resource(InferenceStatusResource {
                state,
                requested_samples: job.requested_samples,
            });
            commands.trigger(SetPosteriorSampleEnabled(true));
            reopen_selected_histogram(&mut commands, &selected, view.as_ref().map(|view| **view));
            reopen_joint_distribution(&mut commands, joint_view);
            let message = if state == InferenceResultState::Cancelled {
                format!(
                    "Inference cancelled after {completed_warmup} warmup steps. Keeping {retained} of {} requested posterior draws.",
                    job.requested_samples,
                )
            } else {
                format!(
                    "Inference complete: {retained} samples after {completed_warmup} warmup steps (seed {completed_seed}). Click a node for its summary."
                )
            };
            commands.trigger(ErrorToast {
                text: message,
                color: SAMPLE_COLOR,
            });
        }
        Ok(outcome) => {
            commands.remove_resource::<InferenceResultResource>();
            commands.remove_resource::<InferenceStatusResource>();
            commands.remove_resource::<SampleSelections>();
            commands.trigger(CloseHistogramPanel);
            commands.trigger(SetPosteriorSampleEnabled(false));
            let message = if outcome.cancelled {
                "Inference cancelled before any posterior draws were retained."
            } else {
                "Inference finished without retaining posterior draws."
            };
            commands.trigger(ErrorToast {
                text: message.to_string(),
                color: SAMPLE_COLOR,
            });
        }
        Err(error) => {
            let partial_count = live_results
                .as_ref()
                .map_or(received_draws, |results| results.0.n_samples);
            commands.insert_resource(InferenceStatusResource {
                state: InferenceResultState::Failed,
                requested_samples: job.requested_samples,
            });
            commands.trigger(SetPosteriorSampleEnabled(false));
            if partial_count == 0 {
                commands.remove_resource::<InferenceResultResource>();
                commands.trigger(CloseHistogramPanel);
            } else {
                reopen_selected_histogram(
                    &mut commands,
                    &selected,
                    view.as_ref().map(|view| **view),
                );
                reopen_joint_distribution(&mut commands, joint_view);
            }
            commands.trigger(ErrorToast {
                text: format!("Inference error after {partial_count} retained draws: {error}"),
                color: ERR_COLOR,
            });
        }
    }
}

pub fn update_inference_progress(
    job: Option<Res<InferenceJob>>,
    mut containers: Query<&mut Node, With<InferenceProgressContainer>>,
    mut fills: Query<
        &mut Node,
        (
            With<InferenceProgressFill>,
            Without<InferenceProgressContainer>,
        ),
    >,
    mut progress_labels: Query<&mut Text, With<InferenceProgressLabel>>,
    mut button_labels: Query<
        &mut Text,
        (
            With<InferenceRunButtonLabel>,
            Without<InferenceProgressLabel>,
        ),
    >,
) {
    let Ok(mut container) = containers.single_mut() else {
        return;
    };
    let Ok(mut fill) = fills.single_mut() else {
        return;
    };
    let Ok(mut progress_label) = progress_labels.single_mut() else {
        return;
    };
    let Ok(mut button_label) = button_labels.single_mut() else {
        return;
    };

    let Some(job) = job else {
        container.display = Display::None;
        fill.width = percent(0.0);
        progress_label.0.clear();
        button_label.0 = "Run inference".to_string();
        return;
    };

    container.display = Display::Flex;
    let stopping = job.control.cancel_requested.load(Ordering::Relaxed);
    button_label.0 = if stopping { "Stopping..." } else { "Stop" }.to_string();
    let warmup = job.control.warmup_completed.load(Ordering::Relaxed);
    let samples = job.control.samples_completed.load(Ordering::Relaxed);
    let (label, completed, total) = if warmup < job.requested_warmup {
        ("Warmup", warmup, job.requested_warmup)
    } else {
        ("Sampling", samples, job.requested_samples)
    };
    let fraction = if total == 0 {
        1.0
    } else {
        completed as f32 / total as f32
    };
    fill.width = percent(fraction.clamp(0.0, 1.0) * 100.0);
    progress_label.0 = if stopping {
        format!("Stopping... {completed}/{total}")
    } else {
        format!("{label}: {completed}/{total}")
    };
}

fn parse_positive_count(text: &EditableText, label: &str) -> Result<usize, String> {
    let value = parse_count(text, label)?;
    if value == 0 {
        Err(format!("{label} must be greater than zero"))
    } else {
        Ok(value)
    }
}

fn parse_count(text: &EditableText, label: &str) -> Result<usize, String> {
    text.value()
        .to_string()
        .trim()
        .parse::<usize>()
        .map_err(|_| format!("{label} must be a non-negative whole number"))
}

pub fn sample_popup(
    event: On<SampleDisplay>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
){
    commands
        .spawn((
        Mesh2d(meshes.add(Rectangle::new(100., 30.))),
        MeshMaterial2d(materials.add(ColorMaterial::from_color(SAMPLE_COLOR))),
        SamplePopup {
            timer: Timer::from_seconds(15.0, TimerMode::Once),
            console_output: event.console_output.clone(),
        },
        Pickable {
            should_block_lower: true,
            is_hoverable: true,
        },
        Transform::from_xyz(event.pos.x, event.pos.y + 50., 99.),
        children![(
            Pickable::IGNORE,
            Text2d::new(event.val.clone()),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(14.),
                ..text_font()
            },
        )],
    ))
    .observe(print_plate_sample);
}

fn first_scalar(value: &ModelResult) -> Option<f64> {
    match value {
        ModelResult::Scalar(value) => Some(*value),
        ModelResult::Plate(values) => values.iter().find_map(first_scalar),
    }
}

fn print_plate_sample(mut event: On<Pointer<Click>>, popups: Query<&SamplePopup>) {
    event.propagate(false);
    let Ok(popup) = popups.get(event.event_target()) else {
        return;
    };
    if let Some(output) = &popup.console_output {
        println!("Plate sample:\n{output}");
    }
}

pub fn tick_sample_popups(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut SamplePopup)>,
) {
    for (entity, mut toast) in &mut q {
        toast.timer.tick(time.delta());

        if toast.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

pub fn compile_ir(
    rand_nodes: &Query<(Entity, &RandomNode), (Without<ComputeNode>, Without<ScalarNode>)>,
    compute_nodes: &Query<(Entity, &ComputeNode), (Without<RandomNode>, Without<ScalarNode>)>,
    scalar_nodes: &Query<(Entity, &ScalarNode), (Without<RandomNode>, Without<ComputeNode>)>,
    node_ids: &Query<(Entity, &GraphNode)>,
    node_positions: &Query<(&GraphNode, &Transform), Without<Plate>>,
    plates: &Query<(&GraphNode, &Plate)>,
) -> Result<GraphIR, String> {
    let mut graph = GraphIR::new();

    let param_to_ir = |param: &ParamValue| -> Result<ParamIR, String> {
        let entity = param
            .1
            .ok_or_else(|| "A node has unspecified parameters!".to_string())?;

        let node_id = node_ids
            .get(entity)
            .map_err(|_| "Parameter references a missing node!".to_string())?
            .1
            .0;
    
        Ok(ParamIR { from_node: node_id })
    };

    for (entity, rand) in rand_nodes.into_iter(){
        let node = node_ids
            .get(entity)
        .map_err(|_| "Node is missing its GraphNode ID")?
        .1;
        let params = rand
            .params
            .iter()
            .map(|param| {
                param_to_ir(param).map_err(|error| format!("node {}: {error}", node.0))
            })
            .collect::<Result<Vec<_>, _>>()?;
        graph.nodes.insert(
            node.0,
            NodeIR::Random {
            id: node.0,
            label: rand.name.clone(),
            dist_type: rand.dist_type.clone(),
            params: params,
            },
        );
    }

    for (entity, compute) in compute_nodes.into_iter(){
        let node = node_ids
            .get(entity)
        .map_err(|_| "Node is missing its GraphNode ID")?
        .1;
        let params = compute
            .params
            .iter()
            .map(|param| {
                param_to_ir(param).map_err(|error| format!("node {}: {error}", node.0))
            })
            .collect::<Result<Vec<_>, _>>()?;
        graph.nodes.insert(
            node.0,
            NodeIR::Compute {
            id: node.0,
            operation: compute.operation,
            params: params,
            },
        );
    }

    for (entity, scalar) in scalar_nodes.into_iter(){
        let node = node_ids
            .get(entity)
        .map_err(|_| "Node is missing its GraphNode ID")?
        .1;
        graph.nodes.insert(
            node.0,
            NodeIR::Scalar {
            id: node.0,
            value: scalar.val,
            },
        );
    }

    let plate_bounds = plates
        .iter()
        .filter(|(_, plate)| plate.bounds.is_substantial())
        .map(|(node, plate)| (node.0, plate.bounds, plate.data.n))
        .collect::<Vec<_>>();
    let positions = node_positions
        .iter()
        .map(|(node, transform)| (node.0, transform.translation.truncate()))
        .collect::<Vec<_>>();
    graph.plates = compile_plate_irs(&plate_bounds, &positions)?;

    for (node, plate) in plates
        .iter()
        .filter(|(_, plate)| plate.bounds.is_substantial())
    {
        let plate_ir = graph
            .plates
            .get_mut(&node.0)
            .expect("substantial plates should have compiled IR");
        plate_ir.data = plate.data.data.clone();

        for (&entity, column) in &plate.mapping {
            if column == "unobserved" {
                continue;
            }

            let node_id = node_ids
                .get(entity)
                .map_err(|_| format!("plate {} maps a missing node", node.0))?
                .1
                .0;
            plate_ir.mapping.insert(node_id, column.clone());
        }
    }

    Ok(graph)
}

fn compile_plate_irs(
    plates: &[(u32, PlateBounds, usize)],
    nodes: &[(u32, Vec2)],
) -> Result<HashMap<u32, PlateIR>, String> {
    for (index, &(left_id, left_bounds, _)) in plates.iter().enumerate() {
        for &(right_id, right_bounds, _) in &plates[index + 1..] {
            let left_contains_right = left_bounds.contains_bounds(right_bounds);
            let right_contains_left = right_bounds.contains_bounds(left_bounds);

            if left_contains_right && right_contains_left {
                return Err(format!(
                    "plates {left_id} and {right_id} have identical bounds"
                ));
            }
        }
    }

    let mut result = HashMap::new();

    for &(plate_id, bounds, n) in plates {
        let contained_plates = plates
            .iter()
            .filter(|(candidate_id, candidate_bounds, _)| {
                *candidate_id != plate_id && bounds.contains_bounds(*candidate_bounds)
            })
            .copied()
            .collect::<Vec<_>>();

        let mut direct_plates = contained_plates
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>();
        direct_plates.sort_unstable();

        let mut member_nodes = nodes
            .iter()
            .filter(|(_, position)| bounds.contains_point(*position))
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        member_nodes.sort_unstable();

        result.insert(
            plate_id,
            PlateIR {
                id: plate_id,
                n,
                nodes: member_nodes,
                plates: direct_plates,
                data: HashMap::new(),
                mapping: HashMap::new(),
            },
        );
    }

    Ok(result)
}

//you can tell ai wrote it when there starts to actually be tests...
#[cfg(test)]
mod plate_tests {
    use super::*;

    #[test]
    fn compilation_errors_extract_stable_node_ids() {
        assert_eq!(
            node_ids_in_compilation_error(
                "invalid dependency from node 12 to node 4 at node#12[plate#3=0]"
            ),
            vec![4, 12]
        );
        assert!(node_ids_in_compilation_error("plate 3 has no dataset").is_empty());
    }

    #[test]
    fn plate_ir_records_direct_nested_contents() {
        let plates = vec![
            (
                1,
                PlateBounds::from_points(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0)),
                10,
            ),
            (
                2,
                PlateBounds::from_points(Vec2::new(20.0, 20.0), Vec2::new(80.0, 80.0)),
                15,
            ),
        ];
        let nodes = vec![
            (1, Vec2::new(10.0, 10.0)),
            (2, Vec2::new(50.0, 50.0)),
            (3, Vec2::new(120.0, 120.0)),
        ];

        let result = compile_plate_irs(&plates, &nodes).unwrap();

        assert_eq!(result[&1].nodes, vec![1, 2]);
        assert_eq!(result[&1].plates, vec![2]);
        assert_eq!(result[&2].nodes, vec![2]);
        assert!(result[&2].plates.is_empty());
    }

    #[test]
    fn plate_ir_accepts_partial_overlap_with_complete_membership() {
        let plates = vec![
            (
                10,
                PlateBounds::from_points(Vec2::ZERO, Vec2::new(100.0, 100.0)),
                3,
            ),
            (
                11,
                PlateBounds::from_points(Vec2::new(50.0, 50.0), Vec2::new(150.0, 150.0)),
                4,
            ),
        ];

        let nodes = vec![(1, Vec2::new(75.0, 75.0))];
        let result = compile_plate_irs(&plates, &nodes).unwrap();
        assert_eq!(result[&10].nodes, vec![1]);
        assert_eq!(result[&11].nodes, vec![1]);
    }

    #[test]
    fn plate_ir_allows_touching_sibling_borders() {
        let plates = vec![
            (
                10,
                PlateBounds::from_points(Vec2::ZERO, Vec2::new(50.0, 50.0)),
                3,
            ),
            (
                11,
                PlateBounds::from_points(Vec2::new(50.0, 0.0), Vec2::new(100.0, 50.0)),
                4,
            ),
        ];

        assert!(compile_plate_irs(&plates, &[]).is_ok());
    }

    #[test]
    fn plate_ir_includes_node_on_every_touching_border() {
        let plates = vec![
            (
                10,
                PlateBounds::from_points(Vec2::ZERO, Vec2::new(50.0, 50.0)),
                3,
            ),
            (
                11,
                PlateBounds::from_points(Vec2::new(50.0, 0.0), Vec2::new(100.0, 50.0)),
                4,
            ),
        ];
        let nodes = vec![(1, Vec2::new(50.0, 25.0))];

        let result = compile_plate_irs(&plates, &nodes).unwrap();
        assert_eq!(result[&10].nodes, vec![1]);
        assert_eq!(result[&11].nodes, vec![1]);
    }

    #[test]
    fn node_motion_invalidates_only_when_complete_membership_changes() {
        let plates = vec![
            (
                10,
                PlateBounds::from_points(Vec2::ZERO, Vec2::new(100.0, 100.0)),
                2,
            ),
            (
                20,
                PlateBounds::from_points(Vec2::new(50.0, 0.0), Vec2::new(150.0, 100.0)),
                3,
            ),
        ];
        let before = compile_plate_irs(&plates, &[(1, Vec2::new(60.0, 20.0))]).unwrap();
        let within = compile_plate_irs(&plates, &[(1, Vec2::new(80.0, 80.0))]).unwrap();
        let crossed = compile_plate_irs(&plates, &[(1, Vec2::new(120.0, 80.0))]).unwrap();
        assert_eq!(before[&10].nodes, within[&10].nodes);
        assert_eq!(before[&20].nodes, within[&20].nodes);
        assert_ne!(before[&10].nodes, crossed[&10].nodes);
        assert_eq!(before[&20].nodes, crossed[&20].nodes);
    }
}
