use bevy::{asset::RenderAssetUsages, mesh::{Indices, PrimitiveTopology}, prelude::*};
use crate::{constants::*, graph::UnfinishedLink};
use crate::{ERR_BORDER_COLOR, nodes::SelectedIndicator};
use bevy::window::{CursorIcon, CustomCursor, CustomCursorImage};

#[derive(Event)]
pub struct ErrorToast {
    pub text: String,
    pub color: Color
}

#[derive(Resource)]
pub struct CursorAssets {
    pub shift_held: Handle<Image>,
    pub finish_link: Handle<Image>,
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum GraphCursorState {
    #[default]
    Default,
    ShiftHeld,
    FinishLink,
}

#[derive(Component)]
pub struct ErrorToastBox {
    pub timer: Timer,
}

fn set_cursor_image(
    commands: &mut Commands,
    window_entity: Entity,
    cursor_assets: &CursorAssets,
    state: GraphCursorState
) {
    commands.entity(window_entity).insert(CursorIcon::Custom(
        CustomCursor::Image(CustomCursorImage {
            handle: match state {
                GraphCursorState::ShiftHeld => cursor_assets.shift_held.clone(),
                GraphCursorState::FinishLink => cursor_assets.finish_link.clone(),
                GraphCursorState::Default => cursor_assets.shift_held.clone()
            },
            texture_atlas: None,
            flip_x: false,
            flip_y: false,
            rect: None,
            hotspot: match state {
                GraphCursorState::ShiftHeld => (16, 1),
                GraphCursorState::FinishLink => (16, 11),
                _ => (0, 0)
            },
        }),
    ));
}

pub fn update_graph_cursor(
    mut commands: Commands,
    asset_server: Res<CursorAssets>,
    input: Res<ButtonInput<KeyCode>>,
    window: Single<Entity, With<Window>>,
    unfinished_link: Query<Entity, With<UnfinishedLink>>,
) {
    let next_cursor = if !unfinished_link.is_empty() {
        GraphCursorState::FinishLink
    } else if input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        GraphCursorState::ShiftHeld
    } else {
        GraphCursorState::Default
    };
    match next_cursor {
        GraphCursorState::Default => {
            commands.entity(*window).remove::<CursorIcon>();
        }
        GraphCursorState::ShiftHeld => {
            set_cursor_image(
                &mut commands,
                *window,
                &asset_server,
                GraphCursorState::ShiftHeld
            );
        }
        GraphCursorState::FinishLink => {
            set_cursor_image(
                &mut commands,
                *window,
                &asset_server,
                GraphCursorState::FinishLink
            );
        }
    }
}


pub fn spin_selection_indicators(
    time: Res<Time>,
    mut indicators: Query<(&SelectedIndicator, &mut Transform)>,
) {
    for (_indicator, mut transform) in indicators.iter_mut() {
        transform.rotate_z(SELECTION_SPIN_SPEED * time.delta_secs());
    }
}

pub fn selection_indicator(inner_radius: f32) -> Mesh {
    let outer_radius = inner_radius + 6.0;
    let segments_per_arc = 24;
    let gap_radians: f32 = 0.18; // gap between segments in radians
    let full_arc = (2.0 * std::f32::consts::PI - 3.0 * gap_radians) / 3.0;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for seg in 0..3 {
        let start_angle = seg as f32 * (full_arc + gap_radians) + gap_radians / 2.0;
        let base = positions.len() as u32;

        for i in 0..=segments_per_arc {
            let angle = start_angle + (i as f32 / segments_per_arc as f32) * full_arc;
            let (sin, cos) = angle.sin_cos();
            positions.push([cos * inner_radius, sin * inner_radius, 0.0]);
            positions.push([cos * outer_radius, sin * outer_radius, 0.0]);
        }

        for i in 0..segments_per_arc {
            let i = i as u32;
            let inner_cur  = base + i * 2;
            let outer_cur  = base + i * 2 + 1;
            let inner_next = base + i * 2 + 2;
            let outer_next = base + i * 2 + 3;
            indices.extend_from_slice(&[inner_cur, outer_cur, inner_next]);
            indices.extend_from_slice(&[outer_cur, outer_next, inner_next]);
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices))
}


pub fn throw_err(
    event: On<ErrorToast>,
    mut commands: Commands,
) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: px(24.),
            left: percent(50.),
            width: px(420.),
            min_height: px(40.),
            padding: px(12.).all(),
            border: px(2.).all(),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(event.color),
        BorderColor::all(ERR_BORDER_COLOR),
        ErrorToastBox {
            timer: Timer::from_seconds(10.0, TimerMode::Once),
        },
        Button,
        ZIndex(999),
        children![(
            Text::new(event.text.clone()),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(14.),
                ..default()
            },
        )],
    ));
}

pub fn tick_error_toasts(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut ErrorToastBox)>,
) {
    for (entity, mut toast) in &mut q {
        toast.timer.tick(time.delta());

        if toast.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

pub fn click_error_toasts(
    mut commands: Commands,
    q: Query<(Entity, &Interaction), (Changed<Interaction>, With<ErrorToastBox>)>,
) {
    for (entity, interaction) in &q {
        if *interaction == Interaction::Pressed {
            commands.entity(entity).despawn();
        }
    }
}
