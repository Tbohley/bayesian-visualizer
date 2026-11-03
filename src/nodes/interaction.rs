//! Canvas node creation, selection, and graph-link interaction.
use super::*;
use crate::bevy_to_fugue::InferenceResultResource;
use crate::constants::*;
use crate::data_vis::{
    CloseHistogramPanel, HistogramView, OpenHistogramPanel, OpenJointDistributionView,
};
use crate::graph::*;
use crate::sidebar::global::graph_change_requires_confirmation;
use crate::sidebar::*;
use crate::ui::*;
use bevy::prelude::*;

//create a node on canvas
pub fn on_background_click(
    event: On<Pointer<Click>>,
    reduced_view: Res<ReducedView>,
    editor: Res<crate::ui::editor::Editor>,
    input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    current_nodes: Query<&GraphNode>,
    selected: Option<Single<(Entity, &mut Selected)>>,
    node_mode: Single<&NodeMode>,
    selection_indicators: Query<(Entity, &ChildOf), With<SelectedIndicator>>,
    mut unfinished_links: Query<Entity, With<UnfinishedLink>>,
    plate_drafts: Query<&Plate, With<PlateDraft>>,
    inference_results: Option<Res<InferenceResultResource>>,
) {
    if event.button != bevy::picking::pointer::PointerButton::Primary
        || editor.dragged
        || input.pressed(KeyCode::Space)
    {
        return;
    }
    commands.trigger(CloseHistogramPanel);
    // Bevy emits Click before DragEnd when a drag is released. Do not treat
    // a real plate gesture as a normal background click. Tiny pointer jitter
    // remains a click; its undersized draft is removed by DragEnd.
    if plate_drafts
        .iter()
        .any(|plate| plate.bounds.is_substantial())
    {
        return;
    }

    for entity in unfinished_links.iter_mut() {
        commands.entity(entity).despawn();
    }
    if let Some(single) = selected {
        let (entity, _selected_comp) = single.into_inner();
        //deselect currently selected node + close context menus
        commands.entity(entity).remove::<Selected>();
        for (indicator_entity, child_of) in selection_indicators.iter() {
            if child_of.parent() == entity {
                commands.entity(indicator_entity).despawn();
            }
        }
        commands.trigger(CloseContextMenus);
        commands.trigger(ReloadSidebar);
    }
    if reduced_view.active || editor.tool != crate::ui::editor::Tool::Node {
        return;
    }
    if graph_change_requires_confirmation(
        &mut commands,
        inference_results.as_deref(),
        Some(event.pointer_location.position),
    ) {
        return;
    }
    let mut node_num = 1;
    //finds the lowest unused node in the least efficient way possible
    while current_nodes.iter().any(|node| node.0 == node_num) {
        node_num += 1;
    }
    println!("Created node #{}", node_num);

    let loc = event.hit.position.unwrap();

    match node_mode.into_inner().0 {
        NodeType::Random => new_random(&mut commands, loc, node_num, meshes, materials),
        NodeType::Compute => new_compute(&mut commands, loc, node_num, meshes, materials),
        NodeType::Scalar => new_scalar(&mut commands, loc, node_num, meshes, materials),
        NodeType::Vector => new_vector(&mut commands, loc, node_num, meshes, materials),
    };
}

// Select nodes normally; the link tool connects a source and destination.
pub fn on_node_click(
    event: On<Pointer<Click>>,
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    mut unfinished_link: Query<(Entity, &mut GraphLink), With<UnfinishedLink>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    transforms: Query<&mut Transform>,
    selected: Option<Single<(Entity, &mut Selected)>>,
    selection_indicators: Query<(Entity, &ChildOf), With<SelectedIndicator>>,
    node_ids: Query<&GraphNode>,
    inference_results: Option<Res<InferenceResultResource>>,
    reduced_view: Res<ReducedView>,
    random_nodes: Query<&RandomNode>,
    compute_nodes: Query<&ComputeNode>,
    scalar_nodes: Query<(), Or<(With<ScalarNode>, With<VectorNode>)>>,
    histogram_view: Option<Single<&HistogramView>>,
    editor: Res<crate::ui::editor::Editor>,
) {
    if event.button != bevy::picking::pointer::PointerButton::Primary
        || input.pressed(KeyCode::Space)
        || editor.dragged
    {
        return;
    }
    let shift_held = input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let joint_pair = histogram_view.as_ref().and_then(|view| {
        let y_node_id = node_ids.get(event.event_target()).ok()?.0;
        let crate::data_vis::HistogramSubject::Node(x_node_id) = view.subject else {
            return None;
        };
        let results = inference_results.as_ref()?;
        (results.0.node_extents.contains_key(&x_node_id)
            && results.0.node_extents.contains_key(&y_node_id)
            && y_node_id != x_node_id)
            .then_some((x_node_id, y_node_id))
    });
    //if there is an unfinished GraphLink, complete it.
    if !reduced_view.active
        && let Ok((unfinished_ent, mut ends)) = unfinished_link.single_mut()
    {
        if graph_change_requires_confirmation(
            &mut commands,
            inference_results.as_deref(),
            Some(event.pointer_location.position),
        ) {
            return;
        }
        let target = event.event_target();
        let free_input = random_nodes
            .get(target)
            .map(|n| &n.params)
            .ok()
            .or_else(|| compute_nodes.get(target).map(|n| &n.params).ok())
            .is_some_and(|params| {
                params.iter().any(|p| p.1.is_none())
                    && !params.iter().any(|p| p.1 == Some(ends.from))
            });
        if !free_input && ends.from != target {
            commands.trigger(ErrorToast {
                text: "Choose a destination with a free input parameter.".into(),
                color: ERR_COLOR,
            });
            return;
        }
        commands.entity(unfinished_ent).remove::<UnfinishedLink>();
        commands.entity(unfinished_ent).remove::<Pickable>();
        commands
            .entity(unfinished_ent)
            .observe(crate::ui::editor::on_edge_click)
            .observe(crate::ui::editor::on_canvas_drag_start)
            .observe(crate::ui::editor::on_canvas_drag)
            .observe(crate::ui::editor::on_canvas_drag_end);

        //if user tries to create a link from a node to itself
        if ends.from == event.event_target() {
            commands.entity(unfinished_ent).despawn();
            return;
        }
        let target = event.event_target();
        ends.to = Some(target);
        commands.trigger(AutofillNextParam {
            node: target,
            linked_node: ends.from,
        });
        commands.trigger(ReloadSidebar);
        println!("Completed a GraphLink");

        //add arrow
        let from_shape = endpoint_shape(
            ends.from,
            &random_nodes,
            &compute_nodes,
            &scalar_nodes,
            &node_ids,
        );
        let to_shape = endpoint_shape(
            target,
            &random_nodes,
            &compute_nodes,
            &scalar_nodes,
            &node_ids,
        );
        if let Some((arrow_transform, arrow_mesh)) =
            link_transform_helper(&ends, &transforms, &mut meshes, from_shape, to_shape)
        {
            commands.entity(unfinished_ent).insert((
                arrow_mesh,
                MeshMaterial2d(materials.add(ARROW_COLOR)),
                arrow_transform,
            ));
        }
    // Joint posterior inspection is available in either graph view.
    } else if editor.tool == crate::ui::editor::Tool::Select
        && shift_held
        && let Some((x_node_id, y_node_id)) = joint_pair
    {
        commands.trigger(OpenJointDistributionView {
            x_node_id,
            y_node_id,
        });
    // Graph-link editing remains unavailable in reduced view.
    } else if !reduced_view.active && editor.tool == crate::ui::editor::Tool::Edge {
        commands.spawn((
            GraphLink {
                from: event.event_target(),
                to: None,
            },
            UnfinishedLink,
        ));
        println!("Created an UnfinishedLink");
    //normal click, select the node
    } else {
        if event.duration.as_millis() < 200 && event.count == 1 {
            println!("Selected a node");

            if let Some(single) = selected {
                let (entity, _selected_comp) = single.into_inner();
                //deselect currently selected node
                commands.entity(entity).remove::<Selected>();

                for (indicator_entity, child_of) in selection_indicators.iter() {
                    if child_of.parent() == entity {
                        commands.entity(indicator_entity).despawn();
                    }
                }
            }
            //select this node
            let selection_mesh = if let Ok(random) = random_nodes.get(event.event_target()) {
                let node_id = node_ids
                    .get(event.event_target())
                    .map(|node| node.0)
                    .unwrap_or_default();
                random_selection_mesh(&random_node_label(random, node_id))
            } else if scalar_nodes.contains(event.event_target()) {
                selection_indicator(SCALAR_NODE_RAD)
            } else if compute_nodes.contains(event.event_target()) {
                selection_indicator(COMPUTE_NODE_RAD)
            } else {
                selection_indicator(RANDOM_NODE_RAD)
            };
            commands
                .entity(event.event_target())
                .insert(Selected)
                .with_child((
                    SelectedIndicator,
                    Pickable::IGNORE,
                    Mesh2d(meshes.add(selection_mesh)),
                    MeshMaterial2d(materials.add(SELECTION_INDICATOR_COLOR)),
                    Transform::from_xyz(0.0, 0.0, 1.),
                ));

            commands.trigger(ReloadSidebar);

            if let Ok(node) = node_ids.get(event.event_target()) {
                if let Some(results) = inference_results.as_ref()
                    && random_nodes.contains(event.event_target())
                {
                    let subject = if results.0.node_extents.contains_key(&node.0) {
                        crate::data_vis::HistogramSubject::Node(node.0)
                    } else {
                        crate::data_vis::HistogramSubject::ObservedData(node.0)
                    };
                    commands.trigger(OpenHistogramPanel {
                        subject,
                        clear_toasts: true,
                    });
                } else {
                    commands.trigger(CloseHistogramPanel);
                }
            }
        }
    }
}

pub fn open_node_inspection(
    event: On<OpenNodeInspection>,
    mut commands: Commands,
    nodes: Query<(Entity, &GraphNode, &RandomNode)>,
    selected: Query<Entity, With<Selected>>,
    indicators: Query<Entity, With<SelectedIndicator>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Some((entity, id, random)) = nodes.iter().find(|(_, id, _)| id.0 == event.node_id) else {
        return;
    };
    for old in &selected {
        commands.entity(old).remove::<Selected>();
    }
    for old in &indicators {
        commands.entity(old).despawn();
    }
    commands.entity(entity).insert(Selected).with_child((
        SelectedIndicator,
        Pickable::IGNORE,
        Mesh2d(meshes.add(random_selection_mesh(&random_node_label(random, id.0)))),
        MeshMaterial2d(materials.add(SELECTION_INDICATOR_COLOR)),
        Transform::from_xyz(0., 0., 1.),
    ));
    commands.trigger(ReloadSidebar);
    commands.trigger(OpenHistogramPanel {
        subject: crate::data_vis::HistogramSubject::Node(event.node_id),
        clear_toasts: false,
    });
}
