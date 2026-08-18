use super::*;
use crate::constants::*;
use crate::graph::*;
use crate::ui::capsule_selection_indicator;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use fugue::*;

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
    let label = random_node_label(&random_node, node_num);
    commands
        .spawn((
            GraphNode(node_num),
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
            Mesh2d(meshes.add(random_node_mesh(RANDOM_NODE_RAD, &label))),
            MeshMaterial2d(materials.add(RANDOM_NODE_COLOR)),
            Transform::from_xyz(loc.x, loc.y, 1.0),
            random_node,
        ))
        .with_child((
            NodeInterior,
            Pickable::IGNORE,
            Mesh2d(meshes.add(random_node_mesh(
                RANDOM_NODE_RAD - NODE_BORDER_WEIGHT,
                &label,
            ))),
            MeshMaterial2d(materials.add(CANVAS_COLOR)),
            Transform::from_xyz(0.0, 0.0, 0.01),
        ))
        .with_child((
            NodeLabel,
            Text2d::new(label),
            text_font(),
            TextColor(NODE_NAME_COLOR),
            Pickable::IGNORE,
            Transform::from_xyz(0.0, 0.0, 2.0),
        ))
        .observe(on_node_drag)
        .observe(on_node_click)
        .id()
}

pub fn random_node_label(random_node: &RandomNode, node_num: u32) -> String {
    random_node
        .name
        .clone()
        .unwrap_or_else(|| node_num.to_string())
}

pub fn random_node_mesh(radius: f32, label: &str) -> Mesh {
    let extra_length = label
        .chars()
        .count()
        .saturating_sub(1) as f32
        * RANDOM_NODE_NAME_ADVANCE;
    if extra_length == 0.0 {
        Mesh::from(Circle::new(radius))
    } else {
        Mesh::from(Capsule2d::new(radius, extra_length))
            .rotated_by(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
    }
}

pub fn random_selection_mesh(label: &str) -> Mesh {
    let extra_length = label
        .chars()
        .count()
        .saturating_sub(1) as f32
        * RANDOM_NODE_NAME_ADVANCE;
    capsule_selection_indicator(RANDOM_NODE_RAD, extra_length)
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
