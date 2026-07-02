use core::f32;

use bevy::{
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
    asset::RenderAssetUsages,
    text::{FontSmoothing, TextBounds},
    input::keyboard::KeyboardInput
};


const CANVAS_HEIGHT: f32 = 500.0;
const CANVAS_WIDTH: f32 = 800.0;
const NODE_RAD: f32 = 20.0;
const CANVAS_COLOR: Color = Color::WHITE;
const ARROW_COLOR: Color = Color::BLACK;
const NODE_NAME_COLOR: Color = Color::BLACK;
const NODE_COLOR: Color = Color::srgb(1.0, 0.0, 0.0);
const ARROW_THICKNESS: f32 = 2.0;
const ARROW_TIP_WIDTH_RATIO: f32 = 10.0;
const ARROW_TIP_LENGTH: f32 = 10.0;

#[derive(Component)]
struct GraphNode(u32);

#[derive(Component)]
struct NamedNode(String);

#[derive(Component)]
struct UnnamedNode;

#[derive(Component)]
struct Canvas;

#[derive(Component)]
struct GraphLink{
    from: Entity,
    to: Option<Entity>
}

#[derive(Component)]
struct UnfinishedLink;

#[derive(Component)]
struct Selected;


//custom arrow mesh
fn arrow_mesh(length: f32) -> Mesh {
    let hw = length / 2.0;
    let hs = ARROW_THICKNESS / 2.0;
    let hh = hs * ARROW_TIP_WIDTH_RATIO;
    let tx = hw - ARROW_TIP_LENGTH;

    let vertices: Vec<[f32; 3]> = vec![
        [-hw, hs, 0.0],   // 0: shaft top-left
        [-hw, -hs, 0.0],  // 1: shaft bottom-left
        [tx, hs, 0.0],    // 2: shaft top-right
        [tx, -hs, 0.0],   // 3: shaft bottom-right
        [tx, hh, 0.0],    // 4: head top
        [tx, -hh, 0.0],   // 5: head bottom
        [hw, 0.0, 0.0],   // 6: tip
    ];

    let indices = vec![0u32, 1, 2, 2, 1, 3, 4, 5, 6];

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
    .with_inserted_indices(Indices::U32(indices))
}


//helper function to compute arrow transform
fn link_transform_helper(
    link: &GraphLink,
    transforms: &Query<&mut Transform>,
    meshes: &mut ResMut<Assets<Mesh>>

) -> Option<(Transform, Mesh2d)> {
    let to = link.to?;

    let from_pos = transforms.get(link.from).ok()?.translation;
    let to_pos = transforms.get(to).ok()?.translation;

    let delta = to_pos - from_pos;
    let angle = delta.y.atan2(delta.x);
    let midpoint = from_pos.lerp(to_pos, 0.5);
    let length = from_pos.distance(to_pos) - (2.0 * NODE_RAD);

    Some(((Transform {
            translation: midpoint,
            rotation: Quat::from_rotation_z(angle),
            scale: Vec3::new(1.0, 1.0, 1.0)}),
        (Mesh2d(meshes.add(arrow_mesh(length))))))
}


//update arrow transforms connecting to dragged node
fn on_node_drag (
    event: On<Pointer<Drag>>,
    mut transforms: Query<&mut Transform>,
    mut mesh_query: Query<&mut Mesh2d>,
    mut graph_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    mut meshes: ResMut<Assets<Mesh>>
) {
    println!("Node drag event");
    {
        if let Ok(mut ent) = transforms.get_mut(event.event_target()) {
        ent.translation.x += event.delta.x;     //update node position
        ent.translation.y -= event.delta.y;
    }
}
    for (link_entity, link_component) in graph_links.iter_mut() { 
        if event.event_target() == link_component.from || event.event_target() == link_component.to.unwrap() {  //update all connected arrow positions/meshes
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

    for event in kbd.read() {
        if !event.state.is_pressed() {
            continue;
        }

        let Some(text) = &event.text else {
            continue;
        };
        if text.len() != 1 || !text.chars().all(|c| c.is_alphabetic()) {
            continue;
        }

        for (unnamed_entity, _unnamed_node, parent) in unnamed.iter_mut() {
            if parent.parent() == entity {
                commands.entity(unnamed_entity).despawn();
            }
        }
        for (named_entity, _named_node, parent) in named.iter_mut() {
            if parent.parent() == entity {
                commands.entity(named_entity).despawn();
            }
        }
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
    selected: Option<Single<(Entity, &mut Selected)>>
){
    if input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]){        //if it is a shift click:
        println!("Node shift click event");

        if let Ok((unfinished_ent, mut ends)) = unfinished_link.single_mut() {  //if there is an unfinished GraphLink, complete it.

            commands.entity(unfinished_ent).remove::<UnfinishedLink>();

            if ends.from == event.event_target() { //if user tries to create a link from a node to itself
                commands.entity(unfinished_ent).despawn();
                return;
            }

            ends.to = Some(event.event_target());

            println!("Completed a GraphLink");

            if let Some((arrow_transform, arrow_mesh)) = link_transform_helper(&ends, &transforms, &mut meshes) {
                commands.entity(unfinished_ent).insert((
                    arrow_mesh,
                    MeshMaterial2d(materials.add(ARROW_COLOR)),     //add arrow
                    arrow_transform,
                ));
            }
            
        }else{      //otherwise, create an invisible UnfinishedLink 
            commands.spawn((
                GraphLink{
                    from: event.event_target(),
                    to: None
                },
                UnfinishedLink
            ));
            println!("Created an UnfinishedLink");
        }
    } else {        //normal click, select the node
        println!("Node click event");
        if event.duration.as_millis() < 200 && event.count == 1 {

            if let Some(single) = selected{     //deselect currently selected node
                let (entity, _selected_comp) = single.into_inner();
                commands.entity(entity).remove::<Selected>();
            }
            commands.entity(event.event_target()).insert(   //select this node
                Selected
            );
        }
        if event.duration.as_millis() < 200 && event.count > 1 { //double click, delete node
            commands.entity(event.entity).despawn();
            
            for (link_entity, link_component) in finished_links.iter_mut() {
                if event.event_target() == link_component.from {
                    commands.entity(link_entity).despawn();
                }
                if event.event_target() == link_component.to.unwrap() {
                    commands.entity(link_entity).despawn();
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
    current_nodes: Query<&GraphNode>
) {
    println!("Click event");

    let mut node_num = 1;

    while current_nodes.iter().any(|node| node.0 == node_num) { //finds the lowest unused node in the least efficient way possible
        node_num += 1;
    }

    commands.spawn((
        GraphNode(node_num),
        Pickable{should_block_lower: true, is_hoverable: true},
        Mesh2d(meshes.add(Circle::new(NODE_RAD))),
        MeshMaterial2d(materials.add(NODE_COLOR)),
        Transform::from_xyz(
            event.hit.position.unwrap().x,
            event.hit.position.unwrap().y,
            1.0)
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

    commands.spawn((        //spawn clickable background
        Canvas,
        Mesh2d(meshes.add(Rectangle::new(CANVAS_WIDTH, CANVAS_HEIGHT))),
        MeshMaterial2d(materials.add(CANVAS_COLOR))
    ))
    .observe(on_background_click);

    commands.spawn((
        Text2d::new("7.1.2026\n\
                        Click to create a new node.\n\
                        Shift click a parent and then a child\n\
                        node to create a link between them.\n\
                        Double-click a node to delete it.\n\
                        Click a node, then a letter key,\n\
                        to assign it a one-letter name."),
        Transform{
            translation: vec3(-450.,300.0,1.0),
            scale: vec3(0.5,0.5,1.0),
            rotation: Quat::from_rotation_z(0.0)
        }
    ));
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MeshPickingPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, on_keypress)
        .run();
}



// PROGRESS
/*

Next steps:

Dragging nodes                              DONE
Shiftclick to create an arrow               DONE


Goals for thursday:

Arrowhead (custom mesh?)                    DONE
Arrows  on drag                             DONE
Arrows disappear on node deletion           DONE


Future goals:

Single click allows node name editing,
eventually will be -> popup with 
distribution/property editing


Optional goals:

Ghost arrow after shift-clicking a node
that tracks cursor until end node is clicked.



*/