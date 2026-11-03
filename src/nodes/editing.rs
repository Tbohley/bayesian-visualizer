//! Parameter autofill, names, and observation-dependent labels.
use super::*;
use crate::bevy_to_fugue::InferenceResultResource;
use crate::constants::*;
use crate::graph::*;
use crate::sidebar::global::graph_change_requires_confirmation;
use crate::sidebar::*;
use bevy::prelude::*;

pub fn autofill_next_param(
    event: On<AutofillNextParam>,
    mut random_nodes: Query<&mut RandomNode>,
    mut compute_nodes: Query<&mut ComputeNode>,
) {
    let params = if let Ok(node) = random_nodes.get_mut(event.node) {
        node.into_inner().params.as_mut_slice()
    } else if let Ok(node) = compute_nodes.get_mut(event.node) {
        node.into_inner().params.as_mut_slice()
    } else {
        return;
    };

    if let Some(param) = params.iter_mut().find(|param| param.1.is_none()) {
        param.1 = Some(event.linked_node);
        println!("Autofilled parameter '{}' from completed link", param.0);
    }
}

pub fn set_node_name(
    event: On<SetNodeName>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut random_nodes: Query<
        (&GraphNode, &mut RandomNode, &mut Mesh2d),
        (
            Without<ScalarNode>,
            Without<NodeInterior>,
            Without<SelectedIndicator>,
        ),
    >,
    mut scalar_nodes: Query<&mut ScalarNode, Without<RandomNode>>,
    mut vector_nodes: Query<&mut VectorNode>,
    mut interiors: Query<
        (&ChildOf, &mut Mesh2d),
        (
            With<NodeInterior>,
            Without<RandomNode>,
            Without<SelectedIndicator>,
        ),
    >,
    mut indicators: Query<
        (&ChildOf, &mut Mesh2d),
        (
            With<SelectedIndicator>,
            Without<RandomNode>,
            Without<NodeInterior>,
        ),
    >,
    mut labels: Query<(&ChildOf, &mut Text2d), With<NodeLabel>>,
    plates: Query<&Plate, Without<PlateDraft>>,
    inference_results: Option<Res<InferenceResultResource>>,
) {
    if graph_change_requires_confirmation(&mut commands, inference_results.as_deref(), None) {
        return;
    }
    let name = event
        .name
        .chars()
        .take(MAX_NODE_NAME_CHARS)
        .collect::<String>();
    let name = (!name.is_empty()).then_some(name);

    if let Ok((node_id, mut random, mut exterior_mesh)) = random_nodes.get_mut(event.entity) {
        random.name = name;
        let label = random_node_label(&random, node_id.0);
        exterior_mesh.0 = meshes.add(random_node_mesh(RANDOM_NODE_RAD, &label));
        for (child_of, mut mesh) in &mut interiors {
            if child_of.parent() == event.entity {
                mesh.0 = meshes.add(random_node_mesh(
                    RANDOM_NODE_RAD - NODE_BORDER_WEIGHT,
                    &label,
                ));
            }
        }
        for (child_of, mut text) in &mut labels {
            if child_of.parent() == event.entity {
                text.0 = label.clone();
            }
        }
        for (child_of, mut mesh) in &mut indicators {
            if child_of.parent() == event.entity {
                mesh.0 = meshes.add(random_selection_mesh(&label));
            }
        }
        commands.trigger(ReloadSidebar);
        return;
    }

    if let Ok(mut vector) = vector_nodes.get_mut(event.entity) {
        vector.name = name;
        for (child_of, mut text) in &mut labels {
            if child_of.parent() == event.entity {
                text.0 = vector.label();
            }
        }
        commands.trigger(ReloadSidebar);
        return;
    }

    if let Ok(mut scalar) = scalar_nodes.get_mut(event.entity) {
        scalar.name = name;
        let label = scalar_display_label(event.entity, &scalar, &plates);
        for (child_of, mut text) in &mut labels {
            if child_of.parent() == event.entity {
                text.0 = label.clone();
            }
        }
        commands.trigger(ReloadSidebar);
    }
}

fn scalar_display_label(
    entity: Entity,
    scalar: &ScalarNode,
    plates: &Query<&Plate, Without<PlateDraft>>,
) -> String {
    scalar
        .name
        .clone()
        .or_else(|| {
            plates.iter().find_map(|plate| {
                plate
                    .mapping
                    .get(&entity)
                    .filter(|column| column.as_str() != "unobserved")
                    .cloned()
            })
        })
        .unwrap_or_else(|| format!("{:.1}", scalar.val))
}

pub fn update_node_observation_colors(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    plates: Query<&Plate, Without<PlateDraft>>,
    mut labels: Query<(&ChildOf, &mut Text2d), With<NodeLabel>>,
    nodes: Query<
        (
            Entity,
            Option<&RandomNode>,
            Option<&ScalarNode>,
            Has<ObservedNode>,
        ),
        Or<(With<RandomNode>, With<ScalarNode>)>,
    >,
    mut interiors: Query<(&ChildOf, &mut MeshMaterial2d<ColorMaterial>), With<NodeInterior>>,
) {
    for (entity, random, scalar, was_observed) in &nodes {
        let is_observed = plates.iter().any(|plate| {
            plate
                .mapping
                .get(&entity)
                .is_some_and(|column| column != "unobserved")
        });

        if random.is_some() && is_observed != was_observed {
            let color = if is_observed {
                RANDOM_NODE_COLOR
            } else {
                CANVAS_COLOR
            };
            for (child_of, mut material) in &mut interiors {
                if child_of.parent() == entity {
                    material.0 = materials.add(color);
                }
            }
        }

        if is_observed != was_observed {
            if is_observed {
                commands.entity(entity).insert(ObservedNode);
            } else {
                commands.entity(entity).remove::<ObservedNode>();
            }
        }

        if let Some(scalar) = scalar {
            let label = scalar
                .name
                .clone()
                .or_else(|| {
                    plates.iter().find_map(|plate| {
                        plate
                            .mapping
                            .get(&entity)
                            .filter(|column| column.as_str() != "unobserved")
                            .cloned()
                    })
                })
                .unwrap_or_else(|| format!("{:.1}", scalar.val));
            for (child_of, mut text) in &mut labels {
                if child_of.parent() == entity && text.0 != label {
                    text.0 = label.clone();
                }
            }
        }
    }
}
