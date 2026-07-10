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
    random_vars: Query<&mut RandomVar>,
    //finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    sidebar: Query<(Entity, &Sidebar)>,
    names: Query<(&NamedNode, &ChildOf)>
){
    for (sidebar_entity, _comp) in sidebar.iter(){
        commands.entity(sidebar_entity).despawn();
    }
    if let Some(single) = selected{
        let (entity, _selected_comp, node) = single.into_inner();
        let mut name = None;
        let random_var = random_vars.get(entity).unwrap();
        for (label, parent) in names.iter(){
            if parent.parent() == entity{
                name = Some(label.0.clone());
            }
        }
        let sidebar_entity = 
        commands.spawn((
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
            BackgroundColor(DARK_GREY.into()),
            children![
                (
                    Text::new(format!("Node ID: {}", node.0)),
                    Node {
                        margin: px(16).bottom(),
                        ..default()
                    },
                    TextColor(NODE_NAME_COLOR),
                ),
                (
                    Text::new(if let Some(label) = name {
                        format!("Label: {}", label)
                    } else {
                        "Type to name".to_string()
                    }),
                    Node {
                        margin: px(8).bottom(),
                        ..default()
                    },
                    TextColor(NODE_NAME_COLOR),
                ),
                (
                    Node {
                        width: px(SIDEBAR_WIDTH - 32.0),
                        height: px(5.0),
                        margin: px(12).bottom(),
                        ..default()
                    },
                    BackgroundColor(NODE_NAME_COLOR),
                ),
                (
                    Text::new("Distribution:"),
                    Node {
                        margin: px(4).bottom(),
                        ..default()
                    },
                    TextColor(NODE_NAME_COLOR),
                )
            ],
        )).observe(
            //sidebar observes clicks to close distribution context menu
            |_: On<Pointer<Press>>, mut commands: Commands| {
                commands.trigger(CloseContextMenus);
        }).id();

        //spawn context menu
        let context_menu = commands.spawn((
            Name::new("distribution_context_menu"),
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
            BorderColor::all(Color::BLACK),
            BackgroundColor(Color::BLACK),
            children![(
                Pickable::IGNORE,
                Text::new(random_var.dist_type.clone()),
                TextColor(Color::WHITE),
                TextShadow::default(),
            )],
        )).observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
            // by default this event would bubble up further leading to the `CloseContextMenus`
            // event being triggered and undoing the opening of one here right away.
            event.propagate(false);
            println!("Clicked context menu");
            debug!("click: {}", event.pointer_location.position);

            commands.trigger(OpenContextMenu {
                pos: event.pointer_location.position,
            });
        }).id();

        commands.entity(sidebar_entity).add_child(context_menu);

        for (i, _param) in random_var.params.iter().enumerate() {        
            let child = build_param_textbox(
                &mut commands,
                random_var,
                i,
            );
            commands.entity(sidebar_entity).add_child(child);
        }
        let sample_button = commands.spawn((
        Name::new("sample_button"),
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
        BorderColor::all(Color::BLACK),
        BackgroundColor(Color::BLACK),
        children![(
            Pickable::IGNORE,
            Text::new("Sample"),
            TextColor(Color::WHITE),
            TextShadow::default(),
        )],
    )).observe(sample_node_toast).id();
    commands.entity(sidebar_entity).add_child(sample_button);
    }
}

fn build_param_textbox(
    commands: &mut Commands,
    random_var: &RandomVar,
    param_num: usize
) -> Entity {
    let param = random_var
        .params
        .get(param_num)
        .expect("invalid param_num");

    let param_name = param.0;
    let param_value = param.1;

    commands
        .spawn((
            Node {
                width: percent(100.),
                flex_direction: FlexDirection::Column,
                row_gap: px(4.),
                margin: px(8.).bottom(),
                ..default()
            },
            Name::new(format!("param_row_{}", param_name)),
            children![
                (
                    Text::new(param_name),
                    TextColor(NODE_NAME_COLOR),
                ),
                (
                    Node {
                        width: px(120.),
                        min_height: px(25.),
                        border: px(2).all(),
                        padding: px(4).all(),
                        ..default()
                    },
                    BorderColor::from(Color::from(SLATE_300)),
                    BackgroundColor(DARK_GREY.into()),
                    EditableText::new(param_value.to_string()),
                    TextLayout::no_wrap(),
                    TextCursorStyle::default(),
                    TabIndex(param_num.try_into().unwrap()),
                    Name::new(format!("{}_textbox", param_name)),
                    ParamTextbox(param_num)
                ),
            ],
        ))
        .id()
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

pub fn on_open_context_menu(
    event: On<OpenContextMenu>, 
    mut commands: Commands,
) {
    commands.trigger(CloseContextMenus);

    let pos = event.pos;

    debug!("open context menu at: {pos}");

    commands
        .spawn((
            Name::new("distribution selector"),
            ContextMenu,
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
                context_item("Normal", String::from("Normal")),
                context_item("LogNormal", String::from("LogNormal")),
                context_item("Beta", String::from("Beta")),
                context_item("Gamma", String::from("Gamma")),
                context_item("Exponential", String::from("Exponential")),
                context_item("Uniform", String::from("Uniform"))
            ],
        ))
        .observe(
            |event: On<Pointer<Press>>,
             menu_items: Query<&ContextMenuItem>,
             mut commands: Commands,
             selected: Option<Single<(Entity, &Selected)>>,
             mut random_vars: Query<(&mut RandomVar, &Selected)>
             | {
                let target = event.original_event_target();
                if let Some(single) = selected{
                    let (entity, _selected_comp) = single.into_inner();

                    if let Ok(item) = menu_items.get(target) {
                        //set distribution of node to new dist... or maybe on apply?
                        println!("Selected distribution {}", item.0);
                        let (mut random_var, _selected_comp) = random_vars.get_mut(entity).unwrap();
                        random_var.dist_type = item.0.clone();
                        //buggy line, can panic, need to set to default params on dist change.
                        refresh_var_dist(&mut random_var, &mut commands);
                        commands.trigger(CloseContextMenus);
                        commands.trigger(ReloadSidebar);
                        
                    }
                }
            },
        );
}

fn context_item(text: &str, dist: String) -> impl Bundle {
    (
        Name::new(format!("item-{text}")),
        ContextMenuItem(dist),
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