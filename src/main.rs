use core::f32;

use bevy::prelude::*;

const CANVAS_HEIGHT: f32 = 500.0;
const CANVAS_WIDTH: f32 = 800.0;
const NODE_RAD: f32 = 20.0;
const CANVAS_COLOR: (f32, f32,f32) = (0.1,0.5,0.1);
const NODE_COLOR: (f32, f32, f32) = (0.1,0.1,0.5);
const LINK_THICKNESS: f32 = 2.0;

#[derive(Component)]
struct GraphNode;

#[derive(Component)]
struct Canvas;

#[derive(Component)]
struct GraphLink{
    from: Entity,
    to: Option<Entity>
}

#[derive(Component)]
struct UnfinishedLink;


//helper function to compute arrow transform
fn link_transform_helper(
    link: &GraphLink,
    transforms: &Query<&mut Transform>,
) -> Option<Transform> {
    let to = link.to?;

    let from_pos = transforms.get(link.from).ok()?.translation;
    let to_pos = transforms.get(to).ok()?.translation;

    let delta = to_pos - from_pos;
    let angle = delta.y.atan2(delta.x);
    let midpoint = from_pos.lerp(to_pos, 0.5);
    let length = from_pos.distance(to_pos) - (2.0 * NODE_RAD);

    Some(Transform {
        translation: midpoint,
        rotation: Quat::from_rotation_z(angle),
        scale: Vec3::new(length, 1.0, 1.0),     //length of link rectangle is exclusively controlled by scale, not the mesh dims
    })
}


//update arrow transforms connecting to dragged node
fn on_node_drag (
    event: On<Pointer<Drag>>,
    mut transforms: Query<&mut Transform>,
    mut graph_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
) {
    println!("Node drag event");
    {
        if let Ok(mut ent) = transforms.get_mut(event.event_target()) {
        ent.translation.x += event.delta.x;
        ent.translation.y -= event.delta.y;
    }
}
    for (link_entity, link_component) in graph_links.iter_mut() {
        if event.event_target() == link_component.from {
            let new_transform = link_transform_helper(&link_component, &transforms).unwrap();
            if let Ok(mut from_transform) = transforms.get_mut(link_entity) {
                *from_transform = new_transform;
            }
        }
        if event.event_target() == link_component.to.unwrap() {
            let new_transform = link_transform_helper(&link_component, &transforms).unwrap();
            if let Ok(mut to_transform) = transforms.get_mut(link_entity) {
                *to_transform = new_transform;
            }
        }
    }
}

//multifunctional: single click to despawn a node, shift click two nodes consecutively to create a link, double click to edit properties.
fn on_node_click(
    event: On<Pointer<Click>>,
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    mut unfinished_link: Query<(Entity, &mut GraphLink), With<UnfinishedLink>>,
    mut finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    transforms: Query<&mut Transform>,
){
    if input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]){        //if it is a shift click:
        println!("Node shift click event");

        if let Ok((unfinished_ent, mut ends)) = unfinished_link.single_mut() {  //if there is an unfinished GraphLink, complete it.

            commands.entity(unfinished_ent).remove::<UnfinishedLink>();

            if ends.from == event.event_target() { //if user tries to create a link from a node to itself
                commands.entity(unfinished_ent).despawn();
                return;
            }

            ends.to = Some(event.event_target().clone());

            println!("Completed a GraphLink");

            if let Some(transform) = link_transform_helper(&ends, &transforms) {
                commands.entity(unfinished_ent).insert((
                    Mesh2d(meshes.add(Rectangle::new(1.0, LINK_THICKNESS))),       //see comment on link_transform_helper for why width = 1.0
                    MeshMaterial2d(materials.add(Color::WHITE)),
                    transform,
                ));
            }
            
        }else{      //otherwise, create an UnfinishedLink
            commands.spawn((
                GraphLink{
                    from: event.event_target(),
                    to: None
                },
                UnfinishedLink
            ));
            println!("Created an UnfinishedLink");
        }
    } else {        //not a shift click, kill node and connected links (for now). Eventually, if count > 1 (double click), edit the node's properties with a pop-up UI.
        println!("Node click event");
        if event.duration.as_millis() < 200 {
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
fn on_click(
    mut event: On<Pointer<Click>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>
) {
    println!("Click event");
    commands.spawn((
        GraphNode,
        Pickable{should_block_lower: true, is_hoverable: true},
        Mesh2d(meshes.add(Circle::new(NODE_RAD))),
        MeshMaterial2d(materials.add(Color::srgb(NODE_COLOR.0, NODE_COLOR.1, NODE_COLOR.2))),
        Transform::from_xyz(
            event.hit.position.unwrap().x,
            event.hit.position.unwrap().y,
            1.0)
    ))
    .observe(on_node_drag)
    .observe(on_node_click);
    event.propagate(true);
}

fn setup (
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>
) {
    commands.spawn(Camera2d);

    commands.spawn((        //spawn clickable background
        Canvas,
        Mesh2d(meshes.add(Rectangle::new(CANVAS_WIDTH, CANVAS_HEIGHT))),
        MeshMaterial2d(materials.add(Color::srgb(CANVAS_COLOR.0, CANVAS_COLOR.1, CANVAS_COLOR.2)))
    ))
    .observe(on_click);
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MeshPickingPlugin))
        .add_systems(Startup, setup)
        //.add_systems(Update, on_node_drag)
        .run();
}



// PROGRESS
/*

Next steps:

Dragging nodes                              DONE
Shiftclick to create an arrow               DONE


Goals for thursday:

Arrowhead (custom mesh?)
Arrows  on drag                             DONE
Arrows disappear on node deletion           DONE




*/