use bevy::prelude::*;
use super::*;

impl SidebarContent for ComputeNode{
    fn build(
        &self, commands: &mut Commands, 
        sidebar_entity: Entity,
        _node_data: &Query<(Option<&RandomNode>, Option<&ScalarNode>, Option<&ComputeNode>)>,
        _finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
        _node: Entity
    ){
        commands.entity(sidebar_entity).with_child(
            (
                Text::new("Unfinished"),
                Node {
                    margin: px(4).bottom(),
                    ..default()
                },
                TextColor(NODE_NAME_COLOR),
            )
        );
    }
}