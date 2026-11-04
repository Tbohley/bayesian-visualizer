//! Observation affordances for nodes inside a dataset plate.
use super::*;

pub(super) fn build(
    commands: &mut Commands,
    sidebar: Entity,
    node: Entity,
    plate: Entity,
    dataset: &str,
    column: &str,
) {
    let assigned = column != "unobserved";
    let button = if assigned {
        commands.entity(sidebar).with_child((
            Text::new(format!("from: {dataset}")),
            text_font(),
            TextColor(NODE_NAME_COLOR),
            Node {
                width: percent(100),
                margin: px(3).bottom(),
                flex_shrink: 0.,
                ..default()
            },
        ));
        let row = commands
            .spawn((
                Name::new("observed_column_row"),
                Node {
                    width: percent(100),
                    display: Display::Grid,
                    grid_template_columns: vec![GridTrack::px(32.), GridTrack::flex(1.)],
                    column_gap: px(8),
                    align_items: AlignItems::Center,
                    margin: px(5).bottom(),
                    flex_shrink: 0.,
                    ..default()
                },
            ))
            .id();
        commands.entity(sidebar).add_child(row);
        let eye = eye_icon(commands);
        commands.entity(row).add_child(eye);
        let pill = commands
            .spawn((
                Name::new("node_observation_selector"),
                Button,
                Node {
                    width: percent(100),
                    min_width: px(0),
                    min_height: px(32),
                    padding: UiRect::axes(px(12), px(5)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(dropdown::FIELD_SURFACE),
                children![(
                    Pickable::IGNORE,
                    Text::new(column),
                    text_font(),
                    TextColor(Color::WHITE),
                    TextLayout::justify(Justify::Center),
                    Node {
                        width: percent(100),
                        min_width: px(0),
                        flex_shrink: 0.,
                        ..default()
                    },
                )],
            ))
            .id();
        commands.entity(row).add_child(pill);
        pill
    } else {
        let button = commands
            .spawn((
                Name::new("observe_from_data"),
                Button,
                Node {
                    align_self: AlignSelf::FlexStart,
                    min_height: px(28),
                    padding: UiRect::axes(px(10), px(5)),
                    margin: px(5).bottom(),
                    flex_shrink: 0.,
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.20, 0.36, 0.32)),
                children![(
                    Pickable::IGNORE,
                    Text::new("Observe from data?"),
                    TextFont {
                        font_size: FontSize::Px(13.),
                        ..text_font()
                    },
                    TextColor(Color::WHITE),
                )],
            ))
            .id();
        commands.entity(sidebar).add_child(button);
        button
    };
    commands.entity(button).observe(
        move |mut event: On<Pointer<Press>>, mut commands: Commands| {
            event.propagate(false);
            if event.button == bevy::picking::pointer::PointerButton::Primary {
                commands.trigger(OpenPlateMappingMenu {
                    plate,
                    node,
                    pos: event.pointer_location.position,
                });
            }
        },
    );
}

/// Almond outline, hollow iris, and five lashes, drawn without font glyphs.
fn eye_icon(commands: &mut Commands) -> Entity {
    let eye = commands
        .spawn((
            Name::new("Observed from data"),
            Pickable::IGNORE,
            Node {
                width: px(32),
                height: px(28),
                flex_shrink: 0.,
                ..default()
            },
        ))
        .id();
    let curve = |t: f32, side: f32| Vec2::new(3. + 26. * t, 17. + side * 32. * t * (1. - t));
    for side in [-1., 1.] {
        for step in 0..24 {
            eye_stroke(
                commands,
                eye,
                curve(step as f32 / 24., side),
                curve((step + 1) as f32 / 24., side),
            );
        }
    }
    for (t, direction) in [
        (0.08, Vec2::new(-4., -4.)),
        (0.28, Vec2::new(-2.5, -6.)),
        (0.5, Vec2::new(0., -7.)),
        (0.72, Vec2::new(2.5, -6.)),
        (0.92, Vec2::new(4., -4.)),
    ] {
        let start = curve(t, -1.);
        eye_stroke(commands, eye, start, start + direction);
    }
    commands.entity(eye).with_child((
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: px(11.5),
            top: px(12.5),
            width: px(9),
            height: px(9),
            border: px(2.3).all(),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(NODE_NAME_COLOR),
    ));
    eye
}

fn eye_stroke(commands: &mut Commands, eye: Entity, start: Vec2, end: Vec2) {
    let delta = end - start;
    let center = (start + end) * 0.5;
    commands.entity(eye).with_child((
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: px(center.x - delta.length() * 0.5),
            top: px(center.y - 1.2),
            width: px(delta.length() + 0.4),
            height: px(2.4),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        UiTransform::from_rotation(Rot2::radians(delta.y.atan2(delta.x))),
        BackgroundColor(NODE_NAME_COLOR),
    ));
}

#[cfg(test)]
#[path = "tests/observation.rs"]
mod tests;
