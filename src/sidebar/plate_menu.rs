use bevy::text::EditableText;

use super::*;


impl SidebarContent for Plate{
    fn build(
        &self, 
        mut commands: &mut Commands, 
        sidebar_entity: Entity, 
        node_data: &Query<(Option<&RandomNode>, Option<&ScalarNode>, Option<&ComputeNode>)>,
        finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
        node: Entity
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
            Name::new(format!("plate_iteration_box")),
            children![
                (
                    Text::new("N"),
                    TextColor(NODE_NAME_COLOR),
                ),
                (
                    PlateNTextbox,
                    Node {
                        width: px(120.),
                        min_height: px(25.),
                        border: px(2).all(),
                        padding: px(4).all(),
                        ..default()
                    },
                    BorderColor::from(Color::from(SLATE_300)),
                    BackgroundColor(DARK_GREY.into()),
                    EditableText::new(self.n.to_string()),
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
