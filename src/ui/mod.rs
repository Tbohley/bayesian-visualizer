use bevy::{asset::RenderAssetUsages, mesh::{Indices, PrimitiveTopology}, prelude::*};

use crate::{ERR_BORDER_COLOR, nodes::SelectedIndicator};

#[derive(Event)]
pub struct ErrorToast {
    pub text: String,
    pub color: Color
}

#[derive(Component)]
pub struct ErrorToastBox {
    pub timer: Timer,
}


pub fn spin_selection_indicators(
    time: Res<Time>,
    mut indicators: Query<(&SelectedIndicator, &mut Transform)>,
) {
    for (indicator, mut transform) in indicators.iter_mut() {
        transform.rotate_z(0.5 * time.delta_secs());
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
