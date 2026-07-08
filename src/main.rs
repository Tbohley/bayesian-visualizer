//use core::f32;

use bevy::{
    asset::RenderAssetUsages, 
    color::palettes::{css::DARK_GREY, tailwind::SLATE_300}, 
    input::keyboard::KeyboardInput, 
    input_focus::{AutoFocus, InputFocus, tab_navigation::{TabGroup, TabIndex}}, 
    platform::collections::HashMap, prelude::*, 
    render::mesh::{Indices, PrimitiveTopology}, 
    text::{EditableText, TextCursorStyle}
};
use fugue::*;
use rand::{rngs::StdRng, thread_rng};
use rand::SeedableRng;

mod arrows;
pub use arrows::*;


const CANVAS_HEIGHT: f32 = 500.0;
const CANVAS_WIDTH: f32 = 800.0;
const NODE_RAD: f32 = 20.0;
const CANVAS_COLOR: Color = Color::srgb(0.173, 0.227, 0.278);
const SIDEBAR_COLOR: Color = Color::srgb(0.827, 0.827, 0.827);
const NODE_NAME_COLOR: Color = Color::BLACK;
const NODE_COLOR: Color = Color::srgb(0.992, 0.447, 0.447);
const ARROW_COLOR: Color = Color::srgb(0.973, 0.937, 0.729);
const ARROW_THICKNESS: f32 = 2.0;
const ARROW_TIP_WIDTH_RATIO: f32 = 10.0;
const ARROW_TIP_LENGTH: f32 = 10.0;
const SIDEBAR_WIDTH: f32 = CANVAS_WIDTH / 4.;

//on all node entities
#[derive(Component)]
struct GraphNode(u32);

//on the text child entity of a named node
#[derive(Component)]
struct NamedNode(String);

//on the text child of a default node
#[derive(Component)]    
struct UnnamedNode;

#[derive(Component)]
struct Canvas;

//on links between nodes
#[derive(Component)]    
struct GraphLink{
    from: Entity,
    to: Option<Entity>
}

#[derive(Component)]
struct Sidebar;

trait DistributionDebug<T>: Distribution<T> + std::fmt::Debug {}
impl<T, D: Distribution<T> + std::fmt::Debug> DistributionDebug<T> for D {}

struct ParamValue {
    name: &'static str,
    value: f64,
}

//on random variable nodes
#[derive(Component)]
struct RandomVar{
    dist_type: String,
    dist: Box<dyn DistributionDebug<f64>>,
    params: Vec<ParamValue>
}

//on unfinished (invisible) arrows
#[derive(Component)]
struct UnfinishedLink;

//on currently selected node
#[derive(Component)]
struct Selected;

#[derive(Component)]
struct ParamTextbox(usize);


fn distribution_params() -> HashMap<String, Vec<&'static str>> {
    HashMap::from([
        (String::from("Normal"), vec!["mean", "std_dev"]),
        (String::from("LogNormal"), vec!["mean", "std_dev"]),
        (String::from("Gamma"), vec!["shape", "scale"]),
        (String::from("Beta"), vec!["alpha", "beta"]),
        (String::from("Exponential"), vec!["rate"]),
        (String::from("Uniform"), vec!["min", "max"])
    ])
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
            let selected_dist_box = distributions.get(event.event_target());
            match selected_dist_box {
                Err(e) => println!("Selected node has no associated distrbution"),
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
        //deselect currently selected node
        commands.entity(entity).remove::<Selected>();
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
            params: vec![ParamValue{name: "mean", value: 0.},ParamValue{name: "std_dev", value: 1.}]
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

// Submit the text when Ctrl+Enter is pressed
fn text_submission(
    input_focus: Res<InputFocus>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut text_input: Query<(&mut EditableText, &ParamTextbox, &Name)>,
    node_to_change: Single<(&mut RandomVar, &Selected)>
) {
    if keyboard_input.just_pressed(KeyCode::Enter)
        && let Some(focused_entity) = input_focus.get()
        && let Ok((mut text_input, param_num, _name)) = text_input.get_mut(focused_entity)
    {
        let (mut random_var, _selected) = node_to_change.into_inner();
        let num = text_input.value().to_string().parse::<f64>();
        match num {
            Ok(f) => {
                random_var.params.get_mut(param_num.0).expect("invalid param_num").value = f;
            }
            Err(_e) => {
                println!("Not a valid parameter number!");
                text_input.clear();
            }
        }
        reset_dist(&mut random_var);
        //random_var
        //set the random_var's dist value to a new distribution with updated param
    }
}

fn reset_dist(node: &mut RandomVar) {
    let p = |i: usize| {
        node.params
            .get(i)
            .unwrap_or_else(|| panic!("missing param {} for {}", i, node.dist_type))
            .value
    };

    node.dist = match node.dist_type.as_str() {
        "Normal" => Box::new(Normal::new(p(0), p(1)).unwrap().clone()),
        "LogNormal" => Box::new(LogNormal::new(p(0), p(1)).unwrap().clone()),
        "Exponential" => Box::new(Exponential::new(p(0)).unwrap().clone()),
        "Gamma" => Box::new(Gamma::new(p(0), p(1)).unwrap().clone()),
        "Beta" => Box::new(Beta::new(p(0), p(1)).unwrap().clone()),
        "Uniform" => Box::new(Uniform::new(p(0), p(1)).unwrap().clone()),
        other => panic!("unsupported distribution type: {}", other),
    };
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

    let param_name = param.name;
    let param_value = param.value;

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

fn node_settings(
    mut commands: Commands,
    selected: Option<Single<(Entity, &mut Selected, &mut GraphNode)>>,
    mut random_vars: Query<&mut RandomVar>,
    mut finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    sidebar: Query<(Entity, &Sidebar)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut names: Query<(&NamedNode, &ChildOf)>
){
    for (sidebar_entity, _comp) in sidebar.iter(){
        commands.entity(sidebar_entity).despawn();
    }
    if let Some(single) = selected{
        let (entity, _selected_comp, node) = single.into_inner();
        let mut name = None;
        let mut random_var = random_vars.get(entity).unwrap();
        for (label, parent) in names.iter(){
            if parent.parent() == entity{
                name = Some(label.0.clone());
            }
        }
        let text_transform: Vec3 = (0f32, (CANVAS_HEIGHT / 2.0) - 15.0, 1f32).into();
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
                ),
                (
                    Text::new(random_var.dist_type.clone()),
                    Node {
                        margin: px(12).bottom(),
                        ..default()
                    },
                    TextColor(NODE_NAME_COLOR),
                ),
            ],
        )).id();
        for (i, _param) in random_var.params.iter().enumerate() {        
            let child = build_param_textbox(
                &mut commands,
                random_var,
                i,
            );
            commands.entity(sidebar_entity).add_child(child);
        }
    }
}

fn selection_changed(
    added: Query<(), Added<Selected>>,
    removed: RemovedComponents<Selected>,
    renamed: Query<(), Added<NamedNode>>
) -> bool {
    !added.is_empty() || removed.len() > 0 || !renamed.is_empty()
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MeshPickingPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (on_keypress, node_settings.run_if(selection_changed), text_submission))
        .run();
}



// PROGRESS
/*

Next steps:

Dragging nodes                              DONE
Shiftclick to create an arrow               DONE


Goals for 7/2:

Arrowhead (custom mesh?)                    DONE
Arrows on drag                              DONE
Arrows disappear on node deletion           DONE

Goals for 7/7:

Basic fugue scaffolding w/ normal dists     DONE
Simple sampling?
Plates, parameters


Goals for 7/10:

Node sidebar{
    random vs parameter
    dist. params
}
Plate dragging creation


Future goals:

Single click allows node name editing,
eventually will be -> popup with 
distribution/property editing
Various distribution options
Single sampling/forward sampling
Plot viewing
Crosslink, brushing interaction


Optional goals:

Ghost arrow after shift-clicking a node
that tracks cursor until end node is clicked.

Different color schemes

Address all uses of .unwrap()



Bug tracker:

Deletion of a node in an UnfinishedLink
leads to panic                                 FIXED

Smashing keys on rename interacts with
a despawned entity (probably NamedNode)
and panics

*/