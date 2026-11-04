//! Compile the current editor graph and present node-specific diagnostics.
use super::{
    GraphIRResource, InferenceJob, InferenceResultResource, InferenceStatusResource,
    TriggerCompilation, ir::compile_ir,
};
#[cfg(test)]
use super::{inference::reopen_selected_histogram, ir::compile_plate_irs};
use crate::{
    constants::{ERR_COLOR, SAMPLE_COLOR},
    data_vis::{CloseHistogramPanel, PlateIndexSelections, SampleSelections},
    graph::{Plate, print_graph_preset},
    nodes::{ComputeNode, GraphNode, IntegerScalar, RandomNode, ScalarNode, VectorNode},
    sidebar::SetInferenceControlsEnabled,
    ui::{ErrorToast, ShowCompilationErrorMarkers},
};
use bevy::prelude::*;
use std::{collections::HashSet, sync::atomic::Ordering};

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
    scalar_nodes: Query<
        (Entity, &ScalarNode, Has<IntegerScalar>),
        (Without<RandomNode>, Without<ComputeNode>),
    >,
    vector_nodes: Query<(Entity, &VectorNode)>,
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
        &vector_nodes,
        &node_ids,
        &node_positions,
        &plates,
    );

    // Any compile attempt supersedes posterior results from the previous graph.
    commands.remove_resource::<InferenceResultResource>();
    commands.remove_resource::<InferenceStatusResource>();
    commands.remove_resource::<SampleSelections>();
    commands.remove_resource::<PlateIndexSelections>();
    commands.trigger(CloseHistogramPanel);
    let graph = compile_ir(
        &rand_nodes,
        &compute_nodes,
        &scalar_nodes,
        &vector_nodes,
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
                    //save graph for other functions
                    match g.compile_checked() {
                        Ok(compiled) => {
                            commands.trigger(ErrorToast {
                                color: SAMPLE_COLOR,
                                text: String::from(
                                    "Graph successfully compiled. No errors detected... yet.",
                                ),
                            });
                            match compiled.bind_debug_string() {
                                Ok(code) => println!("Compiled Fugue execution plan:\n{code}"),
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
                                error.node_ids.clone(),
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

#[cfg(test)]
#[path = "tests/compilation.rs"]
mod plate_tests;
