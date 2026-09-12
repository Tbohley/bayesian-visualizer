use bevy::{
    input_focus::InputFocus,
    prelude::*,
    text::EditableText,
    ui::InteractionDisabled,
};
use std::sync::atomic::Ordering;
use super::*;
use crate::bayesian_core::GraphIR;
use crate::nodes::*;
use crate::constants::*;
use crate::bevy_to_fugue::*;
use crate::data_vis::{
    CloseHistogramPanel, HistogramSelectionControls, HistogramSelectionStatus, SampleSelections,
    deselect_histogram_selection,
};

const DISABLED_CONTROL_COLOR: Color = Color::srgb(0.35, 0.35, 0.35);
const DISABLED_TEXT_COLOR: Color = Color::srgb(0.65, 0.65, 0.65);

#[derive(Component)]
struct NodeTypeButtonLabel;


pub fn load_global_sidebar(
    mut commands: Commands,
    global: Query<(Entity, &GlobalSidebar)>,
){
    for (sidebar_entity, _comp) in global.iter(){
        commands.entity(sidebar_entity).despawn();
    }
    let global_sidebar_entity = commands.spawn((
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
        BackgroundColor(DARK_GREY.into())
    )).observe(
        //sidebar observes clicks to close distribution context menu
        |_: On<Pointer<Press>>, mut commands: Commands| {
            commands.trigger(CloseContextMenus);
    }).id();
    commands.entity(global_sidebar_entity).with_child(
        (
            Text::new("Bayesian Visualizer"),
            text_font(),
            Node {
                margin: px(16).bottom(),
                ..default()
            },
            TextColor(NODE_NAME_COLOR),
        ));

    let load_preset_button = commands.spawn((
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
    )).observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
        event.propagate(false);
        commands.trigger(OpenPresetMenu {
            pos: event.pointer_location.position,
        });
    }).id();
    commands.entity(global_sidebar_entity).add_child(load_preset_button);

    let reduced_view_button = commands.spawn((
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
    )).observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
        event.propagate(false);
        commands.trigger(ToggleReducedView);
    }).id();
    commands.entity(global_sidebar_entity).add_child(reduced_view_button);

    commands.entity(global_sidebar_entity).with_child(divider());

    commands.entity(global_sidebar_entity).with_child((
        Text::new("Node type:"),
        text_font(),
        Node {
            margin: px(8.).bottom(),
            ..default()
        },
        TextColor(NODE_NAME_COLOR),
    ));

    let nodemode_menu = commands.spawn((
        Name::new("node_mode_context_menu"),
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
        BorderColor::all(Color::BLACK),
        BackgroundColor(Color::BLACK),
        children![(
            NodeTypeButtonLabel,
            Pickable::IGNORE,
            Text::new("Random"),
            text_font(),
            TextColor(Color::WHITE),
            TextShadow::default(),
        )],
    )).observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
        event.propagate(false);
        println!("Clicked context menu");
        debug!("click: {}", event.pointer_location.position);
        commands.trigger(OpenNodeTypeMenu {
            pos: event.pointer_location.position,
        });
    }).id();

    let compile_button = commands.spawn((
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
    )).observe(
        |_event: On<Pointer<Press>>, 
        mut commands: Commands| 
        {
            commands.trigger(TriggerCompilation)
        }).id();

    let sample_button = commands.spawn((
        Name::new("sample_button"),
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
            Pickable::IGNORE,
            Text::new("Basic sample"),
            text_font(),
            TextColor(DISABLED_TEXT_COLOR),
        )],
    )).observe(compilation::global_sample).id();

    let posterior_sample_button = commands.spawn((
        Name::new("posterior_sample_button"),
        RequiresInference,
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
            Pickable::IGNORE,
            Text::new("Post sample"),
            text_font(),
            TextColor(DISABLED_TEXT_COLOR),
        )],
    )).observe(compilation::posterior_sample).id();

    commands.entity(global_sidebar_entity).add_child(nodemode_menu);
    commands.entity(global_sidebar_entity).with_child(divider());
    commands.entity(global_sidebar_entity).add_child(compile_button);
    commands.entity(global_sidebar_entity).add_child(sample_button);
    commands.entity(global_sidebar_entity).add_child(posterior_sample_button);
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
    let rounds_box = inference_textbox(&mut commands, "number_of_rounds", "1000", 2);
    commands.entity(rounds_box).insert(NumberOfWarmupTextbox);
    add_inference_field(
        &mut commands,
        global_sidebar_entity,
        "# warmup rounds",
        rounds_box,
    );

    let samples_box = inference_textbox(&mut commands, "number_of_samples", "1000", 1);
    commands.entity(samples_box).insert(NumberOfSamplesTextbox);
    add_inference_field(
        &mut commands,
        global_sidebar_entity,
        "# of samples",
        samples_box,
    );

    let inference_button = commands.spawn((
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
    )).observe(compilation::run_inference).id();
    commands.entity(global_sidebar_entity).add_child(inference_button);

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
    //TODO: context menu for selecting which type of node to create
}

fn inference_textbox(
    commands: &mut Commands,
    name: &'static str,
    value: &'static str,
    tab_index: i32,
) -> Entity {
    let textbox = commands.spawn((
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
    )).id();
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
    let field = commands.spawn((
        Node {
            width: percent(100.),
            flex_direction: FlexDirection::Column,
            row_gap: px(4.),
            margin: px(8.).bottom(),
            ..default()
        },
        Name::new(format!("{label}_box")),
    )).id();
    commands.entity(sidebar).add_child(field);
    commands.entity(field).with_child((
        Text::new(label),
        text_font(),
        TextColor(NODE_NAME_COLOR),
    ));
    commands.entity(field).add_child(textbox);
}

//un-grays out control panel when compilation is done
pub fn set_inference_controls_enabled(
    event: On<SetInferenceControlsEnabled>,
    mut commands: Commands,
    mut input_focus: ResMut<InputFocus>,
    mut controls: Query<(
        Entity,
        &mut BackgroundColor,
        &mut BorderColor,
        Option<&InferenceTextbox>,
        Option<&Children>,
    ), With<RequiresCompilation>>,
    mut text_colors: Query<&mut TextColor, Without<RandomSeedPlaceholder>>,
    mut placeholder_color: Single<&mut TextColor, With<RandomSeedPlaceholder>>,
) {
    placeholder_color.0 = if event.0 { Color::WHITE } else { DISABLED_TEXT_COLOR };

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
            commands.entity(entity).insert((InteractionDisabled, Pickable::IGNORE));
            commands.entity(entity).remove::<TabIndex>();
            if input_focus.get() == Some(entity) {
                input_focus.clear();
            }
        }

        if let Some(children) = children {
            for child in children.iter() {
                if let Ok(mut text_color) = text_colors.get_mut(child) {
                    text_color.0 = if event.0 { Color::WHITE } else { DISABLED_TEXT_COLOR };
                }
            }
        }
        if textbox.is_some() {
            let mut text_color = text_colors
                .get_mut(entity)
                .expect("inference textboxes should have text colors");
            text_color.0 = if event.0 { Color::WHITE } else { DISABLED_TEXT_COLOR };
        }
    }
}

///un-grays out posterior sample button once inference has been run.
pub fn set_posterior_sample_enabled(
    event: On<SetPosteriorSampleEnabled>,
    mut commands: Commands,
    mut controls: Query<(
        Entity,
        &mut BackgroundColor,
        &mut BorderColor,
        Option<&Children>,
    ), With<RequiresInference>>,
    mut text_colors: Query<&mut TextColor>,
) {
    for (entity, mut background, mut border, children) in &mut controls {
        let color = if event.0 { BUTTON_COLOR } else { DISABLED_CONTROL_COLOR };
        background.0 = color;
        *border = BorderColor::all(color);

        if event.0 {
            commands.entity(entity).remove::<InteractionDisabled>();
            commands.entity(entity).remove::<Pickable>();
        } else {
            commands.entity(entity).insert((InteractionDisabled, Pickable::IGNORE));
        }

        if let Some(children) = children {
            for child in children.iter() {
                if let Ok(mut text_color) = text_colors.get_mut(child) {
                    text_color.0 = if event.0 { Color::WHITE } else { DISABLED_TEXT_COLOR };
                }
            }
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

//greys out controls on graph changes in order to require recompilation
pub fn invalidate_compilation_on_graph_change(
    mut commands: Commands,
    graph_resource: Option<Res<GraphIRResource>>,
    inference_job: Option<Res<InferenceJob>>,
    changed_graph: Query<
        (),
        (
            Or<(
                Changed<GraphNode>,
                Changed<RandomNode>,
                Changed<ComputeNode>,
                Changed<ScalarNode>,
                Changed<GraphLink>,
                Changed<Plate>,
            )>,
            Without<PlateDraft>,
        ),
    >,
    changed_positions: Query<(), (
        Changed<Transform>,
        With<GraphNode>,
        Without<Plate>,
        Without<PlateDraft>,
    )>,
    node_positions: Query<(&GraphNode, &Transform), (Without<Plate>, Without<PlateDraft>)>,
    plates: Query<(&GraphNode, &Plate), Without<PlateDraft>>,
    mut removed_graph_nodes: RemovedComponents<GraphNode>,
) {
    let Some(graph_resource) = graph_resource else {
        return;
    };
    let graph_node_removed = removed_graph_nodes.read().next().is_some();
    let membership_changed = !changed_positions.is_empty()
        && node_plate_membership_changed(
            graph_resource.0.graph(),
            &node_positions,
            &plates,
        );
    let graph_changed = !changed_graph.is_empty() || membership_changed || graph_node_removed;

    if graph_changed {
        if let Some(job) = inference_job {
            job.control.discard_result.store(true, Ordering::Relaxed);
            job.control.cancel_requested.store(true, Ordering::Relaxed);
        }
        commands.remove_resource::<GraphIRResource>();
        commands.remove_resource::<InferenceResultResource>();
        commands.remove_resource::<InferenceStatusResource>();
        commands.remove_resource::<SampleSelections>();
        commands.trigger(SetInferenceControlsEnabled(false));
        commands.trigger(SetPosteriorSampleEnabled(false));
        commands.trigger(CloseHistogramPanel);
    }
}

/// Node coordinates only affect the compiled model when they change direct
/// plate ownership. Ordinary dragging within the same scope is presentation-only.
fn node_plate_membership_changed(
    compiled: &GraphIR,
    node_positions: &Query<(&GraphNode, &Transform), (Without<Plate>, Without<PlateDraft>)>,
    plates: &Query<(&GraphNode, &Plate), Without<PlateDraft>>,
) -> bool {
    let current_plates = plates
        .iter()
        .filter(|(_, plate)| plate.bounds.is_substantial())
        .map(|(node, plate)| (node.0, plate.bounds))
        .collect::<Vec<_>>();

    if current_plates.len() != compiled.plates.len()
        || current_plates
            .iter()
            .any(|(id, _)| !compiled.plates.contains_key(id))
    {
        return true;
    }

    for &(plate_id, bounds) in &current_plates {
        let child_bounds = current_plates
            .iter()
            .filter(|(candidate_id, candidate_bounds)| {
                *candidate_id != plate_id && bounds.contains_bounds(*candidate_bounds)
            })
            .map(|(_, bounds)| *bounds)
            .collect::<Vec<_>>();
        let mut current_nodes = node_positions
            .iter()
            .filter(|(_, transform)| {
                let position = transform.translation.truncate();
                bounds.contains_point(position)
                    && !child_bounds
                        .iter()
                        .any(|child| child.contains_point(position))
            })
            .map(|(node, _)| node.0)
            .collect::<Vec<_>>();
        current_nodes.sort_unstable();

        let Some(compiled_plate) = compiled.plates.get(&plate_id) else {
            return true;
        };
        if current_nodes != compiled_plate.nodes {
            return true;
        }
    }

    false
}


//
fn on_set_node_mode(
    event: On<Pointer<Press>>,
    menu_items: Query<&ContextMenuItem>,
    mut commands: Commands,
    node_mode: Single<&mut NodeMode>,
    mut button_label: Single<&mut Text, With<NodeTypeButtonLabel>>,
){
    let target = event.original_event_target();

    if let Ok(item) = menu_items.get(target) {
        //set distribution of node to new dist... or maybe on apply?
        println!("Selected node creation type: {}", item.0);
        button_label.0 = item.0.clone();
        node_mode.into_inner().0 = match item.0.as_str() {
            "Random" => NodeType::Random,
            "Compute" => NodeType::Compute,
            "Scalar" => NodeType::Scalar,
            _ => NodeType::Random
        };
        commands.trigger(CloseContextMenus);
        commands.trigger(ReloadSidebar);
        
    }
    
}

pub fn on_open_node_type_menu(
    event: On<OpenNodeTypeMenu>, 
    mut commands: Commands,
) {
    commands.trigger(CloseContextMenus);
    let pos = event.pos;
    println!("open context menu at: {pos}");

    commands
        .spawn((
            Name::new("node type selector"),
            ContextMenu,
            ZIndex(999),
            Node {
                position_type: PositionType::Absolute,
                left: px(pos.x),
                top: px(pos.y),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BorderColor::all(Color::BLACK),
            BackgroundColor(Color::linear_rgb(0.1, 0.1, 0.1)),
            children![
                context_item("Random"),
                context_item("Compute"),
                context_item("Scalar"),
            ],
        ))
        .observe(on_set_node_mode);
}
