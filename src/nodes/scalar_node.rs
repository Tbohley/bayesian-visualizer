use bevy::{input_focus::InputFocus, prelude::*, text::EditableText};
use crate::sidebar::link_params::format_number;

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
        Mesh2d(meshes.add(Circle::new(SCALAR_NODE_RAD))),
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
        TextColor(NODE_NAME_COLOR),
        Pickable::IGNORE,
        Transform::from_xyz(0.0,0.0,2.0)
    ))
    .observe(on_node_drag)
    .observe(on_node_click);
}

// Submit the new param when Enter is pressed
pub fn on_enter_clicked(
    input_focus: Res<InputFocus>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut scalar_textboxes: Query<(&mut EditableText, &Name), With<ScalarValueTextbox>>,
    selected_scalar: Option<Single<(Entity, &mut ScalarNode, &Selected)>>,
    labels: Query<(Entity, &NodeLabel, &ChildOf)>,
    mut commands: Commands,
) {
    if !keyboard_input.just_pressed(KeyCode::Enter) {
        return;
    }
    let Some(focused_entity) = input_focus.get() else {
        return;
    }; 

    // Scalar-node value behavior
    if let Ok((mut text_input, _name)) = scalar_textboxes.get_mut(focused_entity) {
        let Some(single) = selected_scalar else {
            return;
        };
        let (scalar_entity, mut scalar_node, _selected) = single.into_inner();
        let num = text_input.value().to_string().parse::<f64>();
        match num {
            Ok(f) => {
                scalar_node.val = f;
                replace_node_label(&mut commands,scalar_entity,format!("{f:.1}"), &labels);
                commands.trigger(ReloadSidebar);
            }
            Err(_e) => {
                println!("Not a valid scalar number!");
                text_input.clear();
            }
        }
    }
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