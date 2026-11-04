//! Tool palette layout, actions, and hints.
use super::*;

pub(super) fn update_edit_button(
    view: Res<EditorView>,
    mut button: Single<&mut Node, With<EditModelButton>>,
) {
    button.display = if view.inspecting {
        Display::Flex
    } else {
        Display::None
    };
}

pub(super) fn spawn_toolbar(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
) {
    commands.insert_resource(LinkCursors {
        start: assets.load(CURSOR_LINK_START),
        finish: assets.load(CURSOR_FINISH_LINK),
    });
    commands
        .spawn((
            EditModelButton,
            Button,
            ToolButton {
                action: Action::Edit,
                tip: "edit",
            },
            Name::new("Edit model"),
            GlobalZIndex(50),
            Node {
                position_type: PositionType::Absolute,
                left: px(SIDEBAR_WIDTH + 12.),
                top: px(12.),
                width: px(38),
                height: px(38),
                display: Display::None,
                border: px(1).all(),
                border_radius: BorderRadius::all(px(9)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.98, 0.985, 1.)),
            BorderColor::all(Color::srgb(0.82, 0.85, 0.90)),
            children![(
                ImageNode::new(images.add(icon_image(Action::Edit))),
                Node {
                    width: px(22),
                    height: px(22),
                    ..default()
                },
                Pickable::IGNORE,
            )],
        ))
        .observe(toolbar_action)
        .observe(|mut e: On<Pointer<Press>>| e.propagate(false));
    let toolbar = commands
        .spawn((
            Toolbar,
            Name::new("Canvas tools"),
            GlobalZIndex(50),
            Node {
                position_type: PositionType::Absolute,
                min_height: px(58),
                flex_wrap: FlexWrap::Wrap,
                padding: px(8).all(),
                column_gap: px(4),
                row_gap: px(4),
                align_items: AlignItems::Center,
                border: px(1).all(),
                border_radius: BorderRadius::all(px(16)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.98, 0.985, 1.0)),
            BorderColor::all(Color::srgb(0.82, 0.85, 0.90)),
        ))
        .observe(|mut e: On<Pointer<Press>>| e.propagate(false))
        .observe(|mut e: On<Pointer<Click>>| e.propagate(false))
        .id();
    let grip = commands
        .spawn((
            Name::new("Drag toolbar"),
            Node {
                width: px(18),
                height: px(36),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            children![(
                ImageNode::new(images.add(icon_image(Action::Grip))),
                Node {
                    width: px(16),
                    height: px(24),
                    ..default()
                },
                Pickable::IGNORE
            )],
        ))
        .observe(
            |mut e: On<Pointer<Drag>>, mut pos: ResMut<PalettePosition>| {
                e.propagate(false);
                pos.offset += e.delta;
            },
        )
        .id();
    commands.entity(toolbar).add_child(grip);
    for (action, tip) in [
        (Action::Tool(Tool::Select), "selection"),
        (Action::Tool(Tool::Node), "node creation"),
        (Action::NodeType, "node type"),
        (Action::Tool(Tool::Edge), "link creation"),
        (Action::Tool(Tool::Plate), "plate creation"),
        (Action::Undo, "undo"),
        (Action::Redo, "redo"),
        (Action::ZoomOut, "zoom out"),
        (Action::ZoomIn, "zoom in"),
        (Action::Fit, "fit model"),
    ] {
        if matches!(action, Action::Undo | Action::ZoomOut) {
            let separator = commands
                .spawn((
                    Node {
                        width: px(1),
                        height: px(24),
                        margin: px(4).horizontal(),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.84, 0.87, 0.92)),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(toolbar).add_child(separator);
        }
        let button = commands
            .spawn((
                Button,
                ToolButton { action, tip },
                Name::new(tip),
                Node {
                    width: px(if action == Action::NodeType {
                        108.
                    } else if action == Action::Tool(Tool::Node) {
                        54.
                    } else {
                        36.
                    }),
                    height: px(38),
                    margin: UiRect {
                        left: px(if action == Action::ZoomIn { -10. } else { 0. }),
                        ..default()
                    },
                    flex_shrink: 0.,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(px(9)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .observe(toolbar_action)
            .observe(dismiss_toolbar_hint)
            .id();
        if action == Action::Tool(Tool::Node) {
            commands
                .entity(button)
                .observe(start_node_drag)
                .observe(move_node_drag)
                .observe(finish_node_drag);
            commands.entity(button).with_child((
                NodeAdderPreview,
                Node {
                    width: px(50),
                    height: px(36),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Pickable::IGNORE,
            ));
        }
        if action == Action::NodeType {
            commands.entity(button).with_child((
                NodeTypeLabel,
                Text::new("Random"),
                font(13.),
                TextColor(INK),
                Pickable::IGNORE,
            ));
            commands.entity(button).with_child((
                ImageNode::new(images.add(icon_image(Action::NodeType))),
                Node {
                    width: px(14),
                    height: px(14),
                    margin: px(4).left(),
                    ..default()
                },
                Pickable::IGNORE,
            ));
        } else if action != Action::Tool(Tool::Node) {
            let handle = images.add(icon_image(action));
            commands.entity(button).with_child((
                ImageNode::new(handle),
                Node {
                    width: px(22),
                    height: px(22),
                    ..default()
                },
                Pickable::IGNORE,
            ));
        }
        commands.entity(toolbar).add_child(button);
        if action == Action::ZoomOut {
            let label = commands
                .spawn((
                    ZoomLabel,
                    Text::new("100%"),
                    TextLayout::no_wrap().with_justify(Justify::Center),
                    font(12.),
                    TextColor(INK),
                    Node {
                        width: px(44),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(toolbar).add_child(label);
        }
    }
    commands.entity(toolbar).with_child((
        ToolbarTip,
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            bottom: percent(100),
            margin: px(10).bottom(),
            left: px(8),
            padding: px(8).all(),
            border_radius: BorderRadius::all(px(8)),
            max_width: px(460),
            ..default()
        },
        BackgroundColor(Color::srgb(0.13, 0.17, 0.24)),
        Visibility::Hidden,
        children![(
            ToolHint,
            Text::new(""),
            font(12.),
            TextColor(Color::WHITE),
            Pickable::IGNORE
        )],
    ));
}

fn toolbar_action(
    mut event: On<Pointer<Click>>,
    buttons: Query<&ToolButton>,
    mut editor: ResMut<Editor>,
    mut history: ResMut<crate::ui::history::HistoryRequest>,
    mut commands: Commands,
    mut camera: Single<(&Camera, &mut Projection, &mut Transform), With<Camera2d>>,
    window: Single<&Window>,
    nodes: Query<(&Transform, Option<&Plate>), (With<GraphNode>, Without<Camera2d>)>,
    unfinished: Query<Entity, With<UnfinishedLink>>,
    drag: Res<NodeDrag>,
    palette: Single<(&Node, &ComputedNode), With<Toolbar>>,
) {
    event.propagate(false);
    if event.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    let Ok(button) = buttons.get(event.event_target()) else {
        return;
    };
    if drag.active.is_some() {
        return;
    }
    match button.action {
        Action::Grip => {}
        Action::Edit => {
            commands.trigger(EditModel);
        }
        Action::Tool(tool) => {
            editor.tool = tool;
            editor.panning = false;
            for e in &unfinished {
                commands.entity(e).despawn();
            }
            commands.trigger(CloseContextMenus);
        }
        Action::NodeType => {
            let (node, computed) = *palette;
            let top = match node.top {
                Val::Px(y) => y,
                _ => 0.,
            };
            let height = (computed.size().y / window.scale_factor()).max(58.);
            let y = if top >= 180. {
                top - 180.
            } else {
                top + height + 8.
            };
            commands.trigger(crate::sidebar::OpenNodeTypeMenu {
                pos: Vec2::new((event.pointer_location.position.x - 50.).max(0.), y),
            });
        }
        Action::Undo => history.0 = Some(false),
        Action::Redo => history.0 = Some(true),
        Action::ZoomIn | Action::ZoomOut => {
            let anchor = Vec2::new((SIDEBAR_WIDTH + window.width()) / 2., window.height() / 2.);
            let (cam, projection, transform) = &mut *camera;
            zoom_at(
                cam,
                projection,
                transform,
                anchor,
                if button.action == Action::ZoomIn {
                    1. / 1.2
                } else {
                    1.2
                },
            );
        }
        Action::Fit => {
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for (t, plate) in &nodes {
                if let Some(p) = plate {
                    min = min.min(p.bounds.min);
                    max = max.max(p.bounds.max);
                } else {
                    min = min.min(t.translation.truncate() - Vec2::splat(50.));
                    max = max.max(t.translation.truncate() + Vec2::splat(50.));
                }
            }
            let (_, projection, t) = &mut *camera;
            if let Projection::Orthographic(p) = &mut **projection {
                if min.is_finite() {
                    fit_model(p, t, min, max, Vec2::new(window.width(), window.height()));
                } else {
                    p.scale = 1.;
                    t.translation.x = 0.;
                    t.translation.y = 0.;
                }
            }
        }
    }
}

pub(super) fn update_toolbar(
    editor: Res<Editor>,
    mode: Single<&NodeMode>,
    history: Res<crate::ui::history::History>,
    camera: Single<&Projection, With<Camera2d>>,
    mut buttons: Query<(&ToolButton, &Interaction, &mut BackgroundColor)>,
    mut zoom: Single<&mut Text, With<ZoomLabel>>,
    mut node_label: Single<&mut Text, (With<NodeTypeLabel>, Without<ZoomLabel>, Without<ToolHint>)>,
    mut tip: Single<&mut Text, (With<ToolHint>, Without<ZoomLabel>, Without<NodeTypeLabel>)>,
    mut tip_visibility: Single<&mut Visibility, With<ToolbarTip>>,
    mut icons: Query<(&ChildOf, &mut ImageNode)>,
    button_actions: Query<&ToolButton>,
    menus: Query<(), With<NodeTypeMenu>>,
    mut dismissed: ResMut<DismissedHint>,
    drag: Res<NodeDrag>,
) {
    let mut hover = None;
    for (b, i, mut bg) in &mut buttons {
        let enabled = match b.action {
            Action::Undo => history.can_undo(),
            Action::Redo => history.can_redo(),
            _ => true,
        };
        let selected = b.action == Action::Tool(editor.tool);
        bg.0 = if !enabled {
            Color::srgb(0.91, 0.92, 0.94)
        } else if selected || b.action == Action::NodeType {
            Color::srgb(0.75, 0.75, 0.75)
        } else if *i != Interaction::None {
            Color::srgb(0.85, 0.85, 0.85)
        } else {
            Color::NONE
        };
        if *i == Interaction::Hovered
            && dismissed.0 != Some(b.action)
            && menus.is_empty()
            && drag.active.is_none()
        {
            hover = Some(b.tip);
        }
    }
    if !buttons
        .iter()
        .any(|(b, i, _)| *i != Interaction::None && dismissed.0 == Some(b.action))
    {
        dismissed.0 = None;
    }
    for (parent, mut icon) in &mut icons {
        if let Ok(button) = button_actions.get(parent.parent()) {
            let enabled = match button.action {
                Action::Undo => history.can_undo(),
                Action::Redo => history.can_redo(),
                _ => true,
            };
            icon.color = if enabled {
                Color::WHITE
            } else {
                Color::srgba(1., 1., 1., 0.3)
            };
        }
    }
    zoom.0 = format!("{:.0}%", 100. / camera_scale(&camera));
    node_label.0 = format!(
        "{}",
        match mode.0 {
            NodeType::Random => "Random",
            NodeType::Compute => "Arithmetic",
            NodeType::Scalar => "Constant",
            NodeType::Vector => "Vector",
        }
    );
    if let Some(t) = hover {
        tip.0 = t.to_string();
        **tip_visibility = Visibility::Visible;
    } else {
        **tip_visibility = Visibility::Hidden;
    }
}

pub(super) fn position_toolbar(
    window: Single<&Window>,
    mut pos: ResMut<PalettePosition>,
    mut toolbar: Single<(&mut Node, &ComputedNode), With<Toolbar>>,
    view: Res<EditorView>,
    sidebars: Query<&Node, (With<crate::sidebar::LocalSidebar>, Without<Toolbar>)>,
    histograms: Query<
        &Node,
        (
            With<crate::data_vis::InferenceHistogramPanel>,
            Without<Toolbar>,
        ),
    >,
) {
    if view.inspecting {
        toolbar.0.display = Display::None;
        return;
    }
    let panel_state = u8::from(!sidebars.is_empty()) | (u8::from(!histograms.is_empty()) << 1);
    let panels_open = panel_state != 0;
    if panel_state != pos.panel_state {
        pos.offset = Vec2::ZERO;
        pos.panel_state = panel_state;
    }
    let right = sidebars
        .iter()
        .map(|node| match node.width {
            Val::Px(width) => width,
            _ => SIDEBAR_WIDTH,
        })
        .fold(0., f32::max);
    let bottom = histograms
        .iter()
        .map(|node| match node.height {
            Val::Px(height) => height,
            _ => crate::data_vis::HISTOGRAM_PANEL_HEIGHT,
        })
        .fold(0., f32::max);
    let left = SIDEBAR_WIDTH + 12.;
    let available = window.width() - right - left - 12.;
    let (node, computed) = &mut *toolbar;
    let height = (computed.size().y / window.scale_factor()).max(58.);
    // Do not draw controls over panels when the window leaves no usable canvas.
    if available < 140. || window.height() - bottom - 24. < height {
        node.display = Display::None;
        return;
    }
    node.display = Display::Flex;
    node.max_width = px(available);
    let width = (computed.size().x / window.scale_factor()).min(available);
    let max_x = window.width() - right - width - 12.;
    let max_y = window.height() - bottom - height - 12.;
    let base = Vec2::new(
        left + (available - width) / 2.,
        if panels_open { 12. } else { max_y - 12. },
    );
    let actual = (base + pos.offset).clamp(Vec2::new(left, 12.), Vec2::new(max_x, max_y));
    node.left = px(actual.x);
    node.top = px(actual.y);
}

pub(super) fn position_toolbar_hint(
    palette: Single<&Node, (With<Toolbar>, Without<ToolbarTip>)>,
    mut tip: Single<&mut Node, (With<ToolbarTip>, Without<Toolbar>)>,
) {
    let at_top = matches!(palette.top,Val::Px(y) if y<180.);
    tip.top = if at_top { percent(100) } else { Val::Auto };
    tip.bottom = if at_top { Val::Auto } else { percent(100) };
    tip.margin = if at_top {
        px(10).top()
    } else {
        px(10).bottom()
    };
}

fn dismiss_toolbar_hint(
    event: On<Pointer<Press>>,
    buttons: Query<&ToolButton>,
    mut dismissed: ResMut<DismissedHint>,
    mut tip: Single<&mut Visibility, With<ToolbarTip>>,
) {
    if let Ok(button) = buttons.get(event.event_target()) {
        dismissed.0 = Some(button.action);
        **tip = Visibility::Hidden;
    }
}
