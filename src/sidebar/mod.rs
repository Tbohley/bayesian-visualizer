pub mod random_var;
use bevy::color::palettes::css::DARK_GREY;
use bevy::color::palettes::tailwind::SLATE_300;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::text::EditableText;
use bevy::text::TextCursorStyle;

use bevy::prelude::*;
use crate::constants::*;
use crate::graph::Selected;
use crate::nodes::*;

/*pub trait SidebarContent {
    fn build(&self, commands: &mut Commands, parent: Entity);
}
*/

#[derive(Component)]
pub struct Sidebar;

#[derive(Component)]
pub struct ParamTextbox(pub usize);

/// event opening a new context menu at position `pos`
#[derive(Event)]
pub struct OpenContextMenu {
    pub pos: Vec2,
}

//trait for var types to use to build their specific sidebar content
trait SidebarContent {
    fn build(&self, commands: &mut Commands, sidebar_entity: Entity);
}
impl SidebarContent for ComputeNode{
    fn build(&self, commands: &mut Commands, sidebar_entity: Entity){
        commands.entity(sidebar_entity).with_child(
            (
                Text::new("Unfinished"),
                Node {
                    margin: px(4).bottom(),
                    ..default()
                },
                TextColor(NODE_NAME_COLOR),
            )
        );
    }
}

impl SidebarContent for ScalarNode{
    fn build(&self, commands: &mut Commands, sidebar_entity: Entity){
        commands.entity(sidebar_entity).with_child(
            (
                Text::new("Unfinished"),
                Node {
                    margin: px(4).bottom(),
                    ..default()
                },
                TextColor(NODE_NAME_COLOR),
            )
        );
    }
}

/// event will be sent to close currently open context menus
#[derive(Event)]
pub struct CloseContextMenus;

#[derive(Event)]
pub struct ReloadSidebar;

/// marker component identifying root of a context menu
#[derive(Component)]
pub struct ContextMenu;

/// context menu item data storing what background color `Srgba` it activates
#[derive(Component)]
pub struct ContextMenuItem(pub String);

pub fn reload_sidebar(
    _event: On<ReloadSidebar>,
    mut commands: Commands,
    selected: Option<Single<(Entity, &mut Selected, &mut GraphNode)>>,
    node_data: Query<(Option<&RandomNode>, Option<&ScalarNode>, Option<&ComputeNode>)>,
    //finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    sidebar: Query<(Entity, &Sidebar)>,
){
    for (sidebar_entity, _comp) in sidebar.iter(){
        commands.entity(sidebar_entity).despawn();
    }
    if let Some(single) = selected{
        let (entity, _selected_comp, node) = single.into_inner();

        let sidebar_entity = commands.spawn((
            Sidebar,
            Node {
                position_type: PositionType::Absolute,
                right: px(0.),
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
        commands.entity(sidebar_entity).with_child(
            (
                Text::new(format!("Node ID: {}", node.0)),
                Node {
                    margin: px(16).bottom(),
                    ..default()
                },
                TextColor(NODE_NAME_COLOR),
            ));
        let (maybe_random, maybe_scalar, maybe_transform) = node_data.get(entity).unwrap();
        match (maybe_random, maybe_scalar, maybe_transform) {
            (Some(rv), None, None) => rv.build(&mut commands, sidebar_entity),
            (None, Some(sc), None) => sc.build(&mut commands, sidebar_entity),
            (None, None, Some(cn)) => cn.build(&mut commands, sidebar_entity),
            _ => warn!("Node has invalid or multiple node type components"),
        }
    }
}

pub fn on_trigger_close_menus(
    _event: On<CloseContextMenus>,
    mut commands: Commands,
    menus: Query<Entity, With<ContextMenu>>,
) {
    for e in menus.iter() {
        commands.entity(e).despawn();
    }
}
