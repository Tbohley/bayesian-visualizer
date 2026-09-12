use bevy::{asset::RenderAssetUsages, mesh::{Indices, PrimitiveTopology}, prelude::*};
use crate::graph::UnfinishedLink;
use crate::data_vis::HistogramView;
use crate::ERR_BORDER_COLOR;
use bevy::window::{CursorIcon, CustomCursor, CustomCursorImage};

#[derive(Event)]
pub struct ErrorToast {
    pub text: String,
    pub color: Color
}

#[derive(Event)]
pub struct ClearToasts;

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
    histogram_views: Query<(), With<HistogramView>>,
) {
    let next_cursor = if !unfinished_link.is_empty() {
        GraphCursorState::FinishLink
    } else if histogram_views.is_empty()
        && input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
    {
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

pub fn capsule_selection_indicator(inner_radius: f32, straight_length: f32) -> Mesh {
    let outer_radius = inner_radius + 6.0;
    let reference_radius = (inner_radius + outer_radius) / 2.0;
    let perimeter = 2.0 * straight_length + 2.0 * std::f32::consts::PI * reference_radius;
    let sample_count = 120;
    let gap_fraction = 4.0 / perimeter;

    let mut positions = Vec::new();
    let mut indices = Vec::new();

    for sample in 0..sample_count {
        let t0 = sample as f32 / sample_count as f32;
        let t1 = (sample + 1) as f32 / sample_count as f32;
        let midpoint = (t0 + t1) / 2.0;
        let in_gap = [0.0, 1.0 / 3.0, 2.0 / 3.0].iter().any(|center| {
            let distance = (midpoint - center).abs();
            distance.min(1.0 - distance) < gap_fraction / 2.0
        });
        if in_gap {
            continue;
        }

        let base = positions.len() as u32;
        positions.push(capsule_perimeter_point(
            inner_radius,
            straight_length,
            reference_radius,
            t0,
        ));
        positions.push(capsule_perimeter_point(
            outer_radius,
            straight_length,
            reference_radius,
            t0,
        ));
        positions.push(capsule_perimeter_point(
            inner_radius,
            straight_length,
            reference_radius,
            t1,
        ));
        positions.push(capsule_perimeter_point(
            outer_radius,
            straight_length,
            reference_radius,
            t1,
        ));
        indices.extend_from_slice(&[base, base + 1, base + 2]);
        indices.extend_from_slice(&[base + 1, base + 3, base + 2]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices))
}

fn capsule_perimeter_point(
    radius: f32,
    straight_length: f32,
    reference_radius: f32,
    t: f32,
) -> [f32; 3] {
    let straight = straight_length;
    let arc = std::f32::consts::PI * reference_radius;
    let perimeter = 2.0 * straight + 2.0 * arc;
    let distance = t * perimeter;
    let half_straight = straight / 2.0;

    let point = if distance < straight {
        Vec2::new(-half_straight + distance, radius)
    } else if distance < straight + arc {
        let progress = (distance - straight) / arc;
        let angle = std::f32::consts::FRAC_PI_2 - progress * std::f32::consts::PI;
        Vec2::new(
            half_straight + radius * angle.cos(),
            radius * angle.sin(),
        )
    } else if distance < 2.0 * straight + arc {
        let progress = (distance - straight - arc) / straight.max(f32::EPSILON);
        Vec2::new(half_straight - progress * straight, -radius)
    } else {
        let progress = (distance - 2.0 * straight - arc) / arc;
        let angle = -std::f32::consts::FRAC_PI_2 - progress * std::f32::consts::PI;
        Vec2::new(
            -half_straight + radius * angle.cos(),
            radius * angle.sin(),
        )
    };
    [point.x, point.y, 0.0]
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
                ..crate::constants::text_font()
            },
        )],
    ));
}

pub fn clear_toasts(
    _event: On<ClearToasts>,
    mut commands: Commands,
    toasts: Query<Entity, With<ErrorToastBox>>,
) {
    for toast in &toasts {
        commands.entity(toast).despawn();
    }
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
