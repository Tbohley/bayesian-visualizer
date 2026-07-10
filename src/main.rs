//use core::f32;

use bevy::{
    color::palettes::{css::DARK_GREY, tailwind::SLATE_300}, 
    input::keyboard::KeyboardInput, 
    input_focus::{InputFocus, tab_navigation::{TabIndex}}, 
    prelude::*, 
    text::{EditableText, TextCursorStyle}
};
use fugue::*;
use fugue::error::FugueError::InvalidParameters;
use rand::{thread_rng};

mod helpers;
mod components_constants;
pub use helpers::*;
pub use components_constants::*;




pub fn throw_err(
    event: On<ErrorToast>,
    mut commands: Commands,
) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: px(24.),
            left: percent(50.),
            width: px(420.),
            min_height: px(40.),
            padding: px(12.).all(),
            border: px(2.).all(),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(event.color),
        BorderColor::from(Color::srgb(0.9, 0.15, 0.15)),
        ErrorToastBox {
            timer: Timer::from_seconds(10.0, TimerMode::Once),
        },
        Button,
        ZIndex(999),
        children![(
            Text::new(event.text.clone()),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(14.),
                ..default()
            },
        )],
    ));
}

fn tick_error_toasts(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut ErrorToastBox)>,
) {
    for (entity, mut toast) in &mut q {
        toast.timer.tick(time.delta());

        if toast.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn click_error_toasts(
    mut commands: Commands,
    q: Query<(Entity, &Interaction), (Changed<Interaction>, With<ErrorToastBox>)>,
) {
    for (entity, interaction) in &q {
        if *interaction == Interaction::Pressed {
            commands.entity(entity).despawn();
        }
    }
}

//update arrow transforms connecting to dragged node
fn on_node_drag (
    event: On<Pointer<Drag>>,
    mut transforms: Query<&mut Transform>,
    mut mesh_query: Query<&mut Mesh2d>,
    mut graph_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    mut meshes: ResMut<Assets<Mesh>>
) {
    println!("Dragged a node");
    {
        //update node position
        if let Ok(mut ent) = transforms.get_mut(event.event_target()) {
        ent.translation.x += event.delta.x;     
        ent.translation.y -= event.delta.y;
    }
}
    //update all connected arrow positions/meshes
    for (link_entity, link_component) in graph_links.iter_mut() { 
        if event.event_target() == link_component.from || event.event_target() == link_component.to.unwrap() {
            let (new_transform, new_mesh) = link_transform_helper(&link_component, &transforms, &mut meshes).unwrap();
            if let Ok(mut link_transform) = transforms.get_mut(link_entity) {
                if let Ok(mut link_mesh) = mesh_query.get_mut(link_entity) {
                    *link_transform = new_transform;
                    *link_mesh = new_mesh;
                }
            }
        }
    }
}

//rename selected node to single-letter name from keyboard
fn on_keypress(
    mut kbd: MessageReader<KeyboardInput>,
    mut commands: Commands,
    selected: Option<Single<(Entity, &Selected)>>,
    mut unnamed: Query<(Entity, &UnnamedNode, &ChildOf)>,
    mut named: Query<(Entity, &NamedNode, &ChildOf)>,
){
    let Some(single) = selected else {
        return;
    };
    let (entity, _selected_comp) = single.into_inner();

    //for all keyboard inputs while node is selected
    for event in kbd.read() {
        if !event.state.is_pressed() {
            continue;
        }
        let Some(text) = &event.text else {
            continue;
        };
        //only alphabetic, numbers reserved for unnamed nodes
        if text.len() != 1 || !text.chars().all(|c| c.is_alphabetic()) {
            continue;
        }
        //find UnnamedNode (child entity of node w/display text) and delete
        for (unnamed_entity, _unnamed_node, parent) in unnamed.iter_mut() {
            println!("Named a node");
            if parent.parent() == entity {
                commands.entity(unnamed_entity).despawn();
            }
        }
        //same for NamedNode
        for (named_entity, _named_node, parent) in named.iter_mut() {
            println!("Renamed a node");
            if parent.parent() == entity {
                commands.entity(named_entity).despawn();
            }
        }
        //spawn new NamedNode child with new name/text
        commands.entity(entity).with_child((
            NamedNode(text.to_string()),
            Text2d::new(text.to_string()),
            TextColor(NODE_NAME_COLOR),
            Pickable::IGNORE,
            Transform::from_xyz(0.0, 0.0, 2.0),
        ));
        //reload sidebar
        commands.trigger(ReloadSidebar);
    }

}

//multifunctional: single click to edit a node, shift click two nodes consecutively to create a link, double click to delete the node and its links.
fn on_node_click(
    event: On<Pointer<Click>>,
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    mut unfinished_link: Query<(Entity, &mut GraphLink), With<UnfinishedLink>>,
    mut finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    transforms: Query<&mut Transform>,
    selected: Option<Single<(Entity, &mut Selected)>>,
    distributions: Query<&RandomVar>
){
    //if it is a shift click:
    if input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]){

        //if there is an unfinished GraphLink, complete it.
        if let Ok((unfinished_ent, mut ends)) = unfinished_link.single_mut() {

            commands.entity(unfinished_ent).remove::<UnfinishedLink>();

            //if user tries to create a link from a node to itself
            if ends.from == event.event_target() { 
                commands.entity(unfinished_ent).despawn();
                return;
            }
            ends.to = Some(event.event_target());
            println!("Completed a GraphLink");

            //add arrow
            if let Some((arrow_transform, arrow_mesh)) = link_transform_helper(&ends, &transforms, &mut meshes) {
                commands.entity(unfinished_ent).insert((
                    arrow_mesh,
                    MeshMaterial2d(materials.add(ARROW_COLOR)),
                    arrow_transform,
                ));
            }
            
        //otherwise, create an invisible UnfinishedLink
        }else{ 
            commands.spawn((
                GraphLink{
                    from: event.event_target(),
                    to: None
                },
                UnfinishedLink
            ));
            println!("Created an UnfinishedLink");
        }
    //normal click, select the node
    }else{
        //println!("Node click event");
        if event.duration.as_millis() < 200 && event.count == 1 {
            println!("Selected a node");

            if let Some(single) = selected{
                let (entity, _selected_comp) = single.into_inner();
                //deselect currently selected node
                commands.entity(entity).remove::<Selected>();
            }
            //select this node
            commands.entity(event.event_target()).insert(
                Selected
            );
            commands.trigger(ReloadSidebar);

            let selected_dist_box = distributions.get(event.event_target());
            match selected_dist_box {
                Err(_e) => println!("Selected node has no associated distrbution"),
                Ok(dist) => {
                    let mut rng = thread_rng();
                    println!("Selected node distribution: {:?}", dist.dist);
                    println!("Sample from node: {}", dist.dist.sample(&mut rng))
                }
            };

        }
        //double click, delete node
        if event.duration.as_millis() < 200 && event.count > 1 { 
            println!("Deleted a node");
            commands.entity(event.entity).despawn();
            
            //despawn connected links
            for (link_entity, link_component) in finished_links.iter_mut() {
                if event.event_target() == link_component.from || event.event_target() == link_component.to.unwrap(){
                    commands.entity(link_entity).despawn();
                }
            }
            //despawn unfinished connected link
            if let Ok((unfinished_ent, ends)) = unfinished_link.single_mut() {
                if event.event_target() == ends.from {
                    commands.entity(unfinished_ent).despawn();
                }
            }
            commands.trigger(ReloadSidebar);

        }
    }
}

//create a node on canvas
fn on_background_click(
    mut event: On<Pointer<Click>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    current_nodes: Query<&GraphNode>,
    selected: Option<Single<(Entity, &mut Selected)>>,
) {

    if let Some(single) = selected{
        let (entity, _selected_comp) = single.into_inner();
        //deselect currently selected node + close context menus
        commands.entity(entity).remove::<Selected>();
        commands.trigger(CloseContextMenus);
        commands.trigger(ReloadSidebar);
        return;
    }

    let mut node_num = 1;

    //finds the lowest unused node in the least efficient way possible
    while current_nodes.iter().any(|node| node.0 == node_num) { 
        node_num += 1;
    }
    println!("Created node #{}", node_num);

    commands.spawn((
        GraphNode(node_num),
        Pickable{should_block_lower: true, is_hoverable: true},
        Mesh2d(meshes.add(Circle::new(NODE_RAD))),
        MeshMaterial2d(materials.add(NODE_COLOR)),
        Transform::from_xyz(
            event.hit.position.unwrap().x,
            event.hit.position.unwrap().y,
            1.0),
        RandomVar{
            dist_type: String::from("Normal"),
            dist: Box::new(Normal::new(0.0, 1.0).unwrap().clone()),
            params: vec![ParamValue("mean", 0.),ParamValue("std_dev",1.)]
        }
    )).with_child((
        UnnamedNode,
        Text2d::new(node_num.to_string()),
        TextColor(NODE_NAME_COLOR),
        Pickable::IGNORE,
        Transform::from_xyz(0.0,0.0,2.0)
    ))
    .observe(on_node_drag)
    .observe(on_node_click);
    event.propagate(true);
}

fn setup (
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    //spawn clickable background
    commands.spawn((
        Canvas,
        Mesh2d(meshes.add(Rectangle::new(CANVAS_WIDTH, CANVAS_HEIGHT))),
        MeshMaterial2d(materials.add(CANVAS_COLOR))
    ))
    .observe(on_background_click);

    commands.spawn((
        Text2d::new("Click to create a new node.\n\
                        Shift click a parent and then a child\n\
                        node to create a link between them.\n\
                        Double-click a node to delete it.\n\
                        Click a node, then a letter key,\n\
                        to assign it a one-letter name."),
        Transform{
            translation: vec3(-450.,300.0,1.0),
            scale: vec3(0.7,0.7,1.0),
            rotation: Quat::from_rotation_z(0.0)
        }
    ));
}

// Submit the new param when Enter is pressed
fn text_submission(
    input_focus: Res<InputFocus>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut text_input: Query<(&mut EditableText, &ParamTextbox, &Name)>,
    node_to_change: Single<(&mut RandomVar, &Selected)>,
    mut commands: Commands
) {
    if keyboard_input.just_pressed(KeyCode::Enter)
        && let Some(focused_entity) = input_focus.get()
        && let Ok((mut text_input, param_num, _name)) = text_input.get_mut(focused_entity)
    {
        let (mut random_var, _selected) = node_to_change.into_inner();
        let num = text_input.value().to_string().parse::<f64>();
        match num {
            Ok(f) => {
                println!("Node w/{} distribution: {} set to {}",random_var.dist_type, distribution_params().get(&random_var.dist_type).unwrap().get(param_num.0).unwrap().0, f);
                random_var.params.get_mut(param_num.0).expect("invalid param_num").1 = f;
            }
            Err(_e) => {
                println!("Not a valid parameter number!");
                text_input.clear();
            }
        }
        println!("Dist params: {:?}", random_var.params);
        reset_dist(&mut random_var, &mut commands);
        commands.trigger(ReloadSidebar);
        //random_var
        //set the random_var's dist value to a new distribution with updated param
    }
}

fn sample_node(
    _event: On<Pointer<Click>>,
    mut node: Single<&mut RandomVar, With<Selected>>,
    mut commands: Commands
) {
    let mut rng = thread_rng();
    reset_dist(&mut node, &mut commands);
    println!("Sample from node: {}", node.dist.sample(&mut rng));
    commands.trigger(ErrorToast{text: format!("Sample from node: {}", node.dist.sample(&mut rng)), color: SAMPLE_COLOR})
}

fn reset_dist(
    node: &mut RandomVar,
    commands: &mut Commands
) {
    let mut new_param_vals = Vec::<ParamValue>::new();

    //for all default parameters in the truth set for this distribution:
    for new_param_truth in distribution_params().get(&node.dist_type).unwrap() {
        let value = node
            .params
            .iter()
            .find(|old_param_val| old_param_val.0 == new_param_truth.0)
            .map(|old_param_val| old_param_val.1)
            .unwrap_or(new_param_truth.1);
    
        new_param_vals.push(ParamValue(new_param_truth.0, value));
    }
    
    node.params = new_param_vals;
    println!("New params: {:?}", &node.params);


    let p = |i: usize| {
        node.params
            .get(i).unwrap().1
    };

    let e = |err| {
        match err {
            InvalidParameters { distribution, reason, code, context: _ } => {
                commands.trigger(ErrorToast{ color: ERR_COLOR, text: format!("{} failed: {} (Code: {:?}). Please set new parameters and hit enter.", distribution, reason, code)});
            }
            other => {commands.trigger(ErrorToast{ color: ERR_COLOR, text: format!("distribution construction failed: {:?}. Please set new parameters and hit enter.", other)});}
        }
        None
    };

    let new_dist: Option<Box<dyn DistributionDebug<f64>>> = match node.dist_type.as_str() {
        "Normal" => Normal::new(p(0), p(1))
            .map(|d| Some(Box::new(d.clone()) as Box<dyn DistributionDebug<f64>>))
            .unwrap_or_else(e),
        "LogNormal" => LogNormal::new(p(0), p(1))
            .map(|d| Some(Box::new(d.clone()) as Box<dyn DistributionDebug<f64>>))
            .unwrap_or_else(e),
        "Exponential" => Exponential::new(p(0))
            .map(|d| Some(Box::new(d.clone()) as Box<dyn DistributionDebug<f64>>))
            .unwrap_or_else(e),
        "Gamma" => Gamma::new(p(0), p(1))
            .map(|d| Some(Box::new(d.clone()) as Box<dyn DistributionDebug<f64>>))
            .unwrap_or_else(e),
        "Beta" => Beta::new(p(0), p(1))
            .map(|d| Some(Box::new(d.clone()) as Box<dyn DistributionDebug<f64>>))
            .unwrap_or_else(e),
        "Uniform" => Uniform::new(p(0), p(1))
            .map(|d| Some(Box::new(d.clone()) as Box<dyn DistributionDebug<f64>>))
            .unwrap_or_else(e),
        other => {
            commands.trigger(ErrorToast {
                text: format!("unsupported distribution type: {}", other),
                color: ERR_COLOR
            });
            None
        }
    };
    
    if let Some(new_dist) = new_dist {
        node.dist = new_dist;
    }

    let mut rng = thread_rng();
    println!("Node distribution set to: {:?}", node.dist);
    println!("Sample from node: {}", node.dist.sample(&mut rng))
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

fn on_trigger_close_menus(
    _event: On<CloseContextMenus>,
    mut commands: Commands,
    menus: Query<Entity, With<ContextMenu>>,
) {
    for e in menus.iter() {
        commands.entity(e).despawn();
    }
}

fn on_trigger_menu(
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
                        reset_dist(&mut random_var, &mut commands);
                        commands.trigger(CloseContextMenus);
                        commands.trigger(ReloadSidebar);
                        
                    }
                }
            },
        );
}

fn node_settings(
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
    )).observe(sample_node).id();
    commands.entity(sidebar_entity).add_child(sample_button);
    }
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MeshPickingPlugin))
        .add_observer(on_trigger_menu)
        .add_observer(on_trigger_close_menus)
        .add_observer(throw_err)
        .add_observer(node_settings)
        .add_systems(Startup, setup)
        .add_systems(Update, (
            on_keypress, 
            text_submission,
            tick_error_toasts, 
            click_error_toasts))
        .run();
}



// PROGRESS
/*

------------------Next steps--------------------

Dragging nodes                              DONE
Shiftclick to create an arrow               DONE


-----------------Goals for 7/2------------------

Arrowhead (custom mesh?)                    DONE
Arrows on drag                              DONE
Arrows disappear on node deletion           DONE

-----------------Goals for 7/7------------------

Basic fugue scaffolding w/ normal dists     DONE
Simple sampling?
Plates, parameters


-----------------Goals for 7/10-----------------

Node sidebar{
    random vs parameter
    dist. params{
        change distribution button          
        apply changes button
    }
}
Plate dragging creation


-----------------Future goals-------------------

Global sidebar{
    drag n drop construction
    dummy node/param/plate?
    update button{
        plate logic and implementation      
    }
}


Single click allows node name editing,
eventually will be -> popup with 
distribution/property editing               DONE
Various distribution options
Single sampling/forward sampling
Plot viewing
Crosslink, brushing interaction
WASM support and CI/CD


-----------------Optional goals-----------------

Ghost arrow after shift-clicking a node
that tracks cursor until end node clicked   

Different color schemes

Rewrap all uses of .unwrap()



-------------------Bug tracker------------------

Deletion of a node in an UnfinishedLink     
leads to panic                             FIXED

Smashing keys on rename interacts with
a despawned entity (probably NamedNode)
and panics

Dragging a node, dropping it and then
clicking registers as a double click
and deletes it

*/