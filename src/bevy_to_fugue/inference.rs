//! Inference launch, live draw streaming, cancellation, and progress.
use super::*;
#[cfg(target_arch = "wasm32")]
use crate::bayesian_core::InferenceStep;
use crate::bayesian_core::*;
use crate::constants::*;
use crate::data_vis::{
    CloseHistogramPanel, HistogramSubject, HistogramView, JointDistributionView,
    OpenHistogramPanel, OpenJointDistributionView, PlateIndexSelections, SampleSelections,
};
use crate::graph::*;
use crate::nodes::*;
use crate::sidebar::{
    InferenceProgressContainer, InferenceProgressFill, InferenceProgressLabel,
    InferenceRunButtonLabel, NumberOfSamplesTextbox, NumberOfWarmupTextbox, RandomSeedTextbox,
};
use crate::ui::ErrorToast;
#[cfg(target_arch = "wasm32")]
use bevy::platform::time::Instant;
use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy::tasks::{AsyncComputeTaskPool, futures::check_ready};
use bevy::text::EditableText;
use std::collections::HashMap;
use std::sync::{Arc, atomic::Ordering};
#[cfg(target_arch = "wasm32")]
use std::time::Duration;

pub fn run_inference(
    _event: On<Pointer<Click>>,
    mut commands: Commands,
    inference_job: Option<Res<InferenceJob>>,
    graph_resource: Option<Res<GraphIRResource>>,
    seed_text: Single<&EditableText, With<RandomSeedTextbox>>,
    sample_text: Single<&EditableText, With<NumberOfSamplesTextbox>>,
    warmup_text: Single<&EditableText, With<NumberOfWarmupTextbox>>,
    random_nodes: Query<(&GraphNode, Has<ObservedNode>), With<RandomNode>>,
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
    let auto_histogram_node = random_nodes
        .iter()
        .filter_map(|(node, observed)| (!observed).then_some(node.0))
        .min();
    commands.remove_resource::<InferenceResultResource>();
    commands.remove_resource::<SampleSelections>();
    commands.remove_resource::<PlateIndexSelections>();
    commands.insert_resource(InferenceStatusResource {
        state: InferenceResultState::Running,
        requested_samples: n_samples,
    });

    let compiled_graph = compiled.0.clone();
    let node_extents = compiled_graph.posterior_node_extents();
    let node_shapes = compiled_graph.node_shapes().clone();
    let control = Arc::new(InferenceControl::new());

    #[cfg(not(target_arch = "wasm32"))]
    {
        let worker_control = Arc::clone(&control);
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let cancel_control = Arc::clone(&worker_control);
            let warmup_control = Arc::clone(&worker_control);
            let sample_control = Arc::clone(&worker_control);

            compiled_graph.run_inference_controlled(
                seed,
                n_samples,
                n_warmup,
                move || cancel_control.cancel_requested.load(Ordering::Relaxed),
                move |completed| {
                    warmup_control
                        .warmup_completed
                        .store(completed, Ordering::Relaxed);
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
            auto_histogram_node,
            node_extents,
            node_shapes,
            plot_refresh_elapsed: 0.1,
        });
        commands.trigger(crate::ui::editor::InspectModel);
    }

    #[cfg(target_arch = "wasm32")]
    {
        let runner = match compiled_graph.inference_runner(seed, n_samples, n_warmup) {
            Ok(runner) => runner,
            Err(error) => {
                commands.insert_resource(InferenceStatusResource {
                    state: InferenceResultState::Failed,
                    requested_samples: n_samples,
                });
                commands.trigger(ErrorToast {
                    text: format!("Could not start inference: {error}"),
                    color: ERR_COLOR,
                });
                return;
            }
        };
        commands.insert_resource(InferenceJob {
            runner,
            control,
            seed,
            requested_samples: n_samples,
            requested_warmup: n_warmup,
            auto_histogram_node,
            node_extents,
            node_shapes,
            plot_refresh_elapsed: 0.1,
        });
        commands.trigger(crate::ui::editor::InspectModel);
    }
}

fn append_live_draws(
    result: &mut InferenceResult,
    draws: Vec<crate::bayesian_core::graph_checks::PosteriorValues>,
) {
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

pub(super) fn reopen_selected_histogram(
    commands: &mut Commands,
    selected: &Query<&GraphNode, With<Selected>>,
    view: Option<&HistogramView>,
    auto_histogram_node: Option<u32>,
    inspecting: bool,
) {
    // Returning to editing must not be undone by the next live sampling update.
    if !inspecting {
        return;
    }
    if let Some(view) = view {
        commands.trigger(OpenHistogramPanel {
            subject: view.subject,
            clear_toasts: false,
        });
        return;
    }
    if let Some(node_id) = auto_histogram_node {
        commands.trigger(OpenNodeInspection { node_id });
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
    time: Res<Time>,
    selected: Query<&GraphNode, With<Selected>>,
    view: Option<Single<&HistogramView>>,
    joint_view: Option<Single<&JointDistributionView>>,
    editor_view: Res<crate::ui::editor::EditorView>,
) {
    let Some(job) = job.as_mut() else {
        return;
    };
    job.plot_refresh_elapsed += time.delta_secs();
    let joint_view = joint_view.as_ref().map(|joint| JointDistributionView {
        x_node_id: joint.x_node_id,
        y_node_id: joint.y_node_id,
    });

    #[cfg(not(target_arch = "wasm32"))]
    let outcome = check_ready(&mut job.task);

    // WASM task pools are single-threaded. Drive a bounded number of complete
    // MCMC transitions here, then return to Bevy so rendering, progress, and
    // the stop button remain responsive. A single transition is the smallest
    // safe interruption point in Fugue's current MCMC API.
    #[cfg(target_arch = "wasm32")]
    let outcome = {
        const FRAME_BUDGET: Duration = Duration::from_millis(8);
        let deadline = Instant::now() + FRAME_BUDGET;
        let mut outcome = None;
        loop {
            let cancelled = job.control.cancel_requested.load(Ordering::Relaxed);
            match job.runner.step(cancelled) {
                Ok(InferenceStep::Warmup { completed }) => {
                    job.control
                        .warmup_completed
                        .store(completed, Ordering::Relaxed);
                }
                Ok(InferenceStep::WarmupComplete) => {}
                Ok(InferenceStep::Sample { draw_index, values }) => {
                    job.control
                        .samples_completed
                        .store(draw_index + 1, Ordering::Relaxed);
                    job.control
                        .pending_draws
                        .lock()
                        .expect("inference draw queue should not be poisoned")
                        .push(values);
                }
                Ok(InferenceStep::Complete(result)) => {
                    outcome = Some(Ok(result));
                    break;
                }
                Err(error) => {
                    outcome = Some(Err(error));
                    break;
                }
            }
            if Instant::now() >= deadline {
                break;
            }
        }
        outcome
    };

    let discard = job.control.discard_result.load(Ordering::Relaxed);
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
                node_extents: job.node_extents.clone(),
                node_shapes: job.node_shapes.clone(),
                #[cfg(test)]
                traces: Vec::new(),
            };
            append_live_draws(&mut results, pending);
            commands.insert_resource(InferenceResultResource(results));
        }
        if job.plot_refresh_elapsed >= 0.1 && outcome.is_none() {
            job.plot_refresh_elapsed = 0.0;
            reopen_selected_histogram(
                &mut commands,
                &selected,
                view.as_ref().map(|view| **view),
                job.auto_histogram_node,
                editor_view.inspecting,
            );
            reopen_joint_distribution(&mut commands, joint_view);
        }
    }

    let Some(outcome) = outcome else {
        return;
    };
    let discard = job.control.discard_result.load(Ordering::Relaxed);
    commands.remove_resource::<InferenceJob>();

    if discard {
        commands.remove_resource::<InferenceResultResource>();
        commands.remove_resource::<InferenceStatusResource>();
        commands.remove_resource::<SampleSelections>();
        commands.remove_resource::<PlateIndexSelections>();
        commands.trigger(CloseHistogramPanel);
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
            reopen_selected_histogram(
                &mut commands,
                &selected,
                view.as_ref().map(|view| **view),
                job.auto_histogram_node,
                editor_view.inspecting,
            );
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
            commands.remove_resource::<PlateIndexSelections>();
            commands.trigger(CloseHistogramPanel);
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
            if partial_count == 0 {
                commands.remove_resource::<InferenceResultResource>();
                commands.trigger(CloseHistogramPanel);
            } else {
                reopen_selected_histogram(
                    &mut commands,
                    &selected,
                    view.as_ref().map(|view| **view),
                    job.auto_histogram_node,
                    editor_view.inspecting,
                );
                reopen_joint_distribution(&mut commands, joint_view);
            }
            commands.trigger(ErrorToast {
                text: if error == WARMUP_FAILURE_WARNING {
                    error.clone()
                } else {
                    format!("Inference error after {partial_count} retained draws: {error}")
                },
                color: if error == WARMUP_FAILURE_WARNING {
                    Color::srgb(0.55, 0.30, 0.03)
                } else {
                    ERR_COLOR
                },
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
