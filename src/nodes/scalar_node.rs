use super::*;
use bevy::{input_focus::InputFocus, prelude::*, sprite::Anchor, text::EditableText};

pub fn new_scalar(
    commands: &mut Commands,
    loc: Vec3,
    node_num: u32,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    spawn_scalar(
        commands,
        loc,
        node_num,
        ScalarNode { val: 1.0 },
        &mut meshes,
        &mut materials,
    );
}

pub fn spawn_scalar(
    commands: &mut Commands,
    loc: Vec3,
    node_num: u32,
    scalar_node: ScalarNode,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) -> Entity {
    let label = format!("{:.1}", scalar_node.val);
    commands
        .spawn((
            GraphNode(node_num),
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
            Mesh2d(meshes.add(Circle::new(SCALAR_NODE_RAD))),
            MeshMaterial2d(materials.add(SCALAR_NODE_COLOR)),
            Transform::from_xyz(loc.x, loc.y, 1.0),
            scalar_node,
        ))
        .with_child((
            NodeLabel,
            Text2d::new(label.clone()),
            TextColor(NODE_NAME_COLOR),
            TextFont {
                font_size: match &label.len() {
                    n if *n > 1 => px(NODE_LABEL_FONT_SIZE_SMALL).into(),
                    _ => px(NODE_LABEL_FONT_SIZE).into(),
                },
                ..default()
            },
            Pickable::IGNORE,
            Transform::from_xyz(0.0, 0.0, 2.0),
        ))
        .observe(on_node_drag)
        .observe(on_node_click)
        .id()
}

pub fn replace_node_label(
    commands: &mut Commands,
    node_entity: Entity,
    label_text: impl Into<String>,
    labels: &Query<(Entity, &NodeLabel, &ChildOf)>,
    selected_plate: Option<&Plate>,
) {
    let label_text = label_text.into();

    for (label_entity, _, child_of) in labels.iter() {
        if child_of.parent() == node_entity {
            commands.entity(label_entity).despawn();
        }
    }

    if let Some(plate) = selected_plate {
        commands.entity(node_entity).with_child((
            NodeLabel,
            Text2d::new(label_text.clone()),
            TextColor(PLATE_COLOR),
            Anchor::BOTTOM_RIGHT,
            TextFont {
                font_size: px(NODE_LABEL_FONT_SIZE).into(),
                ..default()
            },
            Pickable::IGNORE,
            Transform::from_translation(plate.get_corner_pos()),
        ));
    } else {
        commands.entity(node_entity).with_child((
            NodeLabel,
            Text2d::new(label_text.clone()),
            TextColor(NODE_NAME_COLOR),
            TextFont {
                font_size: match &label_text.len() {
                    n if *n > 1 => px(NODE_LABEL_FONT_SIZE_SMALL).into(),
                    _ => px(NODE_LABEL_FONT_SIZE).into(),
                },
                ..default()
            },
            Pickable::IGNORE,
            Transform::from_xyz(0.0, 0.0, 2.0),
        ));
    }
}
