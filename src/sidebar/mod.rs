pub mod random_node;
pub mod global;
pub mod compute_node;
pub mod scalar_node;
use bevy::color::palettes::css::DARK_GREY;
use bevy::color::palettes::tailwind::SLATE_300;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::text::TextCursorStyle;
use bevy::prelude::*;
use crate::constants::*;
use crate::graph::*;
use crate::nodes::*;

#[derive(Component)]
pub struct LocalSidebar;

#[derive(Component)]
pub struct GlobalSidebar;

#[derive(Component)]
pub struct ParamTextbox(pub usize);

/// event opening a new context menu at position `pos`
#[derive(Event)]
pub struct OpenDistributionMenu {
    pub pos: Vec2,
}

#[derive(Event)]
pub struct OpenNodeTypeMenu{
    pub pos: Vec2,
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

//trait for var types to use to build their specific sidebar content
trait SidebarContent {
    fn build(
        &self, 
        commands: &mut Commands, 
        sidebar_entity: Entity,
        node_data: &Query<(Option<&RandomNode>, Option<&ScalarNode>, Option<&ComputeNode>)>,
        finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
        node: Entity
    );
}


pub fn context_item(text: &str) -> impl Bundle {
    (
        Name::new(format!("item-{text}")),
        ContextMenuItem(text.to_string()),
        Button,
        Node {
            padding: UiRect::all(px(5)),
            ..default()
        },
        children![(
            Pickable::IGNORE,
            Text::new(text),
            TextColor(Color::WHITE),
        )],
    )
}

//generate menu of incoming links for any node
pub fn available_links(
    commands: &mut Commands,
    node_data: &Query<(Option<&RandomNode>, Option<&ScalarNode>, Option<&ComputeNode>)>,
    finished_links: &Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    sidebar_entity: Entity,
    node: Entity
){
    commands.entity(sidebar_entity).with_child((
        Text::new("Incoming links:"),
        Node {
            margin: px(4).bottom(),
            ..default()
        },
        TextColor(NODE_NAME_COLOR),
    ));
    let link_space = commands.spawn(
        (Node{
            margin: px(4).bottom(),
            flex_direction: FlexDirection::Column,
            border_radius: px(5.).into(),
            ..default()
        },
        BackgroundColor(AVAILABLE_LINKS_COLOR))
    ).id();
    commands.entity(sidebar_entity).add_child(link_space);

    let mut i = 1;
    for (_entity, ends) in finished_links.iter(){
        if ends.to == Some(node) {
            let (maybe_random, maybe_scalar, maybe_transform) = node_data.get(ends.from).unwrap();

            let label = match (maybe_random, maybe_scalar, maybe_transform) {
                (Some(rv), None, None) => rv.label(),
                (None, Some(sc), None) => "scalar: ".to_string() + &sc.label(),
                (None, None, Some(cn)) => "operation: ".to_string() + &cn.label(),
                _ => "sidebar/mod.rs BUG: ".to_string(),
            };

            commands.entity(link_space).with_child((
                Text::new(format!["{}: {}", i, label]),
                Node {
                    margin: px(4).bottom(),
                    ..default()
                },
                TextFont{
                    font_size: px(16).into(),
                    ..default()
                },
                TextColor(NODE_NAME_COLOR),
            ));
            i+=1;
        }
    }
    if i == 1 {
        commands.entity(link_space).with_child((
            Text::new("No incoming links!"),
            Node {
                margin: px(4).bottom(),
                ..default()
            },
            TextFont{
                font_size: px(16).into(),
                ..default()
            },
            TextColor(NODE_NAME_COLOR),
        ));
    }
}

//sidebar loader, event triggered by most graph changes
pub fn reload_sidebar(
    _event: On<ReloadSidebar>,
    mut commands: Commands,
    selected: Option<Single<(Entity, &mut Selected, &mut GraphNode)>>,
    node_data: Query<(Option<&RandomNode>, Option<&ScalarNode>, Option<&ComputeNode>)>,
    finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    sidebar: Query<(Entity, &LocalSidebar)>,
){
    for (sidebar_entity, _comp) in sidebar.iter(){
        commands.entity(sidebar_entity).despawn();
    }
    if let Some(single) = selected{
        let (entity, _selected_comp, node) = single.into_inner();

        let sidebar_entity = commands.spawn((
            LocalSidebar,
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
                    margin: px(10).bottom(),
                    ..default()
                },
                TextColor(NODE_NAME_COLOR),
            ));
        let (maybe_random, maybe_scalar, maybe_transform) = node_data.get(entity).unwrap();
        match (maybe_random, maybe_scalar, maybe_transform) {
            (Some(rv), None, None) => rv.build(&mut commands, sidebar_entity, &node_data, finished_links, entity),
            (None, Some(sc), None) => sc.build(&mut commands, sidebar_entity, &node_data, finished_links, entity),
            (None, None, Some(cn)) => cn.build(&mut commands, sidebar_entity, &node_data, finished_links, entity),
            _ => warn!("Node has invalid or multiple node type components"),
        }

        let delete_button = commands.spawn((
            Name::new("delete_button"),
            Button,
            Node {
                width: px(SIDEBAR_WIDTH * 0.75),
                height: px(30),
                border: UiRect::all(px(5)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(ERR_BORDER_COLOR),
            BackgroundColor(ERR_COLOR),
            children![(
                Pickable::IGNORE,
                Text::new("Delete"),
                TextColor(Color::WHITE),
                TextShadow::default(),
            )],
            )).observe(  //delete button functionality
                |
                _event: On<Pointer<Click>>,
                selected: Single<(Entity, &mut Selected, &mut GraphNode)>,
                mut commands: Commands,
                mut finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
                mut unfinished_link: Query<(Entity, &mut GraphLink), With<UnfinishedLink>>,
                |{
                    let (node, _selected, _graphnode) = selected.into_inner();
                        commands.entity(node).despawn();
                        
                        //despawn connected links
                        for (link_entity, link_component) in finished_links.iter_mut() {
                            if node == link_component.from || link_component.to == Some(node) {
                                commands.entity(link_entity).despawn();
                            }
                        }
                        //despawn unfinished connected link
                        if let Ok((unfinished_ent, ends)) = unfinished_link.single_mut() {
                            if node == ends.from {
                                commands.entity(unfinished_ent).despawn();
                            }
                        }
                        commands.trigger(ReloadSidebar);
                }
            ).id();
            commands.entity(sidebar_entity).add_child(delete_button);
    }

}


//close all context menus
pub fn on_trigger_close_menus(
    _event: On<CloseContextMenus>,
    mut commands: Commands,
    menus: Query<Entity, With<ContextMenu>>,
) {
    for e in menus.iter() {
        commands.entity(e).despawn();
    }
}