//! Node type selection, previews, and drag-to-create interaction.
use super::*;

fn next_node_id(ids: &Query<&GraphNode>) -> u32 {
    let mut id = 1;
    while ids.iter().any(|n| n.0 == id) {
        id += 1;
    }
    id
}

/// A UI miniature of the real default node: its shape, color, and initial label.
fn node_thumbnail(commands: &mut Commands, kind: NodeType, id: u32, scale: f32) -> Entity {
    let frame = commands
        .spawn((
            Node {
                width: px(70. * scale),
                height: px(52. * scale),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let (diameter, color, label) = match kind {
        NodeType::Random => (RANDOM_NODE_RAD * 2., CANVAS_COLOR, id.to_string()),
        NodeType::Compute => (COMPUTE_NODE_RAD * 2., COMPUTE_NODE_COLOR, "+".into()),
        NodeType::Scalar => (SCALAR_NODE_RAD * 2., SCALAR_NODE_COLOR, "1.0".into()),
        NodeType::Vector => (SCALAR_NODE_RAD * 2., SCALAR_NODE_COLOR, "[1, 1, 1]".into()),
    };
    let width = diameter
        + if kind == NodeType::Random {
            random_node_straight_length(&label)
        } else {
            0.
        };
    let shape = commands
        .spawn((
            Node {
                width: px(width * scale),
                height: px(diameter * scale),
                border: px(if kind == NodeType::Random {
                    NODE_BORDER_WEIGHT * scale
                } else {
                    0.
                })
                .all(),
                border_radius: BorderRadius::MAX,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.,
                ..default()
            },
            BackgroundColor(color),
            BorderColor::all(RANDOM_NODE_COLOR),
            Pickable::IGNORE,
        ))
        .id();
    if matches!(kind, NodeType::Scalar | NodeType::Vector) {
        commands.entity(frame).with_child((
            Text::new(label),
            font(12. * scale),
            TextColor(NODE_NAME_COLOR),
            Node {
                position_type: PositionType::Absolute,
                top: px(2. * scale),
                ..default()
            },
            Pickable::IGNORE,
        ));
    } else {
        commands.entity(shape).with_child((
            Text::new(label),
            font(20. * scale),
            TextColor(NODE_NAME_COLOR),
            Pickable::IGNORE,
        ));
    }
    commands.entity(frame).add_child(shape);
    frame
}

pub(super) fn update_node_adder(
    mode: Single<&NodeMode>,
    ids: Query<&GraphNode>,
    preview: Single<Entity, With<NodeAdderPreview>>,
    children: Query<&Children>,
    mut commands: Commands,
    mut previous: Local<Option<(NodeType, u32)>>,
) {
    let key = (mode.0, next_node_id(&ids));
    if *previous == Some(key) {
        return;
    }
    *previous = Some(key);
    if let Ok(children) = children.get(*preview) {
        for child in children {
            commands.entity(*child).despawn();
        }
    }
    let thumbnail = node_thumbnail(&mut commands, key.0, key.1, 0.7);
    commands.entity(*preview).add_child(thumbnail);
}

pub(super) fn open_node_type_menu(
    event: On<crate::sidebar::OpenNodeTypeMenu>,
    mut commands: Commands,
    mode: Single<&NodeMode>,
    ids: Query<&GraphNode>,
    window: Single<&Window>,
) {
    commands.trigger(CloseContextMenus);
    let position = event.pos.clamp(
        Vec2::ZERO,
        Vec2::new(
            (window.width() - 228.).max(0.),
            (window.height() - 228.).max(0.),
        ),
    );
    let menu = commands
        .spawn((
            NodeTypeMenu,
            crate::sidebar::ContextMenu,
            Name::new("Node type"),
            GlobalZIndex(1000),
            Node {
                position_type: PositionType::Absolute,
                left: px(position.x),
                top: px(position.y),
                width: px(220),
                padding: px(6).all(),
                row_gap: px(3),
                flex_direction: FlexDirection::Column,
                border: px(1).all(),
                border_radius: BorderRadius::all(px(12)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.98, 0.985, 1.)),
            BorderColor::all(Color::srgb(0.82, 0.85, 0.90)),
        ))
        .observe(|mut e: On<Pointer<Press>>| e.propagate(false))
        .id();
    for (kind, name) in [
        (NodeType::Random, "Random"),
        (NodeType::Compute, "Arithmetic"),
        (NodeType::Scalar, "Constant"),
        (NodeType::Vector, "Vector"),
    ] {
        let selected = kind == mode.0;
        let row = commands
            .spawn((
                Button,
                NodeTypeOption(kind),
                Node {
                    width: percent(100),
                    height: px(49),
                    column_gap: px(8),
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(px(8)),
                    ..default()
                },
                BackgroundColor(if selected {
                    Color::srgb(0.82, 0.88, 1.)
                } else {
                    Color::NONE
                }),
            ))
            .observe(select_node_type)
            .id();
        let thumbnail = node_thumbnail(&mut commands, kind, next_node_id(&ids), 0.7);
        commands.entity(row).add_child(thumbnail).with_child((
            Text::new(name),
            font(14.),
            TextColor(INK),
            Pickable::IGNORE,
        ));
        commands.entity(menu).add_child(row);
    }
}

fn select_node_type(
    mut event: On<Pointer<Click>>,
    options: Query<&NodeTypeOption>,
    mut mode: Single<&mut NodeMode>,
    mut editor: ResMut<Editor>,
    mut commands: Commands,
) {
    event.propagate(false);
    if event.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    if let Ok(option) = options.get(event.event_target()) {
        mode.0 = option.0;
        editor.tool = Tool::Node;
        commands.trigger(CloseContextMenus);
    }
}

pub(super) fn style_node_options(
    mode: Single<&NodeMode>,
    mut options: Query<(&NodeTypeOption, &Interaction, &mut BackgroundColor)>,
) {
    for (option, interaction, mut bg) in &mut options {
        bg.0 = if option.0 == mode.0 {
            Color::srgb(0.82, 0.88, 1.)
        } else if *interaction != Interaction::None {
            Color::srgb(0.89, 0.92, 0.97)
        } else {
            Color::NONE
        };
    }
}

pub(super) fn start_node_drag(
    mut event: On<Pointer<DragStart>>,
    mode: Single<&NodeMode>,
    mut drag: ResMut<NodeDrag>,
    mut editor: ResMut<Editor>,
    ids: Query<&GraphNode>,
    reduced: Res<ReducedView>,
    mut commands: Commands,
) {
    event.propagate(false);
    if event.button != bevy::picking::pointer::PointerButton::Primary || reduced.active {
        return;
    }
    drag.active = Some(mode.0);
    editor.tool = Tool::Node;
    commands.trigger(CloseContextMenus);
    let ghost = commands
        .spawn((
            NodeDragGhost,
            GlobalZIndex(1100),
            Node {
                position_type: PositionType::Absolute,
                left: px(event.pointer_location.position.x - 35.),
                top: px(event.pointer_location.position.y - 26.),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let thumbnail = node_thumbnail(&mut commands, mode.0, next_node_id(&ids), 1.);
    commands.entity(ghost).add_child(thumbnail);
}

pub(super) fn move_node_drag(
    mut event: On<Pointer<Drag>>,
    mut ghosts: Query<&mut Node, With<NodeDragGhost>>,
) {
    event.propagate(false);
    for mut node in &mut ghosts {
        node.left = px(event.pointer_location.position.x - 35.);
        node.top = px(event.pointer_location.position.y - 26.);
    }
}

pub(super) fn finish_node_drag(
    mut event: On<Pointer<DragEnd>>,
    mut drag: ResMut<NodeDrag>,
    ghosts: Query<Entity, With<NodeDragGhost>>,
    mut commands: Commands,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    ids: Query<&GraphNode>,
    ui: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Option<&Pickable>,
    )>,
    inference: Option<Res<crate::bevy_to_fugue::InferenceResultResource>>,
    reduced: Res<ReducedView>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
) {
    event.propagate(false);
    for e in &ghosts {
        commands.entity(e).despawn();
    }
    let Some(kind) = drag.active.take() else {
        return;
    };
    let pointer = event.pointer_location.position;
    if reduced.active
        || !valid_drop(
            pointer,
            Vec2::new(window.width(), window.height()),
            over_ui(pointer, &ui, window.scale_factor()),
        )
    {
        return;
    }
    if crate::sidebar::global::graph_change_requires_confirmation(
        &mut commands,
        inference.as_deref(),
        Some(pointer),
    ) {
        return;
    }
    let (camera, t) = *camera;
    let Ok(position) = camera.viewport_to_world_2d(t, pointer) else {
        return;
    };
    let id = next_node_id(&ids);
    match kind {
        NodeType::Random => new_random(&mut commands, position.extend(1.), id, meshes, materials),
        NodeType::Compute => new_compute(&mut commands, position.extend(1.), id, meshes, materials),
        NodeType::Scalar => new_scalar(&mut commands, position.extend(1.), id, meshes, materials),
        NodeType::Vector => new_vector(&mut commands, position.extend(1.), id, meshes, materials),
    }
}

pub(super) fn valid_drop(pointer: Vec2, size: Vec2, over_ui: bool) -> bool {
    !over_ui && pointer.cmpge(Vec2::ZERO).all() && pointer.cmplt(size).all()
}

pub(super) fn close_node_menu_on_outside_press(
    event: On<Pointer<Press>>,
    menus: Query<Entity, With<NodeTypeMenu>>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    // Global pointer observers run even when a widget stops propagation.
    if event.event_target() != event.original_event_target() {
        return;
    }
    for menu in &menus {
        let mut target = event.original_event_target();
        loop {
            if target == menu {
                break;
            }
            if let Ok(parent) = parents.get(target) {
                target = parent.parent();
            } else {
                commands.entity(menu).despawn();
                break;
            }
        }
    }
}
