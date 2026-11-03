//! Editor state and plugin wiring; specialized systems live in submodules.
mod creation;
mod edges;
mod icons;
mod input;
mod navigation;
mod toolbar;

pub use edges::on_edge_click;
pub use navigation::{
    camera_scale, on_canvas_drag, on_canvas_drag_end, on_canvas_drag_start, over_ui,
};

use creation::*;
use edges::*;
use icons::*;
use input::*;
use navigation::*;
use toolbar::*;

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
            .add_systems(Startup, (spawn_toolbar, super::grid::spawn_grid))
            .add_systems(
                Update,
                (follow_canvas, super::grid::update_grid).after(wheel_navigation),
            )
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

#[cfg(test)]
#[path = "tests/editor.rs"]
mod tests;
