pub mod random_menu;
pub mod global;
pub mod compute_menu;
pub mod scalar_menu;
pub mod link_params;
pub mod plate_menu;
use std::collections::HashMap;
use bevy::color::palettes::css::BLACK;
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
pub struct ScalarValueTextbox;

#[derive(Component)]
pub struct PlateNTextbox;

#[derive(Component)]
pub struct RequiresCompilation;

#[derive(Component)]
pub struct RequiresInference;

#[derive(Component)]
pub struct InferenceTextbox {
    pub tab_index: i32,
}

#[derive(Component)]
pub struct RandomSeedTextbox;

#[derive(Component)]
pub struct NumberOfSamplesTextbox;

#[derive(Component)]
pub struct NumberOfWarmupTextbox;

#[derive(Component)]
pub struct RandomSeedPlaceholder;

#[derive(Event)]
pub struct SetInferenceControlsEnabled(pub bool);

#[derive(Event)]
pub struct SetPosteriorSampleEnabled(pub bool);

/// event opening a new context menu at position `pos`
#[derive(Event)]
pub struct OpenDistributionMenu {
    pub pos: Vec2,
}

#[derive(Event)]
pub struct OpenNodeTypeMenu{
    pub pos: Vec2,
}

#[derive(Resource)]
pub struct Datasets {
    pub datasets: Vec<Dataset>,
}

#[derive(Event)]
pub struct OpenDatasetMenu{
    pub pos: Vec2,
}

#[derive(Event)]
pub struct OpenPlateMappingMenu {
    pub pos: Vec2,
    pub node: Entity,
}

#[derive(Event)]
pub struct OpenOperationMenu{
    pub pos: Vec2,
}

#[derive(Event)]
pub struct OpenParamLinkMenu{
    pub pos: Vec2,
    pub param_num: usize,
}


/// event will be sent to close currently open context menus
#[derive(Event)]
pub struct CloseContextMenus;

#[derive(Event)]
pub struct ReloadSidebar;

/// marker component identifying root of a context menu
#[derive(Component)]
pub struct ContextMenu;


#[derive(Component)]
pub struct ParamMenuItem {
    pub label: String,
    pub entity: Entity,
    pub param_num: usize,
}

// context menu item data storing what background color `Srgba` it activates
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
        node: Entity,
        observed: bool,
        observed_columns: &HashMap<Entity, String>,
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

pub fn divider() -> (bevy::prelude::Node, bevy::prelude::BackgroundColor, bevy::prelude::TextColor) {
    (Node {
        width: px(SIDEBAR_WIDTH - 32.0),
        height: px(5.0),
        margin: px(12).bottom(),
        ..default()
    },
    BackgroundColor(NODE_NAME_COLOR),
    TextColor(bevy::prelude::Color::Srgba(BLACK)))
}

//generate menu of incoming links for any node
pub fn available_links(
    commands: &mut Commands,
    node_data: &Query<(Option<&RandomNode>, Option<&ScalarNode>, Option<&ComputeNode>)>,
    finished_links: &Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    sidebar_entity: Entity,
    node: Entity,
    observed_columns: &HashMap<Entity, String>,
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
                (None, Some(sc), None) => format!(
                    "scalar: {}",
                    observed_columns
                        .get(&ends.from)
                        .cloned()
                        .unwrap_or_else(|| sc.label())
                ),
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
    selected: Option<Single<(Entity, &Selected, &GraphNode)>>,
    node_data: Query<(Option<&RandomNode>, Option<&ScalarNode>, Option<&ComputeNode>)>,
    plate_nodes: Query<
        (Entity, &GraphNode, &Transform, Option<&RandomNode>, Option<&ScalarNode>),
        Or<(With<RandomNode>, With<ScalarNode>)>,
    >,
    finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    sidebar: Query<(Entity, &LocalSidebar)>,
    mut plates: Query<&mut Plate>,
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
                Text::new(if plates.contains(entity) {
                    format!("Plate ID: {}", node.0)
                } else {
                    format!("Node ID: {}", node.0)
                }),
                Node {
                    margin: px(10).bottom(),
                    ..default()
                },
                TextColor(NODE_NAME_COLOR),
            ));
        let (maybe_random, maybe_scalar, maybe_transform) = node_data.get(entity).unwrap();
        let observed_columns = plates
            .iter()
            .flat_map(|plate| plate.mapping.iter())
            .filter(|(_, column)| column.as_str() != "unobserved")
            .map(|(entity, column)| (*entity, column.clone()))
            .collect::<HashMap<_, _>>();
        let is_observed = plates.iter().any(|plate| {
            plate
                .mapping
                .get(&entity)
                .is_some_and(|column| column != "unobserved")
        });
        match (maybe_random, maybe_scalar, maybe_transform) {
            (Some(rv), None, None) => rv.build(&mut commands, sidebar_entity, &node_data, finished_links, entity, is_observed, &observed_columns),
            (None, Some(sc), None) => sc.build(&mut commands, sidebar_entity, &node_data, finished_links, entity, is_observed, &observed_columns),
            (None, None, Some(cn)) => cn.build(&mut commands, sidebar_entity, &node_data, finished_links, entity, is_observed, &observed_columns),
            (None, None, None) if plates.contains(entity) => plates
                .get_mut(entity)
                .expect("selected plate should exist")
                .build(&mut commands, sidebar_entity, &plate_nodes),
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
