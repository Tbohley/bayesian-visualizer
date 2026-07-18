use bevy::prelude::*;
use super::*;
use crate::nodes::replace_node_label;

impl SidebarContent for ComputeNode{
    fn build(
        &self, 
        mut commands: &mut Commands, 
        sidebar_entity: Entity,
        node_data: &Query<(Option<&RandomNode>, Option<&ScalarNode>, Option<&ComputeNode>)>,
        finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
        node: Entity
    ){
        commands.entity(sidebar_entity).with_child(divider());
        available_links(&mut commands, &node_data, &finished_links, sidebar_entity, node);
        commands.entity(sidebar_entity).with_child(divider());

        commands.entity(sidebar_entity).with_child((
            Text::new("Operation:"),
            Node {
                margin: px(4).bottom(),
                ..default()
            },
            TextColor(NODE_NAME_COLOR),
        ));

        let context_menu = commands.spawn((
            Name::new("operation_context_menu"),
            Button,
            Node {
                margin: px(4).bottom(),
                width: px(SIDEBAR_WIDTH * 0.75),
                height: px(30),
                border: UiRect::all(px(5)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(Color::BLACK),
            BackgroundColor(Color::BLACK),
            children![(
                Pickable::IGNORE,
                Text::new(format!("{:?}", self.operation)),
                TextColor(Color::WHITE),
                TextShadow::default(),
            )],
        )).observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
            event.propagate(false);
            println!("Clicked context menu");
            debug!("click: {}", event.pointer_location.position);

            commands.trigger(OpenOperationMenu {
                pos: event.pointer_location.position,
            });
        }).id();
        commands.entity(sidebar_entity).add_child(context_menu);
        commands.entity(sidebar_entity).with_child(divider());


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

pub fn on_open_operation_menu(
    event: On<OpenOperationMenu>, 
    mut commands: Commands,
) {
    commands.trigger(CloseContextMenus);
    let pos = event.pos;
    debug!("open context menu at: {pos}");

    commands
        .spawn((
            Name::new("operation selector"),
            ContextMenu,
            ZIndex(999),
            Node {
                position_type: PositionType::Absolute,
                left: px(pos.x),
                top: px(pos.y),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BorderColor::all(Color::BLACK),
            BackgroundColor(Color::linear_rgb(0.1, 0.1, 0.1)),
            children![
                context_item("Add"),
                context_item("Subtract"),
                context_item("Multiply"),
                context_item("Divide"),
                context_item("Exp"),
                context_item("Log"),
                context_item("Power"),
                //context_item("Sum"),
                //context_item("Product")
            ],
        ))
        .observe(on_select_operation);
}


fn operation_symbol(operation: &Operation) -> &'static str {
    match operation {
        Operation::Add => "+",
        Operation::Subtract => "-",
        Operation::Multiply => "*",
        Operation::Divide => "/",
        Operation::Power => "^",
        Operation::Exponential => "exp",
        Operation::Logarithm => "log",
        Operation::Sum => "∑",
        Operation::Product => "π",
    }
}

fn on_select_operation(
    event: On<Pointer<Press>>,
    menu_items: Query<&ContextMenuItem>,
    mut commands: Commands,
    selected: Option<Single<(Entity, &Selected)>>,
    mut compute_vars: Query<(&mut ComputeNode, &Selected)>,
    labels: Query<(Entity, &NodeLabel, &ChildOf)>,
){
    let target = event.original_event_target();
    if let Some(single) = selected{
        let (entity, _selected_comp) = single.into_inner();

        if let Ok(item) = menu_items.get(target) {
            //set operation/label of compute node
            println!("Selected operation {}", item.0);
            let (mut compute_var, _selected_comp) = compute_vars.get_mut(entity).unwrap();
            compute_var.operation = match item.0.as_str() {
                "Add" => Operation::Add,
                "Subtract" => Operation::Subtract,
                "Multiply" => Operation::Multiply,
                "Divide" => Operation::Divide,
                "Power" => Operation::Power,
                "Exp" => Operation::Exponential,
                "Log" => Operation::Logarithm,
                "Sum" => Operation::Sum,
                "Product" => Operation::Product,
                _ => Operation::Add
            };

            replace_node_label(
                &mut commands,
                entity,
                operation_symbol(&compute_var.operation),
                &labels,
            );
            
            commands.trigger(CloseContextMenus);
            commands.trigger(ReloadSidebar);
            
        }
    }
}