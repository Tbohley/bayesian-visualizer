use bevy::{input_focus::InputFocus, prelude::*, text::EditableText};
use super::*;

impl SidebarContent for ScalarNode{
    fn build(
        &self, 
        commands: &mut Commands, 
        sidebar_entity: Entity,
        _node_data: &Query<(Option<&RandomNode>, Option<&ScalarNode>, Option<&ComputeNode>)>,
        _finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
        _node: Entity,
        observed: bool,
        _observed_columns: &std::collections::HashMap<Entity, String>,
    ){
        add_node_name_field(commands, sidebar_entity, self.name.as_deref());
        commands.entity(sidebar_entity).with_child(divider());

        let value_box = commands.spawn((
            Node {
                width: percent(100.),
                flex_direction: FlexDirection::Column,
                row_gap: px(4.),
                margin: px(8.).bottom(),
                ..default()
            },
            Name::new(format!("value_box")),
        )).id();
        commands.entity(sidebar_entity).add_child(value_box);
        commands.entity(value_box).with_child((
            Text::new("value"),
            text_font(),
            TextColor(NODE_NAME_COLOR),
        ));

        if observed {
            commands.entity(value_box).with_child((
                Node {
                    width: px(120.),
                    min_height: px(25.),
                    border: px(2).all(),
                    padding: px(4).all(),
                    ..default()
                },
                BorderColor::from(Color::from(SLATE_300)),
                BackgroundColor(DARK_GREY.into()),
                Text::new("from data"),
                text_font(),
                TextLayout::no_wrap(),
                Name::new("value_textbox"),
            ));
        } else {
            commands.entity(value_box).with_child((
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
                    text_font(),
                    TextLayout::no_wrap(),
                    TextCursorStyle::default(),
                    TabIndex(1),
                    Name::new(format!("value_textbox")),
            ));
        }
        
        commands.entity(sidebar_entity).with_child(divider());

    }
}

// Submit the new param when Enter is pressed
pub fn on_enter_clicked(
    input_focus: Res<InputFocus>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut scalar_textboxes: Query<
        (&mut EditableText, &Name),
        (
            With<ScalarValueTextbox>,
            Without<NodeNameTextbox>,
            Without<PlateNTextbox>,
        ),
    >,
    node_name_textboxes: Query<
        &EditableText,
        (
            With<NodeNameTextbox>,
            Without<ScalarValueTextbox>,
            Without<PlateNTextbox>,
        ),
    >,
    mut plate_textboxes: Query<
        (&mut EditableText, &Name),
        (
            With<PlateNTextbox>,
            Without<ScalarValueTextbox>,
            Without<NodeNameTextbox>,
        ),
    >,
    selected_scalar: Option<Single<(Entity, &mut ScalarNode, &Selected)>>,
    selected_random: Option<Single<Entity, (With<RandomNode>, With<Selected>)>>,
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

    if let Ok(text_input) = node_name_textboxes.get(focused_entity) {
        let entity = selected_random
            .map(|selected| *selected)
            .or_else(|| selected_scalar.as_ref().map(|selected| selected.0));
        if let Some(entity) = entity {
            commands.trigger(SetNodeName {
                entity,
                name: text_input.value().to_string(),
            });
        }
        return;
    }

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
                let label = scalar_node
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("{f:.1}"));
                replace_scalar_label(&mut commands, scalar_entity, label, &labels);
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
                plate_node.data.n = f;
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
