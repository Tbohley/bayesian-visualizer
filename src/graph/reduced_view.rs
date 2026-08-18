use std::collections::HashSet;

use bevy::prelude::*;

use super::{GraphLink, Selected, UnfinishedLink, spawn_link_visual};
use crate::constants::RANDOM_NODE_RAD;
use crate::data_vis::CloseHistogramPanel;
use crate::nodes::{ComputeNode, GraphNode, RandomNode, ScalarNode, SelectedIndicator};
use crate::sidebar::ReloadSidebar;

#[derive(Resource, Default)]
pub struct ReducedView {
    pub active: bool,
}

#[derive(Event)]
pub struct ToggleReducedView;

#[derive(Component)]
pub struct ReducedViewButtonLabel;

#[derive(Component)]
pub struct ReducedViewLink;

#[allow(clippy::too_many_arguments)]
pub fn toggle_reduced_view(
    _event: On<ToggleReducedView>,
    mut commands: Commands,
    mut reduced_view: ResMut<ReducedView>,
    mut button_label: Single<&mut Text, With<ReducedViewButtonLabel>>,
    nodes: Query<
        (
            Entity,
            Option<&RandomNode>,
            Option<&ComputeNode>,
            Option<&ScalarNode>,
        ),
        With<GraphNode>,
    >,
    graph_links: Query<Entity, (With<GraphLink>, Without<ReducedViewLink>)>,
    unfinished_links: Query<Entity, With<UnfinishedLink>>,
    hidden_selections: Query<Entity, (With<Selected>, Or<(With<ComputeNode>, With<ScalarNode>)>)>,
    selection_indicators: Query<(Entity, &ChildOf), With<SelectedIndicator>>,
    old_reduced_links: Query<Entity, With<ReducedViewLink>>,
    node_data: Query<
        (
            Entity,
            Option<&RandomNode>,
            Option<&ComputeNode>,
            Option<&ScalarNode>,
        ),
        Or<(With<RandomNode>, With<ComputeNode>, With<ScalarNode>)>,
    >,
    transforms: Query<&Transform, With<GraphNode>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    reduced_view.active = !reduced_view.active;
    button_label.0 = if reduced_view.active {
        "Full view".to_string()
    } else {
        "Reduced view".to_string()
    };

    for entity in &unfinished_links {
        commands.entity(entity).despawn();
    }
    if reduced_view.active {
        let mut deselected = false;
        for selected in &hidden_selections {
            commands.entity(selected).remove::<Selected>();
            for (indicator, child_of) in &selection_indicators {
                if child_of.parent() == selected {
                    commands.entity(indicator).despawn();
                }
            }
            deselected = true;
        }
        if deselected {
            commands.trigger(ReloadSidebar);
            commands.trigger(CloseHistogramPanel);
        }
    }
    for entity in &old_reduced_links {
        commands.entity(entity).despawn();
    }

    apply_visibility(&mut commands, reduced_view.active, &nodes, &graph_links);
    if reduced_view.active {
        spawn_reduced_links(
            &mut commands,
            &node_data,
            &transforms,
            &mut meshes,
            &mut materials,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn refresh_reduced_view(
    reduced_view: Res<ReducedView>,
    mut commands: Commands,
    changed_nodes: Query<
        (),
        (
            With<GraphNode>,
            Or<(
                Changed<RandomNode>,
                Changed<ComputeNode>,
                Changed<ScalarNode>,
                Changed<Transform>,
            )>,
        ),
    >,
    changed_links: Query<(), Changed<GraphLink>>,
    mut removed_nodes: RemovedComponents<GraphNode>,
    mut removed_links: RemovedComponents<GraphLink>,
    nodes: Query<
        (
            Entity,
            Option<&RandomNode>,
            Option<&ComputeNode>,
            Option<&ScalarNode>,
        ),
        With<GraphNode>,
    >,
    graph_links: Query<Entity, (With<GraphLink>, Without<ReducedViewLink>)>,
    old_reduced_links: Query<Entity, With<ReducedViewLink>>,
    node_data: Query<
        (
            Entity,
            Option<&RandomNode>,
            Option<&ComputeNode>,
            Option<&ScalarNode>,
        ),
        Or<(With<RandomNode>, With<ComputeNode>, With<ScalarNode>)>,
    >,
    transforms: Query<&Transform, With<GraphNode>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let removed = removed_nodes.read().next().is_some() || removed_links.read().next().is_some();
    if !reduced_view.active || (changed_nodes.is_empty() && changed_links.is_empty() && !removed) {
        return;
    }

    for entity in &old_reduced_links {
        commands.entity(entity).despawn();
    }
    apply_visibility(&mut commands, true, &nodes, &graph_links);
    spawn_reduced_links(
        &mut commands,
        &node_data,
        &transforms,
        &mut meshes,
        &mut materials,
    );
}

fn apply_visibility(
    commands: &mut Commands,
    reduced: bool,
    nodes: &Query<
        (
            Entity,
            Option<&RandomNode>,
            Option<&ComputeNode>,
            Option<&ScalarNode>,
        ),
        With<GraphNode>,
    >,
    graph_links: &Query<Entity, (With<GraphLink>, Without<ReducedViewLink>)>,
) {
    for (entity, random, compute, scalar) in nodes {
        if random.is_some() {
            commands.entity(entity).insert(Visibility::Inherited);
        } else if compute.is_some() || scalar.is_some() {
            commands.entity(entity).insert(if reduced {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            });
        }
    }
    for entity in graph_links {
        commands.entity(entity).insert(if reduced {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
}

fn spawn_reduced_links(
    commands: &mut Commands,
    node_data: &Query<
        (
            Entity,
            Option<&RandomNode>,
            Option<&ComputeNode>,
            Option<&ScalarNode>,
        ),
        Or<(With<RandomNode>, With<ComputeNode>, With<ScalarNode>)>,
    >,
    transforms: &Query<&Transform, With<GraphNode>>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) {
    let mut dependencies = HashSet::new();
    for (target_entity, random, _, _) in node_data {
        let Some(random) = random else {
            continue;
        };
        for parameter in &random.params {
            let Some(source) = parameter.1 else {
                continue;
            };
            collect_nearest_randoms(
                source,
                node_data,
                &mut HashSet::new(),
                &mut dependencies,
                target_entity,
            );
        }
    }

    for (source, target) in dependencies {
        if source == target {
            continue;
        }
        let (Ok(from), Ok(to)) = (transforms.get(source), transforms.get(target)) else {
            continue;
        };
        spawn_link_visual(
            commands,
            (ReducedViewLink, Pickable::IGNORE),
            from.translation,
            to.translation,
            RANDOM_NODE_RAD,
            RANDOM_NODE_RAD,
            meshes,
            materials,
        );
    }
}

fn collect_nearest_randoms(
    entity: Entity,
    node_data: &Query<
        (
            Entity,
            Option<&RandomNode>,
            Option<&ComputeNode>,
            Option<&ScalarNode>,
        ),
        Or<(With<RandomNode>, With<ComputeNode>, With<ScalarNode>)>,
    >,
    visiting: &mut HashSet<Entity>,
    dependencies: &mut HashSet<(Entity, Entity)>,
    target: Entity,
) {
    if !visiting.insert(entity) {
        return;
    }
    if let Ok((_, random, compute, _scalar)) = node_data.get(entity) {
        if random.is_some() {
            dependencies.insert((entity, target));
        } else if let Some(compute) = compute {
            for parameter in &compute.params {
                if let Some(source) = parameter.1 {
                    collect_nearest_randoms(source, node_data, visiting, dependencies, target);
                }
            }
        }
    }
    visiting.remove(&entity);
}
