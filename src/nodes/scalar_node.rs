use bevy::prelude::*;
use super::*;


pub fn new_scalar(
    commands: &mut Commands,
    loc: Vec3,
    node_num: u32,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
){
    commands.spawn((
        GraphNode(node_num),
        Pickable{should_block_lower: true, is_hoverable: true},
        Mesh2d(meshes.add(Circle::new(NODE_RAD*0.5))),
        MeshMaterial2d(materials.add(SCALAR_NODE_COLOR)),
        Transform::from_xyz(
            loc.x,
            loc.y,
            1.0),
        ScalarNode{      //TODO: move to global sidebar
            val: 1.
        }
    )).with_child((
        NodeLabel("1".to_string()),
        Text2d::new("1"),
        TextFont{
            font_size: px(12).into(),
            ..default()
        },
        TextColor(NODE_NAME_COLOR),
        Pickable::IGNORE,
        Transform::from_xyz(0.0,0.0,2.0)
    ))
    .observe(on_node_drag)
    .observe(on_node_click);
}


pub fn replace_node_label(
    commands: &mut Commands,
    node_entity: Entity,
    label_text: impl Into<String>,
    labels: &Query<(Entity, &NodeLabel, &ChildOf)>,
) {
    let label_text = label_text.into();

    for (label_entity, _, child_of) in labels.iter() {
        if child_of.parent() == node_entity {
            commands.entity(label_entity).despawn();
        }
    }

    commands.entity(node_entity).with_child((
        NodeLabel(label_text.clone()),
        Text2d::new(label_text.clone()),
        TextColor(NODE_NAME_COLOR),
        TextFont{
            font_size: match &label_text.len() {
                n if *n > 1 => px(NODE_LABEL_FONT_SIZE_SMALL).into(),
                _ => px(NODE_LABEL_FONT_SIZE).into()
            },
            ..default()
        },
        Pickable::IGNORE,
        Transform::from_xyz(0.0, 0.0, 2.0),
    ));
}