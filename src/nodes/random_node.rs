use super::*;
use crate::constants::*;
use crate::graph::*;
use crate::sidebar::*;
use crate::ui::*;
use bevy::input::keyboard::KeyboardInput;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use fugue::*;
use rand::thread_rng;

pub fn new_random(
    commands: &mut Commands,
    loc: Vec3,
    node_num: u32,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    spawn_random(
        commands,
        loc,
        node_num,
        RandomNode {
            name: None,
            dist_type: String::from("Normal"),
            dist: Box::new(Normal::new(0.0, 1.0).unwrap().clone()),
            params: vec![ParamValue("mean", None), ParamValue("std_dev", None)],
        },
        &mut meshes,
        &mut materials,
    );
}

pub fn spawn_random(
    commands: &mut Commands,
    loc: Vec3,
    node_num: u32,
    random_node: RandomNode,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) -> Entity {
    let label = random_node
        .name
        .clone()
        .unwrap_or_else(|| node_num.to_string());
    commands
        .spawn((
            GraphNode(node_num),
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
            Mesh2d(meshes.add(Circle::new(RANDOM_NODE_RAD))),
            MeshMaterial2d(materials.add(RANDOM_NODE_COLOR)),
            Transform::from_xyz(loc.x, loc.y, 1.0),
            random_node,
        ))
        .with_child((
            NodeInterior,
            Pickable::IGNORE,
            Mesh2d(meshes.add(Circle::new(RANDOM_NODE_RAD - NODE_BORDER_WEIGHT))),
            MeshMaterial2d(materials.add(CANVAS_COLOR)),
            Transform::from_xyz(0.0, 0.0, 0.01),
        ))
        .with_child((
            NodeLabel,
            Text2d::new(label),
            TextColor(NODE_NAME_COLOR),
            Pickable::IGNORE,
            Transform::from_xyz(0.0, 0.0, 2.0),
        ))
        .observe(on_node_drag)
        .observe(on_node_click)
        .id()
}

//rename selected node to single-letter name from keyboard
pub fn on_keypress(
    mut kbd: MessageReader<KeyboardInput>,
    mut commands: Commands,
    selected: Option<Single<(Entity, &mut RandomNode), With<Selected>>>,
    labels: Query<(Entity, &NodeLabel, &ChildOf)>,
) {
    let Some(single) = selected else {
        return;
    };
    let (entity, mut random_node) = single.into_inner();

    //for all keyboard inputs while node is selected
    for event in kbd.read() {
        if !event.state.is_pressed() {
            continue;
        }
        let Some(text) = &event.text else {
            continue;
        };
        //only alphabetic, numbers reserved for unnamed nodes
        if text.chars().count() != 1 || !text.chars().all(|c| c.is_alphabetic()) {
            continue;
        }
        random_node.name = Some(text.to_string());

        for (label_entity, _, child_of) in labels.iter() {
            if child_of.parent() == entity {
                commands.entity(label_entity).despawn();
            }
        }

        commands.entity(entity).with_child((
            NodeLabel,
            Text2d::new(text.to_string()),
            TextColor(NODE_NAME_COLOR),
            Pickable::IGNORE,
            Transform::from_xyz(0.0, 0.0, 2.0),
        ));
        //reload sidebar
        commands.trigger(ReloadSidebar);
    }
}

//store parameters for distributions plus a valid default value
pub fn distribution_params() -> HashMap<String, Vec<ParamValue>> {
    HashMap::from([
        (
            String::from("Normal"),
            vec![ParamValue("mean", None), ParamValue("std_dev", None)],
        ),
        (
            String::from("LogNormal"),
            vec![ParamValue("mean", None), ParamValue("std_dev", None)],
        ),
        (
            String::from("Gamma"),
            vec![ParamValue("shape", None), ParamValue("scale", None)],
        ),
        (
            String::from("Beta"),
            vec![ParamValue("alpha", None), ParamValue("beta", None)],
        ),
        (String::from("Exponential"), vec![ParamValue("rate", None)]),
        (
            String::from("Uniform"),
            vec![ParamValue("min", None), ParamValue("max", None)],
        ),
    ])
}
