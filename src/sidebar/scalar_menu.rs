use bevy::{prelude::*, text::EditableText};
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