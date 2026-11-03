//! Confirmation before editing a graph with retained inference results.
use super::*;
use crate::bevy_to_fugue::*;
use crate::data_vis::{CloseHistogramPanel, PlateIndexSelections, SampleSelections};
use std::sync::atomic::Ordering;

const GRAPH_CHANGE_WARNING: &str = "Changing the graph will remove all results from the last inference run and require recompilation.";

#[derive(Event)]
pub struct RequestGraphChangeConfirmation {
    pub pos: Option<Vec2>,
}

#[derive(Component)]
pub(crate) struct GraphChangeConfirmation;

#[derive(Component)]
enum GraphChangeConfirmationChoice {
    Confirm,
    Cancel,
}

/// Returns `true` when a completed or partially completed inference must be
/// acknowledged before the requested edit can be made.
pub fn graph_change_requires_confirmation(
    commands: &mut Commands,
    inference_results: Option<&InferenceResultResource>,
    pos: Option<Vec2>,
) -> bool {
    if inference_results.is_none() {
        return false;
    }
    commands.trigger(RequestGraphChangeConfirmation { pos });
    true
}

pub fn on_request_graph_change_confirmation(
    event: On<RequestGraphChangeConfirmation>,
    mut commands: Commands,
    existing: Query<(), With<GraphChangeConfirmation>>,
) {
    if !existing.is_empty() {
        return;
    }
    commands.trigger(CloseContextMenus);

    let mut node = Node {
        position_type: PositionType::Absolute,
        flex_direction: FlexDirection::Column,
        width: px(390.0),
        padding: px(16.0).all(),
        row_gap: px(10.0),
        border: px(1.0).all(),
        border_radius: BorderRadius::all(px(8.0)),
        ..default()
    };
    if let Some(pos) = event.pos {
        node.left = px(pos.x);
        node.top = px(pos.y);
    } else {
        node.left = percent(50.0);
        node.top = percent(38.0);
    }

    let confirmation = commands
        .spawn((
            Name::new("graph change confirmation"),
            GraphChangeConfirmation,
            ZIndex(1000),
            node,
            BorderColor::all(Color::srgb(0.22, 0.48, 0.78)),
            BackgroundColor(Color::linear_rgb(0.07, 0.08, 0.11)),
        ))
        .observe(on_confirm_graph_change)
        .id();

    commands.entity(confirmation).with_children(|parent| {
        parent.spawn((
            Pickable::IGNORE,
            Text::new("Are you sure?"),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..text_font()
            },
            TextColor(Color::WHITE),
        ));
        parent.spawn((
            Pickable::IGNORE,
            Text::new(GRAPH_CHANGE_WARNING),
            text_font(),
            TextColor(Color::srgb(0.82, 0.85, 0.90)),
        ));
        parent.spawn((
            GraphChangeConfirmationChoice::Confirm,
            menu_item("Change graph"),
        ));
        parent.spawn((
            GraphChangeConfirmationChoice::Cancel,
            menu_item("Keep results"),
        ));
    });
}

fn on_confirm_graph_change(
    mut event: On<Pointer<Press>>,
    mut commands: Commands,
    choices: Query<&GraphChangeConfirmationChoice>,
    confirmations: Query<Entity, With<GraphChangeConfirmation>>,
    inference_job: Option<Res<InferenceJob>>,
) {
    let Ok(choice) = choices.get(event.original_event_target()) else {
        return;
    };
    event.propagate(false);
    for entity in &confirmations {
        commands.entity(entity).despawn();
    }

    if matches!(choice, GraphChangeConfirmationChoice::Cancel) {
        return;
    }
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

fn menu_item(text: &str) -> impl Bundle {
    (
        Button,
        Node {
            padding: px(7.0).all(),
            border_radius: BorderRadius::all(px(4.0)),
            ..default()
        },
        BackgroundColor(BUTTON_COLOR),
        children![(
            Pickable::IGNORE,
            Text::new(text.to_string()),
            text_font(),
            TextColor(Color::WHITE),
        )],
    )
}
