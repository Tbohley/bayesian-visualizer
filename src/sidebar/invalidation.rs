//! Invalidate compiled models only when execution-relevant graph data changes.
use super::*;
use crate::bayesian_core::GraphIR;
use crate::bevy_to_fugue::*;
use crate::data_vis::{CloseHistogramPanel, PlateIndexSelections, SampleSelections};
use std::{collections::HashMap, sync::atomic::Ordering};

//greys out controls on graph changes in order to require recompilation
pub fn invalidate_compilation_on_graph_change(
    mut commands: Commands,
    graph_resource: Option<Res<GraphIRResource>>,
    inference_job: Option<Res<InferenceJob>>,
    changed_graph: Query<
        (),
        (
            Or<(
                Changed<GraphNode>,
                Changed<RandomNode>,
                Changed<ComputeNode>,
                Changed<ScalarNode>,
                Changed<VectorNode>,
                Changed<GraphLink>,
            )>,
            Without<PlateDraft>,
        ),
    >,
    changed_plates: Query<(), (Changed<Plate>, Without<PlateDraft>)>,
    changed_positions: Query<
        (),
        (
            Changed<Transform>,
            With<GraphNode>,
            Without<Plate>,
            Without<PlateDraft>,
        ),
    >,
    node_positions: Query<(&GraphNode, &Transform), (Without<Plate>, Without<PlateDraft>)>,
    plates: Query<(&GraphNode, &Plate), Without<PlateDraft>>,
    graph_nodes: Query<&GraphNode>,
    mut removed_graph_nodes: RemovedComponents<GraphNode>,
) {
    let Some(graph_resource) = graph_resource else {
        return;
    };
    let graph_node_removed = removed_graph_nodes.read().next().is_some();
    let membership_changed = !changed_positions.is_empty()
        && node_plate_membership_changed(graph_resource.0.graph(), &node_positions, &plates);
    let plate_changed = !changed_plates.is_empty()
        && plate_execution_metadata_changed(
            graph_resource.0.graph(),
            &node_positions,
            &plates,
            &graph_nodes,
        );
    let graph_changed =
        !changed_graph.is_empty() || membership_changed || plate_changed || graph_node_removed;

    if graph_changed {
        if let Some(job) = inference_job {
            job.control.discard_result.store(true, Ordering::Relaxed);
            job.control.cancel_requested.store(true, Ordering::Relaxed);
        }
        commands.remove_resource::<GraphIRResource>();
        commands.remove_resource::<InferenceResultResource>();
        commands.remove_resource::<InferenceStatusResource>();
        commands.remove_resource::<SampleSelections>();
        commands.remove_resource::<PlateIndexSelections>();
        commands.trigger(SetInferenceControlsEnabled(false));
        commands.trigger(CloseHistogramPanel);
    }
}

/// Node coordinates only affect the compiled model when they change complete
/// plate membership. Ordinary dragging within the same scope is presentation-only.
fn node_plate_membership_changed(
    compiled: &GraphIR,
    node_positions: &Query<(&GraphNode, &Transform), (Without<Plate>, Without<PlateDraft>)>,
    plates: &Query<(&GraphNode, &Plate), Without<PlateDraft>>,
) -> bool {
    let current_plates = plates
        .iter()
        .filter(|(_, plate)| plate.bounds.is_substantial())
        .map(|(node, plate)| (node.0, plate.bounds))
        .collect::<Vec<_>>();

    if current_plates.len() != compiled.plates.len()
        || current_plates
            .iter()
            .any(|(id, _)| !compiled.plates.contains_key(id))
    {
        return true;
    }

    for &(plate_id, bounds) in &current_plates {
        let mut current_nodes = node_positions
            .iter()
            .filter(|(_, transform)| bounds.contains_point(transform.translation.truncate()))
            .map(|(node, _)| node.0)
            .collect::<Vec<_>>();
        current_nodes.sort_unstable();

        let Some(compiled_plate) = compiled.plates.get(&plate_id) else {
            return true;
        };
        if current_nodes != compiled_plate.nodes {
            return true;
        }
    }

    false
}

/// Plate movement is presentation-only unless it changes execution metadata.
fn plate_execution_metadata_changed(
    compiled: &GraphIR,
    node_positions: &Query<(&GraphNode, &Transform), (Without<Plate>, Without<PlateDraft>)>,
    plates: &Query<(&GraphNode, &Plate), Without<PlateDraft>>,
    graph_nodes: &Query<&GraphNode>,
) -> bool {
    if node_plate_membership_changed(compiled, node_positions, plates) {
        return true;
    }

    let current = plates.iter().collect::<Vec<_>>();
    for (plate_node, plate) in &current {
        let Some(compiled_plate) = compiled.plates.get(&plate_node.0) else {
            return true;
        };
        if plate.data.n != compiled_plate.n || plate.data.data != compiled_plate.data {
            return true;
        }

        let mapping = plate
            .mapping
            .iter()
            .filter(|(_, column)| column.as_str() != "unobserved")
            .map(|(entity, column)| {
                graph_nodes
                    .get(*entity)
                    .map(|node| (node.0, column.clone()))
            })
            .collect::<Result<HashMap<_, _>, _>>();
        match mapping {
            Ok(mapping) if mapping == compiled_plate.mapping => {}
            _ => return true,
        }

        let mut contained = current
            .iter()
            .filter(|(candidate, candidate_plate)| {
                candidate.0 != plate_node.0 && plate.bounds.contains_bounds(candidate_plate.bounds)
            })
            .map(|(candidate, _)| candidate.0)
            .collect::<Vec<_>>();
        contained.sort_unstable();
        if contained != compiled_plate.plates {
            return true;
        }
    }
    false
}
