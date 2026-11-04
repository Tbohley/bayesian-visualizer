pub use super::confirmation::{
    graph_change_requires_confirmation, on_request_graph_change_confirmation,
};
pub use super::invalidation::invalidate_compilation_on_graph_change;

use super::*;
use crate::bevy_to_fugue::*;
use crate::constants::*;
use crate::data_vis::{
    HistogramSelectionControls, HistogramSelectionStatus, deselect_histogram_selection,
};
use bevy::{input_focus::InputFocus, prelude::*, text::EditableText, ui::InteractionDisabled};

const DISABLED_CONTROL_COLOR: Color = Color::srgb(0.35, 0.35, 0.35);
const DISABLED_TEXT_COLOR: Color = Color::srgb(0.65, 0.65, 0.65);

pub fn load_global_sidebar(mut commands: Commands, global: Query<(Entity, &GlobalSidebar)>) {
    for (sidebar_entity, _comp) in global.iter() {
        commands.entity(sidebar_entity).despawn();
    }
    let global_sidebar_entity = commands
        .spawn((
            GlobalSidebar,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.),
                top: px(0.),
                width: px(SIDEBAR_WIDTH),
                height: percent(100.),
                flex_direction: FlexDirection::Column,
                padding: px(16).all(),
                ..default()
            },
            BackgroundColor(DARK_GREY.into()),
        ))
        .observe(
            //sidebar observes clicks to close distribution context menu
            |_: On<Pointer<Press>>, mut commands: Commands| {
                commands.trigger(CloseContextMenus);
            },
        )
        .id();
    commands.entity(global_sidebar_entity).with_child((
        Text::new("Bayesian Visualizer"),
        text_font(),
        Node {
            margin: px(16).bottom(),
            ..default()
        },
        TextColor(NODE_NAME_COLOR),
    ));

    let load_preset_button = commands
        .spawn((
            Name::new("load_preset_button"),
            Button,
            Node {
                width: px(SIDEBAR_WIDTH * 0.75),
                height: px(30),
                border: UiRect::all(px(5)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::MAX,
                margin: px(4).bottom(),
                ..default()
            },
            BorderColor::all(BUTTON_COLOR),
            BackgroundColor(BUTTON_COLOR),
            children![(
                Pickable::IGNORE,
                Text::new("Load preset"),
                text_font(),
                TextColor(Color::WHITE),
                TextShadow::default(),
            )],
        ))
        .observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
            event.propagate(false);
            commands.trigger(OpenPresetMenu {
                pos: event.pointer_location.position,
            });
        })
        .id();
    commands
        .entity(global_sidebar_entity)
        .add_child(load_preset_button);

    let reduced_view_button = commands
        .spawn((
            Name::new("reduced_view_button"),
            Button,
            Node {
                width: px(SIDEBAR_WIDTH * 0.75),
                height: px(30),
                border: UiRect::all(px(5)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::MAX,
                margin: px(4).bottom(),
                ..default()
            },
            BorderColor::all(BUTTON_COLOR),
            BackgroundColor(BUTTON_COLOR),
            children![(
                ReducedViewButtonLabel,
                Pickable::IGNORE,
                Text::new("Reduced view"),
                text_font(),
                TextColor(Color::WHITE),
                TextShadow::default(),
            )],
        ))
        .observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
            event.propagate(false);
            commands.trigger(ToggleReducedView);
        })
        .id();
    commands
        .entity(global_sidebar_entity)
        .add_child(reduced_view_button);

    commands.entity(global_sidebar_entity).with_child(divider());

    let compile_button = commands
        .spawn((
            Name::new("compile_button"),
            Button,
            Node {
                width: px(SIDEBAR_WIDTH * 0.75),
                height: px(30),
                border: UiRect::all(px(5)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::MAX,
                margin: px(4).bottom(),
                ..default()
            },
            BorderColor::all(BUTTON_COLOR),
            BackgroundColor(BUTTON_COLOR),
            children![(
                Pickable::IGNORE,
                Text::new("Compile"),
                text_font(),
                TextColor(Color::WHITE),
                TextShadow::default(),
            )],
        ))
        .observe(|_event: On<Pointer<Press>>, mut commands: Commands| {
            commands.trigger(TriggerCompilation)
        })
        .id();

    commands
        .entity(global_sidebar_entity)
        .add_child(compile_button);
    commands.entity(global_sidebar_entity).with_child(divider());
    commands.entity(global_sidebar_entity).with_child((
        Text::new("Inference:"),
        text_font(),
        Node {
            margin: px(8.).bottom(),
            ..default()
        },
        TextColor(NODE_NAME_COLOR),
    ));

    let seed_box = inference_textbox(&mut commands, "random_seed", "", 0);
    commands.entity(seed_box).with_child((
        RandomSeedPlaceholder,
        Pickable::IGNORE,
        Text::new("random..."),
        text_font(),
        TextColor(DISABLED_TEXT_COLOR),
    ));
    add_inference_field(
        &mut commands,
        global_sidebar_entity,
        "random seed",
        seed_box,
    );
    let rounds_box = inference_textbox(&mut commands, "number_of_rounds", "5000", 2);
    commands.entity(rounds_box).insert(NumberOfWarmupTextbox);
    add_inference_field(
        &mut commands,
        global_sidebar_entity,
        "# warmup rounds",
        rounds_box,
    );

    let samples_box = inference_textbox(&mut commands, "number_of_samples", "15000", 1);
    commands.entity(samples_box).insert(NumberOfSamplesTextbox);
    add_inference_field(
        &mut commands,
        global_sidebar_entity,
        "# of samples",
        samples_box,
    );

    let inference_button = commands
        .spawn((
            Name::new("run_inference_button"),
            RequiresCompilation,
            InteractionDisabled,
            Pickable::IGNORE,
            Button,
            Node {
                width: px(SIDEBAR_WIDTH * 0.75),
                height: px(30),
                border: UiRect::all(px(5)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::MAX,
                margin: px(4).bottom(),
                ..default()
            },
            BorderColor::all(DISABLED_CONTROL_COLOR),
            BackgroundColor(DISABLED_CONTROL_COLOR),
            children![(
                InferenceRunButtonLabel,
                Pickable::IGNORE,
                Text::new("Run inference"),
                text_font(),
                TextColor(DISABLED_TEXT_COLOR),
            )],
        ))
        .observe(crate::bevy_to_fugue::inference::run_inference)
        .id();
    commands
        .entity(global_sidebar_entity)
        .add_child(inference_button);

    let progress_fill = commands
        .spawn((
            InferenceProgressFill,
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(0.0),
                height: percent(100.0),
                ..default()
            },
            BackgroundColor(SAMPLE_COLOR),
        ))
        .id();
    let progress_label = commands
        .spawn((
            InferenceProgressLabel,
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100.0),
                height: percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..text_font()
            },
            TextColor(Color::WHITE),
        ))
        .id();
    let progress = commands
        .spawn((
            InferenceProgressContainer,
            Node {
                display: Display::None,
                position_type: PositionType::Relative,
                width: px(SIDEBAR_WIDTH * 0.75),
                height: px(20.0),
                border: px(1.0).all(),
                margin: px(4.0).bottom(),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(Color::srgb(0.55, 0.57, 0.62)),
            BackgroundColor(Color::srgb(0.20, 0.21, 0.24)),
        ))
        .add_children(&[progress_fill, progress_label])
        .id();
    commands.entity(global_sidebar_entity).add_child(progress);

    let selection_controls = commands
        .spawn((
            HistogramSelectionControls,
            Node {
                display: Display::None,
                width: percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: px(6.0),
                margin: px(12.0).top(),
                ..default()
            },
        ))
        .id();
    let selection_status = commands
        .spawn((
            HistogramSelectionStatus,
            Pickable::IGNORE,
            Text::new(""),
            text_font(),
            TextColor(NODE_NAME_COLOR),
        ))
        .id();
    let deselect_button = commands
        .spawn((
            Button,
            Node {
                width: px(SIDEBAR_WIDTH * 0.75),
                height: px(30.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(BUTTON_COLOR),
            children![(
                Pickable::IGNORE,
                Text::new("Clear selections"),
                text_font(),
                TextColor(Color::WHITE),
            )],
        ))
        .observe(deselect_histogram_selection)
        .id();
    commands
        .entity(selection_controls)
        .add_children(&[selection_status, deselect_button]);
    commands
        .entity(global_sidebar_entity)
        .add_child(selection_controls);
}

fn inference_textbox(
    commands: &mut Commands,
    name: &'static str,
    value: &'static str,
    tab_index: i32,
) -> Entity {
    let textbox = commands
        .spawn((
            RequiresCompilation,
            InferenceTextbox { tab_index },
            InteractionDisabled,
            Pickable::IGNORE,
            Node {
                width: px(120.),
                min_height: px(25.),
                border: px(2).all(),
                padding: px(4).all(),
                ..default()
            },
            BorderColor::from(DISABLED_CONTROL_COLOR),
            BackgroundColor(DISABLED_CONTROL_COLOR),
            EditableText::new(value),
            text_font(),
            TextColor(DISABLED_TEXT_COLOR),
            TextLayout::no_wrap(),
            TextCursorStyle::default(),
            Name::new(format!("{name}_textbox")),
        ))
        .id();
    if name == "random_seed" {
        commands.entity(textbox).insert(RandomSeedTextbox);
    }
    textbox
}

fn add_inference_field(
    commands: &mut Commands,
    sidebar: Entity,
    label: &'static str,
    textbox: Entity,
) {
    let field = commands
        .spawn((
            Node {
                width: percent(100.),
                flex_direction: FlexDirection::Column,
                row_gap: px(2.),
                margin: px(6.).bottom(),
                ..default()
            },
            Name::new(format!("{label}_box")),
        ))
        .id();
    commands.entity(sidebar).add_child(field);
    commands
        .entity(field)
        .with_child((Text::new(label), text_font(), TextColor(NODE_NAME_COLOR)));
    commands.entity(field).add_child(textbox);
}

//un-grays out control panel when compilation is done
pub fn set_inference_controls_enabled(
    event: On<SetInferenceControlsEnabled>,
    mut commands: Commands,
    mut input_focus: ResMut<InputFocus>,
    mut controls: Query<
        (
            Entity,
            &mut BackgroundColor,
            &mut BorderColor,
            Option<&InferenceTextbox>,
            Option<&Children>,
        ),
        With<RequiresCompilation>,
    >,
    mut text_colors: Query<&mut TextColor, Without<RandomSeedPlaceholder>>,
    mut placeholder_color: Single<&mut TextColor, With<RandomSeedPlaceholder>>,
) {
    placeholder_color.0 = if event.0 {
        Color::WHITE
    } else {
        DISABLED_TEXT_COLOR
    };

    for (entity, mut background, mut border, textbox, children) in &mut controls {
        if event.0 {
            if textbox.is_some() {
                background.0 = DARK_GREY.into();
                *border = BorderColor::all(Color::from(SLATE_300));
            } else {
                background.0 = BUTTON_COLOR;
                *border = BorderColor::all(BUTTON_COLOR);
            }
        } else {
            background.0 = DISABLED_CONTROL_COLOR;
            *border = BorderColor::all(DISABLED_CONTROL_COLOR);
        }

        if event.0 {
            commands.entity(entity).remove::<InteractionDisabled>();
            commands.entity(entity).remove::<Pickable>();
            if let Some(textbox) = textbox {
                commands.entity(entity).insert(TabIndex(textbox.tab_index));
            }
        } else {
            commands
                .entity(entity)
                .insert((InteractionDisabled, Pickable::IGNORE));
            commands.entity(entity).remove::<TabIndex>();
            if input_focus.get() == Some(entity) {
                input_focus.clear();
            }
        }

        if let Some(children) = children {
            for child in children.iter() {
                if let Ok(mut text_color) = text_colors.get_mut(child) {
                    text_color.0 = if event.0 {
                        Color::WHITE
                    } else {
                        DISABLED_TEXT_COLOR
                    };
                }
            }
        }
        if textbox.is_some() {
            let mut text_color = text_colors
                .get_mut(entity)
                .expect("inference textboxes should have text colors");
            text_color.0 = if event.0 {
                Color::WHITE
            } else {
                DISABLED_TEXT_COLOR
            };
        }
    }
}

pub fn update_random_seed_placeholder(
    seed: Query<&EditableText, (With<RandomSeedTextbox>, Changed<EditableText>)>,
    mut placeholder: Single<&mut Visibility, With<RandomSeedPlaceholder>>,
) {
    for seed in &seed {
        **placeholder = if seed.value().to_string().is_empty() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}
