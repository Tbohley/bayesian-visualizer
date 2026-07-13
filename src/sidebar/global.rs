use bevy::prelude::*;
use super::*;

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
            Node {
                margin: px(16).bottom(),
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
            ..default()
        },
        BorderColor::all(Color::BLACK),
        BackgroundColor(Color::BLACK),
        children![(
            Pickable::IGNORE,
            Text::new("Node type"),
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

    commands.entity(global_sidebar_entity).add_child(nodemode_menu);
    //TODO: context menu for selecting which type of node to create
}

pub fn on_set_node_mode(
    event: On<Pointer<Press>>,
    menu_items: Query<&ContextMenuItem>,
    mut commands: Commands,
    node_mode: Single<&mut NodeMode>
){
    let target = event.original_event_target();

    if let Ok(item) = menu_items.get(target) {
        //set distribution of node to new dist... or maybe on apply?
        println!("Selected node creation type: {}", item.0);
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
    debug!("open context menu at: {pos}");

    commands
        .spawn((
            Name::new("node type selector"),
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
                context_item("Random"),
                context_item("Compute"),
                context_item("Scalar"),
            ],
        ))
        .observe(on_set_node_mode);
}