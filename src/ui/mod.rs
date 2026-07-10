use bevy::prelude::*;

#[derive(Event)]
pub struct ErrorToast {
    pub text: String,
    pub color: Color
}

#[derive(Component)]
pub struct ErrorToastBox {
    pub timer: Timer,
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
        BorderColor::from(Color::srgb(0.9, 0.15, 0.15)),
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
