//! Canvas interaction and the floating tool palette. Icon paths live in `icon_segments`.
use crate::{constants::*, graph::*, nodes::*, sidebar::CloseContextMenus};
use bevy::{
    input::{
        ButtonInput,
        mouse::{MouseScrollUnit, MouseWheel},
    },
    input_focus::InputFocus,
    prelude::*,
    text::EditableText,
    window::{CursorIcon, CustomCursor, CustomCursorImage, SystemCursorIcon},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Select,
    Node,
    Edge,
    Plate,
}
#[derive(Resource, Default)]
pub struct Editor {
    pub tool: Tool,
    pub panning: bool,
    pub dragged: bool,
}
#[derive(Component)]
pub struct Toolbar;
#[derive(Component)]
struct EditModelButton;
#[derive(Resource, Default)]
pub struct EditorView {
    pub inspecting: bool,
}
#[derive(Event)]
pub struct InspectModel;
#[derive(Event)]
pub struct EditModel;
#[derive(Component)]
struct ToolHint;
#[derive(Component)]
struct ZoomLabel;
#[derive(Component)]
pub struct NodeTypeLabel;
#[derive(Component)]
struct ToolbarTip;
#[derive(Component)]
struct NodeAdderPreview;
#[derive(Component)]
struct NodeTypeMenu;
#[derive(Component)]
struct NodeTypeOption(NodeType);
#[derive(Component)]
struct NodeDragGhost;
#[derive(Resource, Default)]
struct NodeDrag {
    active: Option<NodeType>,
}
#[derive(Resource, Default)]
struct DismissedHint(Option<Action>);
#[derive(Resource)]
struct LinkCursors {
    start: Handle<Image>,
    finish: Handle<Image>,
}
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Action {
    Grip,
    Edit,
    Tool(Tool),
    NodeType,
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    Fit,
}
#[derive(Component)]
struct ToolButton {
    action: Action,
    tip: &'static str,
}
#[derive(Resource, Default)]
struct PalettePosition {
    offset: Vec2,
    panel_state: u8,
}

pub struct EditorPlugin;
impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Editor>()
            .init_resource::<EditorView>()
            .init_resource::<PalettePosition>()
            .init_resource::<NodeDrag>()
            .init_resource::<DismissedHint>()
            .add_observer(open_node_type_menu)
            .add_observer(inspect_model)
            .add_observer(edit_model)
            .add_observer(crate::nodes::open_node_inspection)
            .add_observer(close_node_menu_on_outside_press)
            .add_plugins(super::history::HistoryPlugin)
            .add_systems(Startup, spawn_toolbar)
            .add_systems(Update, follow_canvas.after(wheel_navigation))
            .add_systems(Update, position_toolbar_hint.after(position_toolbar))
            .add_systems(
                PreUpdate,
                reset_gesture.before(bevy::picking::PickingSystems::ProcessInput),
            )
            .add_systems(
                Update,
                (
                    editor_keys,
                    wheel_navigation,
                    update_toolbar,
                    position_toolbar,
                    update_edit_button,
                    update_editor_cursor,
                    update_edge_preview,
                    highlight_edges,
                    update_node_adder,
                    style_node_options,
                ),
            );
    }
}

fn update_edit_button(view: Res<EditorView>, mut button: Single<&mut Node, With<EditModelButton>>) {
    button.display = if view.inspecting {
        Display::Flex
    } else {
        Display::None
    };
}

fn inspect_model(
    _: On<InspectModel>,
    mut view: ResMut<EditorView>,
    mut editor: ResMut<Editor>,
    mut drag: ResMut<NodeDrag>,
    mut commands: Commands,
    unfinished: Query<Entity, Or<(With<UnfinishedLink>, With<PlateDraft>, With<NodeDragGhost>)>>,
) {
    view.inspecting = true;
    editor.tool = Tool::Select;
    editor.panning = false;
    drag.active = None;
    for entity in &unfinished {
        commands.entity(entity).despawn();
    }
    commands.trigger(CloseContextMenus);
}

fn edit_model(
    _: On<EditModel>,
    mut view: ResMut<EditorView>,
    mut editor: ResMut<Editor>,
    mut palette: ResMut<PalettePosition>,
    mut commands: Commands,
    sidebars: Query<Entity, With<crate::sidebar::LocalSidebar>>,
    selected: Query<Entity, With<Selected>>,
    indicators: Query<Entity, With<SelectedIndicator>>,
) {
    view.inspecting = false;
    editor.tool = Tool::Select;
    palette.offset = Vec2::ZERO;
    for entity in &selected {
        commands.entity(entity).remove::<Selected>();
    }
    for entity in &indicators {
        commands.entity(entity).despawn();
    }
    for entity in &sidebars {
        commands.entity(entity).despawn();
    }
    commands.trigger(crate::data_vis::CloseHistogramPanel);
    commands.trigger(CloseContextMenus);
}

fn font(size: f32) -> TextFont {
    TextFont {
        font_size: size.into(),
        ..text_font()
    }
}
const INK: Color = Color::srgb(0.25, 0.30, 0.39);

fn spawn_toolbar(
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
    mut history: ResMut<super::history::HistoryRequest>,
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

pub fn camera_scale(projection: &Projection) -> f32 {
    if let Projection::Orthographic(p) = projection {
        p.scale
    } else {
        1.
    }
}
fn zoom_at(
    camera: &Camera,
    projection: &mut Projection,
    transform: &mut Transform,
    pointer: Vec2,
    factor: f32,
) {
    let Some(size) = camera.logical_viewport_size() else {
        return;
    };
    apply_zoom(projection, transform, size, pointer, factor);
}
fn apply_zoom(
    projection: &mut Projection,
    transform: &mut Transform,
    size: Vec2,
    pointer: Vec2,
    factor: f32,
) {
    let Projection::Orthographic(p) = projection else {
        return;
    };
    let old = p.scale;
    p.scale = (old * factor).clamp(0.1, 8.);
    let offset = Vec2::new(pointer.x - size.x / 2., size.y / 2. - pointer.y);
    transform.translation += (offset * (old - p.scale)).extend(0.);
}

/// Hit-test UI in physical pixels so scroll and global panning cannot leak through panels.
pub fn over_ui(
    pointer: Vec2,
    ui: &Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Option<&Pickable>,
    )>,
    scale: f32,
) -> bool {
    ui.iter().any(|(node, t, v, p)| {
        v.get()
            && !p.is_some_and(|p| !p.should_block_lower)
            && node.contains_point(*t, pointer * scale)
    })
}
fn wheel_navigation(
    mut wheel: MessageReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window>,
    ui: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Option<&Pickable>,
    )>,
    mut camera: Single<(&Camera, &mut Projection, &mut Transform), With<Camera2d>>,
) {
    let events: Vec<_> = wheel.read().copied().collect();
    let Some(pointer) = window.cursor_position() else {
        return;
    };
    if over_ui(pointer, &ui, window.scale_factor()) {
        return;
    }
    let (cam, projection, transform) = &mut *camera;
    for e in events {
        let amount = if e.unit == MouseScrollUnit::Line {
            32.
        } else {
            1.
        };
        if keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
            zoom_at(
                cam,
                projection,
                transform,
                pointer,
                (-e.y * amount * 0.004).exp(),
            );
        } else {
            let scale = camera_scale(projection);
            transform.translation += Vec3::new(-e.x * amount * scale, e.y * amount * scale, 0.);
        }
    }
}

fn reset_gesture(mouse: Res<ButtonInput<MouseButton>>, mut editor: ResMut<Editor>) {
    if mouse.just_pressed(MouseButton::Left) {
        editor.dragged = false;
        editor.panning = false;
    }
}
pub fn on_canvas_drag_start(
    event: On<Pointer<DragStart>>,
    mut editor: ResMut<Editor>,
    keys: Res<ButtonInput<KeyCode>>,
    canvas: Query<(), With<Canvas>>,
) {
    editor.panning = event.button == bevy::picking::pointer::PointerButton::Primary
        && (keys.pressed(KeyCode::Space)
            || (canvas.contains(event.event_target()) && editor.tool == Tool::Select));
    editor.dragged = false;
}
pub fn on_canvas_drag(
    event: On<Pointer<Drag>>,
    mut editor: ResMut<Editor>,
    mut camera: Single<(&Projection, &mut Transform), With<Camera2d>>,
) {
    editor.dragged |= event.distance.length() > 3.;
    if !editor.panning {
        return;
    }
    let (p, t) = &mut *camera;
    let delta = Vec2::new(-event.delta.x, event.delta.y) * camera_scale(p);
    t.translation += delta.extend(0.);
}
pub fn on_canvas_drag_end(_: On<Pointer<DragEnd>>, mut editor: ResMut<Editor>) {
    editor.panning = false;
}

fn editor_keys(
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    editable: Query<(), With<EditableText>>,
    mut editor: ResMut<Editor>,
    mut commands: Commands,
    unfinished: Query<Entity, Or<(With<UnfinishedLink>, With<PlateDraft>, With<NodeDragGhost>)>>,
    mut drag: ResMut<NodeDrag>,
    mut history: ResMut<super::history::HistoryRequest>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        editor.tool = Tool::Select;
        drag.active = None;
        editor.panning = false;
        editor.dragged = false;
        for e in &unfinished {
            commands.entity(e).despawn();
        }
        commands.trigger(CloseContextMenus);
    }
    if focus.get().is_some_and(|e| editable.contains(e)) {
        return;
    }
    if keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]) {
        if keys.just_pressed(KeyCode::KeyZ) {
            history.0 = Some(keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]));
        } else if keys.just_pressed(KeyCode::KeyY) {
            history.0 = Some(true);
        }
    }
}
fn update_toolbar(
    editor: Res<Editor>,
    mode: Single<&NodeMode>,
    history: Res<super::history::History>,
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
        } else if selected {
            Color::srgb(0.82, 0.88, 1.)
        } else if *i != Interaction::None {
            Color::srgb(0.89, 0.92, 0.97)
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
        }
    );
    if let Some(t) = hover {
        tip.0 = t.to_string();
        **tip_visibility = Visibility::Visible;
    } else {
        **tip_visibility = Visibility::Hidden;
    }
}
fn position_toolbar(
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

fn position_toolbar_hint(
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

fn update_editor_cursor(
    mut commands: Commands,
    editor: Res<Editor>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<(Entity, &Window)>,
    ui: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Option<&Pickable>,
    )>,
    unfinished: Query<(), With<UnfinishedLink>>,
    cursors: Res<LinkCursors>,
) {
    let (entity, w) = *window;
    let over_panel = w
        .cursor_position()
        .is_some_and(|p| over_ui(p, &ui, w.scale_factor()));
    if !over_panel
        && !editor.panning
        && !keys.pressed(KeyCode::Space)
        && (editor.tool == Tool::Edge || !unfinished.is_empty())
    {
        let finishing = !unfinished.is_empty();
        commands
            .entity(entity)
            .insert(CursorIcon::Custom(CustomCursor::Image(CustomCursorImage {
                handle: if finishing {
                    cursors.finish.clone()
                } else {
                    cursors.start.clone()
                },
                texture_atlas: None,
                flip_x: false,
                flip_y: false,
                rect: None,
                hotspot: if finishing { (16, 11) } else { (16, 1) },
            })));
        return;
    }
    let icon = if w
        .cursor_position()
        .is_some_and(|p| over_ui(p, &ui, w.scale_factor()))
    {
        SystemCursorIcon::Default
    } else if editor.panning {
        SystemCursorIcon::Grabbing
    } else if keys.pressed(KeyCode::Space) {
        SystemCursorIcon::Grab
    } else if editor.tool != Tool::Select || !unfinished.is_empty() {
        SystemCursorIcon::Crosshair
    } else {
        SystemCursorIcon::Default
    };
    commands.entity(entity).insert(CursorIcon::System(icon));
}

// Small antialiased line icons, kept independent of button behavior for easy replacement.
fn icon_segments(action: Action) -> Vec<([f32; 2], [f32; 2])> {
    match action {
        Action::Edit => vec![
            ([5., 16.], [15., 6.]),
            ([15., 6.], [19., 10.]),
            ([19., 10.], [9., 20.]),
            ([9., 20.], [4., 21.]),
            ([4., 21.], [5., 16.]),
            ([13., 8.], [17., 12.]),
            ([17., 4.], [21., 8.]),
        ],
        Action::Grip => vec![
            ([8., 6.], [8., 6.5]),
            ([16., 6.], [16., 6.5]),
            ([8., 12.], [8., 12.5]),
            ([16., 12.], [16., 12.5]),
            ([8., 18.], [8., 18.5]),
            ([16., 18.], [16., 18.5]),
        ],
        Action::NodeType => vec![([7., 10.], [12., 15.]), ([12., 15.], [17., 10.])],
        Action::Tool(Tool::Select) => vec![
            ([5., 2.], [5., 20.]),
            ([5., 20.], [9., 16.]),
            ([9., 16.], [12., 22.]),
            ([12., 22.], [15., 20.5]),
            ([15., 20.5], [12., 14.5]),
            ([12., 14.5], [18., 14.5]),
            ([18., 14.5], [5., 2.]),
        ],
        Action::Tool(Tool::Node) => vec![
            ([12., 3.], [21., 12.]),
            ([21., 12.], [12., 21.]),
            ([12., 21.], [3., 12.]),
            ([3., 12.], [12., 3.]),
            ([8., 12.], [16., 12.]),
            ([12., 8.], [12., 16.]),
        ],
        Action::Tool(Tool::Edge) => vec![
            ([4., 20.], [20., 4.]),
            ([12., 4.], [20., 4.]),
            ([20., 4.], [20., 12.]),
        ],
        // The filled corner badge is rasterized by plate_pixel.
        Action::Tool(Tool::Plate) => vec![],
        Action::Undo => vec![
            ([9., 5.], [4., 10.]),
            ([4., 10.], [9., 15.]),
            ([4., 10.], [15., 10.]),
            ([15., 10.], [19., 13.]),
            ([19., 13.], [19., 18.]),
        ],
        Action::Redo => icon_segments(Action::Undo)
            .into_iter()
            .map(|(a, b)| ([24. - a[0], a[1]], [24. - b[0], b[1]]))
            .collect(),
        Action::ZoomOut => vec![([5., 12.], [19., 12.])],
        Action::ZoomIn => vec![([5., 12.], [19., 12.]), ([12., 5.], [12., 19.])],
        Action::Fit => vec![
            ([4., 9.], [4., 4.]),
            ([4., 4.], [9., 4.]),
            ([15., 4.], [20., 4.]),
            ([20., 4.], [20., 9.]),
            ([20., 15.], [20., 20.]),
            ([20., 20.], [15., 20.]),
            ([9., 20.], [4., 20.]),
            ([4., 20.], [4., 15.]),
            ([9., 9.], [15., 9.]),
            ([15., 9.], [15., 15.]),
            ([15., 15.], [9., 15.]),
            ([9., 15.], [9., 9.]),
        ],
    }
}
fn icon_image(action: Action) -> Image {
    let segments = icon_segments(action);
    let size = 48u32;
    let mut pixels = vec![0; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let point = Vec2::new((x as f32 + 0.5) / 2., (y as f32 + 0.5) / 2.);
            let i = ((y * size + x) * 4) as usize;
            if action == Action::Tool(Tool::Plate) {
                pixels[i..i + 4].copy_from_slice(&plate_pixel(point));
                continue;
            }
            let d = segments
                .iter()
                .map(|(a, b)| {
                    let a = Vec2::from_array(*a);
                    let ab = Vec2::from_array(*b) - a;
                    point.distance(
                        a + ab * ((point - a).dot(ab) / ab.length_squared()).clamp(0., 1.),
                    )
                })
                .fold(f32::INFINITY, f32::min);
            if action == Action::Tool(Tool::Select) {
                let inside = segments.iter().fold(false, |inside, (a, b)| {
                    let crosses = (a[1] > point.y) != (b[1] > point.y)
                        && point.x < (b[0] - a[0]) * (point.y - a[1]) / (b[1] - a[1]) + a[0];
                    inside ^ crosses
                });
                let pixel = if inside {
                    [24, 30, 39, 255]
                } else {
                    [255, 255, 255, ((1. - d).clamp(0., 1.) * 255.) as u8]
                };
                pixels[i..i + 4].copy_from_slice(&pixel);
                continue;
            }
            let color = match action {
                Action::Tool(Tool::Plate) => [10, 51, 122],
                Action::Tool(Tool::Edge) => [55, 113, 169],
                Action::Tool(Tool::Select) => [24, 30, 39],
                Action::ZoomIn | Action::ZoomOut | Action::Fit => [43, 99, 163],
                Action::Undo | Action::Redo => [112, 84, 145],
                _ => [50, 65, 85],
            };
            pixels[i..i + 4].copy_from_slice(&[
                color[0],
                color[1],
                color[2],
                ((1.25 - d).clamp(0., 1.) * 255.) as u8,
            ]);
        }
    }
    Image::new(
        bevy::render::render_resource::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        pixels,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    )
}

pub fn on_edge_click(
    mut event: On<Pointer<Click>>,
    editor: Res<Editor>,
    keys: Res<ButtonInput<KeyCode>>,
    links: Query<(), With<GraphLink>>,
    selected: Query<Entity, With<Selected>>,
    indicators: Query<Entity, With<SelectedIndicator>>,
    mut commands: Commands,
) {
    if !links.contains(event.event_target())
        || editor.tool != Tool::Select
        || keys.pressed(KeyCode::Space)
        || editor.dragged
    {
        return;
    }
    event.propagate(false);
    for e in &selected {
        commands.entity(e).remove::<Selected>();
    }
    for e in &indicators {
        commands.entity(e).despawn();
    }
    commands.entity(event.event_target()).insert(Selected);
    commands.trigger(crate::sidebar::ReloadSidebar);
    commands.trigger(crate::data_vis::CloseHistogramPanel);
}
fn highlight_edges(
    links: Query<(&MeshMaterial2d<ColorMaterial>, Has<Selected>), With<GraphLink>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (handle, selected) in &links {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            let color = if selected {
                Color::srgb(0.20, 0.36, 0.83)
            } else {
                ARROW_COLOR
            };
            if material.color != color {
                material.color = color;
            }
        }
    }
}
fn update_edge_preview(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    links: Query<(Entity, &GraphLink), With<UnfinishedLink>>,
    nodes: Query<&Transform, With<GraphNode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut last: Local<Option<Vec2>>,
) {
    if links.is_empty() {
        *last = None;
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if *last == Some(cursor) {
        return;
    }
    *last = Some(cursor);
    let (camera, t) = *camera;
    let Ok(target) = camera.viewport_to_world_2d(t, cursor) else {
        return;
    };
    for (e, link) in &links {
        let Ok(start) = nodes.get(link.from) else {
            continue;
        };
        let (translation, rotation, length) = link_geometry(
            start.translation,
            target.extend(1.),
            EndpointShape::circle(20.),
            EndpointShape::circle(0.),
        );
        commands.entity(e).insert((
            Mesh2d(meshes.add(arrow_mesh(length))),
            MeshMaterial2d(materials.add(Color::srgb(0.4, 0.5, 0.7))),
            Transform {
                translation,
                rotation,
                ..default()
            },
            Pickable::IGNORE,
        ));
    }
}

fn follow_canvas(
    window: Single<&Window>,
    camera: Single<(&Transform, &Projection), (With<Camera2d>, Without<Canvas>)>,
    mut canvas: Single<&mut Transform, With<Canvas>>,
) {
    let (t, p) = *camera;
    canvas.translation = Vec3::new(t.translation.x, t.translation.y, -100.);
    let scale = camera_scale(p);
    canvas.scale = Vec3::new(
        window.width() * scale * 3.,
        window.height() * scale * 3.,
        1.,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inference_hides_toolbar_and_pencil_restores_editing_without_panels() {
        use bevy::picking::{backend::HitData, pointer::PointerButton};
        let mut app = test_app();
        app.add_observer(crate::data_vis::close_histogram_panel);
        let toolbar = app
            .world_mut()
            .query_filtered::<Entity, With<Toolbar>>()
            .single(app.world())
            .unwrap();
        let pencil = button(&mut app, Action::Edit);
        let selected = app
            .world_mut()
            .spawn((
                GraphNode(1),
                Selected,
                Transform::from_xyz(0., 0., 1.),
                ScalarNode {
                    val: 1.,
                    name: None,
                },
            ))
            .id();
        app.world_mut()
            .spawn((crate::sidebar::LocalSidebar, Node::default()));
        app.world_mut()
            .spawn((crate::data_vis::InferenceHistogramPanel, Node::default()));
        app.world_mut().trigger(InspectModel);
        app.update();
        assert_eq!(
            app.world().get::<Node>(toolbar).unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(pencil).unwrap().display,
            Display::Flex
        );
        pointer(
            &mut app,
            pencil,
            Vec2::new(SIDEBAR_WIDTH + 20., 20.),
            Click {
                button: PointerButton::Primary,
                hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
                duration: std::time::Duration::from_millis(50),
                count: 1,
            },
        );
        app.update();
        assert!(!app.world().resource::<EditorView>().inspecting);
        assert_eq!(
            app.world().get::<Node>(toolbar).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().get::<Node>(pencil).unwrap().display,
            Display::None
        );
        assert!(app.world().get::<ScalarNode>(selected).is_some());
        assert!(app.world().get::<Selected>(selected).is_none());
        let mut panels = app.world_mut().query_filtered::<Entity, Or<(
            With<crate::sidebar::LocalSidebar>,
            With<crate::data_vis::InferenceHistogramPanel>,
        )>>();
        assert_eq!(panels.iter(app.world()).count(), 0);
    }

    #[test]
    fn automatic_node_inspection_selects_node_and_opens_matching_sidebar() {
        #[derive(Resource, Default)]
        struct OpenedHistogram(Option<crate::data_vis::HistogramSubject>);
        let mut app = test_app();
        app.init_resource::<OpenedHistogram>()
            .add_observer(crate::sidebar::reload_sidebar)
            .add_observer(
                |event: On<crate::data_vis::OpenHistogramPanel>,
                 mut opened: ResMut<OpenedHistogram>| {
                    opened.0 = Some(event.subject);
                },
            );
        let node = app
            .world_mut()
            .spawn((
                GraphNode(7),
                Transform::from_xyz(0., 0., 1.),
                RandomNode {
                    name: Some("theta".into()),
                    dist_type: "Normal".into(),
                    params: vec![ParamValue("mean", None), ParamValue("std_dev", None)],
                },
            ))
            .id();
        app.world_mut()
            .trigger(crate::nodes::OpenNodeInspection { node_id: 7 });
        app.world_mut().flush();
        assert!(app.world().get::<Selected>(node).is_some());
        let mut sidebars = app
            .world_mut()
            .query_filtered::<Entity, With<crate::sidebar::LocalSidebar>>();
        assert_eq!(sidebars.iter(app.world()).count(), 1);
        assert_eq!(
            app.world().resource::<OpenedHistogram>().0,
            Some(crate::data_vis::HistogramSubject::Node(7))
        );
    }

    #[test]
    fn zoom_keeps_the_pointer_world_position_fixed_even_at_limits() {
        let mut projection = Projection::Orthographic(OrthographicProjection::default_2d());
        let mut t = Transform::from_xyz(120., -35., 0.);
        let viewport = Vec2::new(1200., 800.);
        let pointer = Vec2::new(930., 250.);
        let offset = Vec2::new(pointer.x - viewport.x / 2., viewport.y / 2. - pointer.y);
        let anchor = t.translation.truncate() + offset * camera_scale(&projection);
        for factor in [0.5, 1000., 0.00001, 1.2] {
            apply_zoom(&mut projection, &mut t, viewport, pointer, factor);
            let new_anchor = t.translation.truncate() + offset * camera_scale(&projection);
            assert!(new_anchor.distance(anchor) < 0.001);
            assert!((0.1..=8.).contains(&camera_scale(&projection)));
        }
    }
    #[test]
    fn editor_systems_initialize_and_run_without_query_conflicts() {
        let mut app = test_app();
        assert_eq!(app.world().resource::<Editor>().tool, Tool::Select);
        let mut toolbar = app.world_mut().query_filtered::<&Node, With<Toolbar>>();
        assert_eq!(toolbar.iter(app.world()).count(), 1);
    }
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            EditorPlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<ColorMaterial>>()
        .init_asset::<Image>()
        .init_resource::<ReducedView>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<InputFocus>()
        .add_message::<MouseWheel>();
        app.world_mut().spawn(Window::default());
        app.world_mut().spawn(Camera2d);
        app.world_mut()
            .spawn((Canvas, NodeMode(NodeType::Random), Transform::default()));
        app.add_observer(crate::sidebar::on_trigger_close_menus);
        app.update();
        app.update();
        app
    }
    fn pointer<E: std::fmt::Debug + Clone + bevy::reflect::Reflect>(
        app: &mut App,
        entity: Entity,
        position: Vec2,
        event: E,
    ) {
        use bevy::{
            camera::NormalizedRenderTarget,
            picking::pointer::{Location, PointerId},
            window::WindowRef,
        };
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<Window>>()
            .single(app.world())
            .unwrap();
        let location = Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Entity(window).normalize(None).unwrap(),
            ),
            position,
        };
        app.world_mut()
            .trigger(Pointer::new(PointerId::Mouse, location, event, entity));
        app.world_mut().flush();
    }
    fn button(app: &mut App, action: Action) -> Entity {
        let mut q = app.world_mut().query::<(Entity, &ToolButton)>();
        q.iter(app.world())
            .find(|(_, b)| b.action == action)
            .unwrap()
            .0
    }
    #[test]
    fn type_picker_dismisses_hint_and_updates_the_creation_type() {
        use bevy::picking::{backend::HitData, pointer::PointerButton};
        let mut app = test_app();
        let picker = button(&mut app, Action::NodeType);
        *app.world_mut().get_mut::<Interaction>(picker).unwrap() = Interaction::Hovered;
        app.update();
        let mut visibility = app
            .world_mut()
            .query_filtered::<&Visibility, With<ToolbarTip>>();
        assert_eq!(
            *visibility.single(app.world()).unwrap(),
            Visibility::Visible
        );
        let hit = HitData::new(Entity::PLACEHOLDER, 0., None, None);
        pointer(
            &mut app,
            picker,
            Vec2::new(600., 600.),
            Press {
                button: PointerButton::Primary,
                hit: hit.clone(),
                count: 1,
            },
        );
        app.update();
        assert_eq!(*visibility.single(app.world()).unwrap(), Visibility::Hidden);
        app.world_mut().trigger(crate::sidebar::OpenNodeTypeMenu {
            pos: Vec2::new(600., 400.),
        });
        app.world_mut().flush();
        app.update();
        let mut options = app.world_mut().query::<(Entity, &NodeTypeOption)>();
        assert_eq!(options.iter(app.world()).count(), 3);
        let constant = options
            .iter(app.world())
            .find(|(_, o)| o.0 == NodeType::Scalar)
            .unwrap()
            .0;
        pointer(
            &mut app,
            constant,
            Vec2::new(600., 450.),
            Click {
                button: PointerButton::Primary,
                hit,
                duration: std::time::Duration::from_millis(50),
                count: 1,
            },
        );
        let mut mode = app.world_mut().query::<&NodeMode>();
        assert_eq!(mode.single(app.world()).unwrap().0, NodeType::Scalar);
        let mut menus = app
            .world_mut()
            .query_filtered::<Entity, With<NodeTypeMenu>>();
        assert_eq!(menus.iter(app.world()).count(), 0);
    }
    #[test]
    fn dragging_creates_one_default_node_and_escape_cancels_the_next_drag() {
        use bevy::picking::{backend::HitData, pointer::PointerButton};
        let mut app = test_app();
        let mut modes = app.world_mut().query::<&mut NodeMode>();
        modes.single_mut(app.world_mut()).unwrap().0 = NodeType::Scalar;
        let adder = button(&mut app, Action::Tool(Tool::Node));
        let camera = app
            .world_mut()
            .query_filtered::<Entity, With<Camera2d>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .get_mut::<Camera>(camera)
            .unwrap()
            .computed
            .target_info = Some(bevy::camera::RenderTargetInfo {
            physical_size: UVec2::new(1280, 720),
            scale_factor: 1.,
        });
        app.world_mut()
            .get_mut::<Camera>(camera)
            .unwrap()
            .computed
            .clip_from_view = Mat4::IDENTITY;
        let start = || DragStart {
            button: PointerButton::Primary,
            hit: HitData::new(camera, 0., None, None),
        };
        let end = || DragEnd {
            button: PointerButton::Primary,
            distance: Vec2::new(200., -200.),
        };
        pointer(&mut app, adder, Vec2::new(600., 650.), start());
        assert_eq!(
            app.world().resource::<NodeDrag>().active,
            Some(NodeType::Scalar)
        );
        pointer(&mut app, adder, Vec2::new(800., 400.), end());
        let mut scalars = app.world_mut().query::<&ScalarNode>();
        assert_eq!(scalars.iter(app.world()).count(), 1);
        assert_eq!(scalars.single(app.world()).unwrap().val, 1.);
        pointer(&mut app, adder, Vec2::new(600., 650.), start());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert_eq!(app.world().resource::<NodeDrag>().active, None);
        pointer(&mut app, adder, Vec2::new(800., 400.), end());
        assert_eq!(scalars.iter(app.world()).count(), 1);
        assert!(!valid_drop(
            Vec2::new(800., 400.),
            Vec2::new(1280., 720.),
            true
        ));
        assert!(!valid_drop(
            Vec2::new(1300., 400.),
            Vec2::new(1280., 720.),
            false
        ));
    }
    #[test]
    fn outside_presses_close_the_type_picker_on_canvas_and_toolbar() {
        use bevy::picking::{backend::HitData, pointer::PointerButton};
        let mut app = test_app();
        let canvas = app
            .world_mut()
            .query_filtered::<Entity, With<Canvas>>()
            .single(app.world())
            .unwrap();
        let zoom = button(&mut app, Action::ZoomIn);
        let widget = app.world_mut().spawn(Node::default()).id();
        for target in [canvas, zoom, widget] {
            app.world_mut().trigger(crate::sidebar::OpenNodeTypeMenu {
                pos: Vec2::new(600., 400.),
            });
            app.world_mut().flush();
            let mut menus = app
                .world_mut()
                .query_filtered::<Entity, With<NodeTypeMenu>>();
            assert_eq!(menus.iter(app.world()).count(), 1);
            let option = app
                .world_mut()
                .query_filtered::<Entity, With<NodeTypeOption>>()
                .iter(app.world())
                .next()
                .unwrap();
            pointer(
                &mut app,
                option,
                Vec2::new(620., 420.),
                Press {
                    button: PointerButton::Primary,
                    hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
                    count: 1,
                },
            );
            assert_eq!(
                menus.iter(app.world()).count(),
                1,
                "pressing inside should keep the menu open"
            );
            pointer(
                &mut app,
                target,
                Vec2::new(900., 300.),
                Press {
                    button: PointerButton::Primary,
                    hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
                    count: 1,
                },
            );
            assert_eq!(
                menus.iter(app.world()).count(),
                0,
                "outside press should dismiss the menu"
            );
        }
    }

    #[test]
    fn toolbar_docks_above_panels_and_dragging_stays_within_canvas() {
        let mut app = test_app();
        let palette = app
            .world_mut()
            .query_filtered::<Entity, With<Toolbar>>()
            .single(app.world())
            .unwrap();
        let window = app
            .world_mut()
            .query_filtered::<&Window, ()>()
            .single(app.world())
            .unwrap();
        let size = Vec2::new(window.width(), window.height());
        assert!(matches!(app.world().get::<Node>(palette).unwrap().top,Val::Px(y) if y>12.));
        app.world_mut().resource_mut::<PalettePosition>().offset = Vec2::splat(50.);
        let sidebar = app
            .world_mut()
            .spawn((
                crate::sidebar::LocalSidebar,
                Node {
                    width: px(SIDEBAR_WIDTH),
                    ..default()
                },
            ))
            .id();
        app.update();
        assert_eq!(app.world().get::<Node>(palette).unwrap().top, px(12));
        let histogram = app
            .world_mut()
            .spawn((
                crate::data_vis::InferenceHistogramPanel,
                Node {
                    height: px(crate::data_vis::HISTOGRAM_PANEL_HEIGHT),
                    ..default()
                },
            ))
            .id();
        app.update();
        app.world_mut().resource_mut::<PalettePosition>().offset = Vec2::splat(10000.);
        app.update();
        let node = app.world().get::<Node>(palette).unwrap();
        assert!(
            matches!(node.left,Val::Px(x) if x>=SIDEBAR_WIDTH+12. && x<=size.x-SIDEBAR_WIDTH-12.)
        );
        assert!(
            matches!(node.top,Val::Px(y) if y>=12. && y+58.<=size.y-crate::data_vis::HISTOGRAM_PANEL_HEIGHT-12.)
        );
        app.world_mut().despawn(sidebar);
        app.world_mut().despawn(histogram);
        app.update();
        assert!(matches!(app.world().get::<Node>(palette).unwrap().top,Val::Px(y) if y>12.));
    }
    #[test]
    fn shift_alone_neither_starts_links_nor_shows_the_link_cursor() {
        use bevy::{
            picking::{backend::HitData, pointer::PointerButton},
            window::CursorIcon,
        };
        let mut app = test_app();
        let source = app
            .world_mut()
            .spawn((
                GraphNode(1),
                ScalarNode {
                    val: 1.,
                    name: None,
                },
                Transform::from_xyz(0., 0., 1.),
            ))
            .observe(crate::nodes::on_node_click)
            .id();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        app.update();
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<Window>>()
            .single(app.world())
            .unwrap();
        assert!(matches!(
            app.world().get::<CursorIcon>(window),
            Some(CursorIcon::System(SystemCursorIcon::Default))
        ));
        let click = || Click {
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
            duration: std::time::Duration::from_millis(50),
            count: 1,
        };
        pointer(&mut app, source, Vec2::new(700., 300.), click());
        let mut links = app
            .world_mut()
            .query_filtered::<Entity, With<UnfinishedLink>>();
        assert_eq!(links.iter(app.world()).count(), 0);
        app.world_mut().resource_mut::<Editor>().tool = Tool::Edge;
        pointer(&mut app, source, Vec2::new(700., 300.), click());
        assert_eq!(links.iter(app.world()).count(), 1);
    }
    #[test]
    fn fit_reserves_identical_space_with_panels_closed_and_open() {
        use bevy::picking::{backend::HitData, pointer::PointerButton};
        let mut app = test_app();
        let fit = button(&mut app, Action::Fit);
        let camera = app
            .world_mut()
            .query_filtered::<Entity, With<Camera2d>>()
            .single(app.world())
            .unwrap();
        app.world_mut().spawn((
            GraphNode(1),
            ScalarNode {
                val: 1.,
                name: None,
            },
            Transform::from_xyz(100., 200., 1.),
        ));
        let click = || Click {
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
            duration: std::time::Duration::from_millis(50),
            count: 1,
        };
        pointer(&mut app, fit, Vec2::new(900., 650.), click());
        let closed = (
            *app.world().get::<Transform>(camera).unwrap(),
            camera_scale(app.world().get::<Projection>(camera).unwrap()),
        );
        app.world_mut().spawn((
            crate::sidebar::LocalSidebar,
            Node {
                width: px(SIDEBAR_WIDTH),
                ..default()
            },
        ));
        app.world_mut().spawn((
            crate::data_vis::InferenceHistogramPanel,
            Node {
                height: px(crate::data_vis::HISTOGRAM_PANEL_HEIGHT),
                ..default()
            },
        ));
        pointer(&mut app, fit, Vec2::new(900., 650.), click());
        let open = (
            *app.world().get::<Transform>(camera).unwrap(),
            camera_scale(app.world().get::<Projection>(camera).unwrap()),
        );
        assert_eq!(closed, open);
    }
    #[test]
    fn fit_centers_and_contains_model_in_the_reserved_canvas() {
        let mut projection = OrthographicProjection::default_2d();
        let mut transform = Transform::from_xyz(0., 0., 7.);
        let window = Vec2::new(1280., 800.);
        let min = Vec2::new(-200., -100.);
        let max = Vec2::new(400., 200.);
        fit_model(&mut projection, &mut transform, min, max, window);
        let screen = |point: Vec2| {
            let relative = (point - transform.translation.truncate()) / projection.scale;
            window / 2. + Vec2::new(relative.x, -relative.y)
        };
        assert!(screen((min + max) / 2.).distance(Vec2::new(640., 270.)) < 0.001);
        for point in [min, max] {
            let p = screen(point);
            assert!(p.x >= 340. - 0.001 && p.x <= 940. + 0.001);
            assert!(p.y >= 40. - 0.001 && p.y <= 500. + 0.001);
        }
        assert_eq!(transform.translation.z, 7.);
    }
}

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
    if kind == NodeType::Scalar {
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

fn update_node_adder(
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

fn open_node_type_menu(
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
            (window.height() - 180.).max(0.),
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
fn style_node_options(
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
fn start_node_drag(
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
fn move_node_drag(mut event: On<Pointer<Drag>>, mut ghosts: Query<&mut Node, With<NodeDragGhost>>) {
    event.propagate(false);
    for mut node in &mut ghosts {
        node.left = px(event.pointer_location.position.x - 35.);
        node.top = px(event.pointer_location.position.y - 26.);
    }
}
fn finish_node_drag(
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
    }
}
fn valid_drop(pointer: Vec2, size: Vec2, over_ui: bool) -> bool {
    !over_ui && pointer.cmpge(Vec2::ZERO).all() && pointer.cmplt(size).all()
}

// Signed box distance keeps the reference icon's square corners sharp at any scale.
fn box_distance(point: Vec2, center: Vec2, half_size: Vec2) -> f32 {
    let d = (point - center).abs() - half_size;
    d.max(Vec2::ZERO).length() + d.max_element().min(0.)
}
fn plate_pixel(point: Vec2) -> [u8; 4] {
    let outer = box_distance(point, Vec2::splat(12.), Vec2::splat(11.));
    let inner = box_distance(point, Vec2::splat(12.), Vec2::splat(9.));
    let border = outer.max(-inner);
    let badge = box_distance(point, Vec2::splat(17.5), Vec2::splat(5.5));
    let coverage = (0.5 - border.min(badge)).clamp(0., 1.);
    let glyph = [
        (Vec2::new(15., 20.), Vec2::new(17., 14.)),
        (Vec2::new(17., 14.), Vec2::new(19., 20.)),
        (Vec2::new(16., 18.), Vec2::new(18., 18.)),
    ];
    let distance = glyph
        .iter()
        .map(|(a, b)| {
            let ab = *b - *a;
            point.distance(*a + ab * ((point - *a).dot(ab) / ab.length_squared()).clamp(0., 1.))
        })
        .fold(f32::INFINITY, f32::min);
    let white = ((0.9 - distance) * 2.).clamp(0., 1.);
    let blue = [13., 46., 112.];
    [
        (blue[0] + (255. - blue[0]) * white) as u8,
        (blue[1] + (255. - blue[1]) * white) as u8,
        (blue[2] + (255. - blue[2]) * white) as u8,
        (coverage * 255.) as u8,
    ]
}

/// Reserve both sidebars and the histogram before they open, so fitting remains stable.
fn fit_model(
    projection: &mut OrthographicProjection,
    transform: &mut Transform,
    min: Vec2,
    max: Vec2,
    window: Vec2,
) {
    let padding = 40.;
    let available_min = Vec2::new(SIDEBAR_WIDTH + padding, padding);
    let available_max = Vec2::new(
        window.x - SIDEBAR_WIDTH - padding,
        window.y - crate::data_vis::HISTOGRAM_PANEL_HEIGHT - padding,
    );
    let size = (available_max - available_min).max(Vec2::splat(40.));
    projection.scale = ((max.x - min.x) / size.x)
        .max((max.y - min.y) / size.y)
        .clamp(0.1, 8.);
    let screen_center = available_min + size / 2.;
    let offset = Vec2::new(
        screen_center.x - window.x / 2.,
        window.y / 2. - screen_center.y,
    );
    let center = (min + max) / 2. - offset * projection.scale;
    transform.translation.x = center.x;
    transform.translation.y = center.y;
}

fn close_node_menu_on_outside_press(
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
