use std::collections::HashSet;

use bevy::prelude::*;
use super::*;
use crate::nodes::{*};

//fn get_ents_and_labels
/*
takes same args as available_links


*/
pub fn get_ents_and_labels(
    _commands: &mut Commands,
    node_data: &Query<(
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
    )>,
    finished_links: &Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    node: Entity,
) -> Vec<(Entity, String)> {
    finished_links
        .iter()
        .filter_map(|(_link_entity, link)| {
            if link.to == Some(node) {
                let mut visiting = HashSet::new();

                Some((
                    link.from,
                    node_expression(
                        link.from,
                        node_data,
                        &mut visiting,
                        false,
                    ),
                ))
            } else {
                None
            }
        })
        .collect()
}

//fn build_link_param_selector
/*
builds a context menu from a Vec<ParamValue(String param_name, Entity current_node)>, a Vec<(Entity available_node, String node_label)>.

displays node_label associated with current_node by default
*/
pub fn build_link_param_selector(
    commands: &mut Commands,
    link_labels: Vec<(Entity, String)>,
    params: Vec<ParamValue>,
    param_num: usize,
    sidebar_entity: &Entity,
) {
    let param = params
        .get(param_num)
        .expect("invalid param_num");

    let param_name = param.0;
    let param_entity = param.1;

    let param_label = param_entity
        .and_then(|selected_entity| {
            link_labels
                .iter()
                .find(|(ent, _label)| *ent == selected_entity)
                .map(|(_ent, label)| label.clone())
        })
        .unwrap_or_else(|| "Pick a link".to_string());

    let context_root = commands.spawn((
        Node {
            width: percent(100.),
            flex_direction: FlexDirection::Column,
            row_gap: px(4.),
            margin: px(8.).bottom(),
            ..default()
        },
        Name::new(format!("param_row_{}", param_name)),
        children![
            (
                Text::new(param_name),
                TextColor(NODE_NAME_COLOR),
            )],
    )).id();

    let button = commands.spawn((
        Name::new(format!("param_{}_context_menu", param_name)),
        Button,
        Node {
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
            Text::new(param_label),
            TextColor(Color::WHITE),
            TextShadow::default(),
        )],
    ))
    .observe(move |mut event: On<Pointer<Press>>, mut commands: Commands| {
        event.propagate(false);

        commands.trigger(OpenParamLinkMenu {
            pos: event.pointer_location.position,
            param_num,
        });
    })
    .id();
    commands.entity(context_root).add_child(button);

    commands.entity(*sidebar_entity).add_child(context_root);
}


pub fn on_open_param_link_menu(
    event: On<OpenParamLinkMenu>,
    mut commands: Commands,
    node_data: Query<(Option<&RandomNode>, Option<&ScalarNode>, Option<&ComputeNode>)>,
    finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    selected: Single<(Entity, &Selected, &GraphNode)>,
) {
    let (node, _selected_comp, _graphnode) = selected.into_inner();

    let link_labels = get_ents_and_labels(
        &mut commands,
        &node_data,
        &finished_links,
        node,
    );

    commands.trigger(CloseContextMenus);

    let pos = event.pos;

    let menu = commands
        .spawn((
            Name::new("param link selector"),
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
        ))
        .observe(on_pick_param)
        .id();

    if link_labels.is_empty() {
        commands.entity(menu).with_child((
            Name::new("item-no-links"),
            Button,
            Node {
                padding: UiRect::all(px(5)),
                ..default()
            },
            children![(
                Pickable::IGNORE,
                Text::new("No available links"),
                TextColor(Color::WHITE),
            )],
        ));
        return;
    }
    for (ent, text) in link_labels.iter() {
        let child = param_context_item(
            text.clone(),
            *ent,
            event.param_num,
        );

        commands.entity(menu).with_child(child);
    }
}

pub fn param_context_item(
    text: String,
    entity: Entity,
    param_num: usize,
) -> impl Bundle {
    (
        Name::new(format!("item-{text}")),
        ParamMenuItem{
            label: text.clone(),
            entity,
            param_num,
        },
        Button,
        Node {
            padding: UiRect::all(px(5)),
            ..default()
        },
        children![(
            Pickable::IGNORE,
            Text::new(text),
            TextColor(Color::WHITE),
        )],
    )
}
//fn on_pick_param
/*
also takes a Vec<ParamValue(String param_name, Entity current_node)> and an Entity from the context menu
modifies the Vec<ParamValue to match the Entity chosen
*/

fn on_pick_param(
    mut event: On<Pointer<Press>>,
    menu_items: Query<&ParamMenuItem>,
    mut commands: Commands,
    selected: Option<Single<(Entity, &Selected)>>,

    mut random_nodes: Query<&mut RandomNode>,
    mut compute_nodes: Query<&mut ComputeNode>,
) {
    event.propagate(false);
    let target = event.original_event_target();
    let Ok(item) = menu_items.get(target) else {
        return;
    };
    let Some(single) = selected else {
        return;
    };
    let (selected_entity, _selected) = single.into_inner();
    // Try RandomNode params first.
    if let Ok(mut random_node) = random_nodes.get_mut(selected_entity) {
        if let Some(param) = random_node.params.get_mut(item.param_num) {
            param.1 = Some(item.entity);

            println!(
                "Set RandomNode param '{}' to {}",
                param.0,
                item.label,
            );

            commands.trigger(CloseContextMenus);
            commands.trigger(ReloadSidebar);
            return;
        }
        println!("Invalid RandomNode param index: {}", item.param_num);
        return;
    }
    // Then try ComputeNode params.
    if let Ok(mut compute_node) = compute_nodes.get_mut(selected_entity) {
        if let Some(param) = compute_node.params.get_mut(item.param_num) {
            param.1 = Some(item.entity);
            println!(
                "Set ComputeNode param '{}' to {}",
                param.0,
                item.label,
            );
            commands.trigger(CloseContextMenus);
            commands.trigger(ReloadSidebar);
            return;
        }
        println!("Invalid ComputeNode param index: {}", item.param_num);
        return;
    }
    println!("Selected node has no assignable params");
}

//neither of these have to interact with any GraphLinks, any node_data 3-way queries for all node types,
//and both work exactly the same regardless of if it's a compute or random node being selected.

//except thats not how systems work you dumbass, and now this is spaghetti. bruh. oh well. if it works.


//helpers
fn node_expression(
    entity: Entity,
    node_data: &Query<(
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
    )>,
    visiting: &mut HashSet<Entity>,
    nested_inside_operation: bool,
) -> String {
    if !visiting.insert(entity) {
        return "?".to_string();
    }

    let expression = match node_data.get(entity) {
        Ok((random_node, scalar_node, compute_node)) => {
            if let Some(random_node) = random_node {
                random_short_label(random_node)
            } else if let Some(scalar_node) = scalar_node {
                scalar_label(scalar_node)
            } else if let Some(compute_node) = compute_node {
                compute_expression(
                    compute_node,
                    node_data,
                    visiting,
                    nested_inside_operation,
                )
            } else {
                "?".to_string()
            }
        }
        Err(_) => "?".to_string(),
    };

    visiting.remove(&entity);

    expression
}

fn random_short_label(random_node: &RandomNode) -> String {
    random_node
        .name
        .clone()
        .unwrap_or_else(|| format!("var"))
}

fn scalar_label(scalar_node: &ScalarNode) -> String {
    format_number(scalar_node.val)
}

pub fn format_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.3}")
    }
}

fn compute_expression(
    compute_node: &ComputeNode,
    node_data: &Query<(
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
    )>,
    visiting: &mut HashSet<Entity>,
    nested_inside_operation: bool,
) -> String {
    let expression = match &compute_node.operation {
        Operation::Add => infix_expression(compute_node," + ",node_data,visiting),
        Operation::Subtract => infix_expression(compute_node, " - ", node_data, visiting),
        Operation::Multiply => infix_expression(compute_node, " * ", node_data, visiting),
        Operation::Divide => infix_expression(compute_node, " / ", node_data, visiting),
        Operation::Power => infix_expression(compute_node, " ^ ", node_data, visiting),
        Operation::Exponential => {
            let input = function_arg_expression(compute_node, 0, node_data, visiting);
            format!("exp({input})")
        }
        Operation::Logarithm => {
            let input = function_arg_expression(compute_node, 0, node_data, visiting);
            format!("log({input})")
        }
        Operation::Sum => compact_expression("sum", compute_node, node_data, visiting),
        Operation::Product => compact_expression("product", compute_node, node_data, visiting),
    };

    if nested_inside_operation && is_infix_operation(&compute_node.operation) {
        format!("({expression})")
    } else {
        expression
    }
}

fn infix_expression(
    compute_node: &ComputeNode,
    operator: &str,
    node_data: &Query<(
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
    )>,
    visiting: &mut HashSet<Entity>,
) -> String {
    let left = infix_arg_expression(compute_node,0,node_data, visiting,);
    let right = infix_arg_expression(compute_node,1,node_data, visiting,);
    format!("{left}{operator}{right}")
}

fn infix_arg_expression(
    compute_node: &ComputeNode,
    param_index: usize,
    node_data: &Query<(
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
    )>,
    visiting: &mut HashSet<Entity>,
) -> String {
    let Some(entity) = compute_node
        .params
        .get(param_index)
        .and_then(|param| param.1)
    else {
        return "?".to_string();
    };
    node_expression(entity, node_data, visiting, true)
}

fn function_arg_expression(
    compute_node: &ComputeNode,
    param_index: usize,
    node_data: &Query<(
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
    )>,
    visiting: &mut HashSet<Entity>,
) -> String {
    let Some(entity) = compute_node
        .params
        .get(param_index)
        .and_then(|param| param.1)
    else {
        return "?".to_string();
    };

    // Do not force extra parens here.
    // `log(x + y)` is cleaner than `log((x + y))`.
    node_expression(entity, node_data, visiting, false)
}

fn compact_expression(
    function_name: &str,
    compute_node: &ComputeNode,
    node_data: &Query<(
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
    )>,
    visiting: &mut HashSet<Entity>,
) -> String {
    if compute_node.params.is_empty() {
        return format!("{function_name}(?)");
    }
    let args = compute_node
        .params
        .iter()
        .map(|param| {
            let Some(entity) = param.1 else {
                return "?".to_string();
            };
            node_expression(entity, node_data, visiting, false)
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!("{function_name}({args})")
}

fn is_infix_operation(operation: &Operation) -> bool {
    matches!(
        operation,
        Operation::Add
            | Operation::Subtract
            | Operation::Multiply
            | Operation::Divide
            | Operation::Power
    )
}