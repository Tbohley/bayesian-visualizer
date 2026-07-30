use bevy::{input_focus::InputFocus, prelude::*, text::EditableText};
use super::*;

impl SidebarContent for ScalarNode{
    fn build(
        &self, 
        commands: &mut Commands, 
        sidebar_entity: Entity,
        _node_data: &Query<(Option<&RandomNode>, Option<&ScalarNode>, Option<&ComputeNode>)>,
        _finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
        _node: Entity
    ){
        commands.entity(sidebar_entity).with_child(divider());

        commands.entity(sidebar_entity).with_child((
            Node {
                width: percent(100.),
                flex_direction: FlexDirection::Column,
                row_gap: px(4.),
                margin: px(8.).bottom(),
                ..default()
            },
            Name::new(format!("value_box")),
            children![
                (
                    Text::new("value"),
                    TextColor(NODE_NAME_COLOR),
                ),
                (
                    ScalarValueTextbox,
                    Node {
                        width: px(120.),
                        min_height: px(25.),
                        border: px(2).all(),
                        padding: px(4).all(),
                        ..default()
                    },
                    BorderColor::from(Color::from(SLATE_300)),
                    BackgroundColor(DARK_GREY.into()),
                    EditableText::new(self.val.to_string()),
                    TextLayout::no_wrap(),
                    TextCursorStyle::default(),
                    TabIndex(0),
                    Name::new(format!("value_textbox")),
                ),
            ],
        ));
        
        commands.entity(sidebar_entity).with_child(divider());

    }
}

// Submit the new param when Enter is pressed
pub fn on_enter_clicked(
    input_focus: Res<InputFocus>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut scalar_textboxes: Query<(&mut EditableText, &Name), With<ScalarValueTextbox>>,
    mut plate_textboxes: Query<(&mut EditableText, &Name), (With<PlateNTextbox>, Without<ScalarValueTextbox>)>,
    selected_scalar: Option<Single<(Entity, &mut ScalarNode, &Selected)>>,
    selected_plate: Option<Single<(Entity, &mut Plate, &Selected)>>,
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
                replace_node_label(&mut commands,scalar_entity,format!("{f:.1}"), &labels, None);
                commands.trigger(ReloadSidebar);
            }
            Err(_e) => {
                println!("Not a valid scalar number!");
                text_input.clear();
            }
        }
    }
    // Plate behavior
    if let Ok((mut text_input, _name)) = plate_textboxes.get_mut(focused_entity) {
        let Some(single) = selected_plate else {
            return;
        };
        let (plate_entity, mut plate_node, _selected) = single.into_inner();
        let num = text_input.value().to_string().parse::<usize>();
        match num {
            Ok(f) => {
                plate_node.n = f;
                replace_node_label(&mut commands,plate_entity,format!("{f:}"), &labels, Some(&plate_node));
                commands.trigger(ReloadSidebar);
            }
            Err(_e) => {
                println!("Not a valid plate size integer!");
                text_input.clear();
            }
        }
    }
}