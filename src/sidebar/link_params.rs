use std::collections::{HashMap, HashSet};

use super::*;
use crate::nodes::*;
use bevy::prelude::*;

pub fn get_ents_and_labels(
    _commands: &mut Commands,
    node_data: &Query<(
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
        Option<&VectorNode>,
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
                    node_expression(link.from, node_data, &mut visiting, false),
                ))
            } else {
                None
            }
        })
        .collect()
}

pub fn apply_observed_scalar_labels(
    link_labels: &mut [(Entity, String)],
    node_data: &Query<(
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
        Option<&VectorNode>,
    )>,
    observed_columns: &HashMap<Entity, String>,
) {
    for (entity, label) in link_labels {
        let unnamed_scalar = node_data
            .get(*entity)
            .is_ok_and(|(_, scalar, _, _)| scalar.is_some_and(|scalar| scalar.name.is_none()));
        if unnamed_scalar {
            if let Some(column) = observed_columns.get(entity) {
                *label = column.clone();
            }
        }
    }
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
) -> Entity {
    let param = params.get(param_num).expect("invalid param_num");

    let param_name = if param.0 == "element" {
        format!("element {}", param_num + 1)
    } else {
        param.0.to_string()
    };
    let param_entity = param.1;

    let param_label = param_entity
        .and_then(|selected_entity| {
            link_labels
                .iter()
                .find(|(ent, _label)| *ent == selected_entity)
                .map(|(_ent, label)| label.clone())
        })
        .unwrap_or_else(|| "Pick a link".to_string());

    let collect = param.0 == "element";
    let context_root = commands
        .spawn((
            Node {
                width: percent(100.),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.,
                row_gap: px(1.),
                padding: if collect { px(8).all() } else { UiRect::ZERO },
                border: if collect { px(2).all() } else { UiRect::ZERO },
                border_radius: BorderRadius::all(px(8)),
                margin: px(8.).bottom(),
                ..default()
            },
            Name::new(format!("param_row_{}", param_name)),
            BackgroundColor(if collect {
                Color::srgb(0.1, 0.1, 0.1)
            } else {
                Color::NONE
            }),
            BorderColor::all(Color::srgb(0.35, 0.35, 0.35)),
        ))
        .id();

    if collect {
        commands.entity(context_root).with_child((
            Text::new(format!("Element {}", param_num + 1)),
            text_font(),
            TextColor(Color::WHITE),
            Node {
                margin: px(1).bottom(),
                padding: px(64).right(),
                ..default()
            },
            Pickable::IGNORE,
        ));
    } else {
        commands
            .entity(context_root)
            .with_child(dropdown::label(param_name.clone()));
    }
    let button = commands
        .spawn(dropdown::selector(
            format!("param_{}_context_menu", param_name),
            &param_label,
        ))
        .observe(
            move |mut event: On<Pointer<Press>>, mut commands: Commands| {
                event.propagate(false);

                commands.trigger(OpenParamLinkMenu {
                    pos: event.pointer_location.position,
                    param_num,
                });
            },
        )
        .id();
    commands.entity(button).insert(Node {
        width: percent(100),
        min_height: px(if collect { 30 } else { 36 }),
        padding: UiRect::axes(px(9), px(if collect { 5 } else { 7 })),
        border: px(1).all(),
        border_radius: BorderRadius::MAX,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::SpaceBetween,
        column_gap: px(6),
        flex_shrink: 0.,
        ..default()
    });
    commands.entity(context_root).add_child(button);

    commands.entity(*sidebar_entity).add_child(context_root);
    context_root
}

pub fn on_open_param_link_menu(
    event: On<OpenParamLinkMenu>,
    mut commands: Commands,
    node_data: Query<(
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
        Option<&VectorNode>,
    )>,
    finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    plates: Query<&Plate>,
    selected: Single<(Entity, &Selected, &GraphNode)>,
) {
    let (node, _selected_comp, _graphnode) = selected.into_inner();

    let mut link_labels = get_ents_and_labels(&mut commands, &node_data, &finished_links, node);
    let observed_columns = plates
        .iter()
        .flat_map(|plate| plate.mapping.iter())
        .filter(|(_, column)| column.as_str() != "unobserved")
        .map(|(entity, column)| (*entity, column.clone()))
        .collect::<HashMap<_, _>>();
    apply_observed_scalar_labels(&mut link_labels, &node_data, &observed_columns);

    commands.trigger(CloseContextMenus);

    let pos = event.pos;

    let menu = commands
        .spawn((
            Name::new("param link selector"),
            crate::sidebar::dropdown::panel(pos),
        ))
        .observe(super::scroll::scroll_panel)
        .observe(on_pick_param)
        .id();

    if link_labels.is_empty() {
        commands
            .entity(menu)
            .with_child((dropdown::item("No available links"), Pickable::IGNORE));
        return;
    }
    for (ent, text) in link_labels.iter() {
        let child = param_context_item(text.clone(), *ent, event.param_num);

        commands.entity(menu).with_child(child);
    }
}

pub fn param_context_item(text: String, entity: Entity, param_num: usize) -> impl Bundle {
    (
        Name::new(format!("item-{text}")),
        ParamMenuItem {
            label: text.clone(),
            entity,
            param_num,
        },
        dropdown::item(&text),
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
    inference_results: Option<Res<crate::bevy_to_fugue::InferenceResultResource>>,
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
    if crate::sidebar::global::graph_change_requires_confirmation(
        &mut commands,
        inference_results.as_deref(),
        Some(event.pointer_location.position),
    ) {
        return;
    }
    // Try RandomNode params first.
    if let Ok(mut random_node) = random_nodes.get_mut(selected_entity) {
        if let Some(param) = random_node.params.get_mut(item.param_num) {
            param.1 = Some(item.entity);

            println!("Set RandomNode param '{}' to {}", param.0, item.label,);

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
            println!("Set ComputeNode param '{}' to {}", param.0, item.label,);
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
        Option<&VectorNode>,
    )>,
    visiting: &mut HashSet<Entity>,
    nested_inside_operation: bool,
) -> String {
    if !visiting.insert(entity) {
        return "?".to_string();
    }

    let expression = match node_data.get(entity) {
        Ok((random_node, scalar_node, compute_node, vector_node)) => {
            if let Some(random_node) = random_node {
                random_short_label(random_node)
            } else if let Some(scalar_node) = scalar_node {
                scalar_label(scalar_node)
            } else if let Some(vector_node) = vector_node {
                vector_node.label()
            } else if let Some(compute_node) = compute_node {
                compute_expression(compute_node, node_data, visiting, nested_inside_operation)
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
    random_node.name.clone().unwrap_or_else(|| format!("var"))
}

fn scalar_label(scalar_node: &ScalarNode) -> String {
    scalar_node
        .name
        .clone()
        .unwrap_or_else(|| format_number(scalar_node.val))
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
        Option<&VectorNode>,
    )>,
    visiting: &mut HashSet<Entity>,
    nested_inside_operation: bool,
) -> String {
    let expression = match &compute_node.operation {
        Operation::Add => infix_expression(compute_node, " + ", node_data, visiting),
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
        Operation::Logit => compact_expression("logit", compute_node, node_data, visiting),
        Operation::InverseLogit => compact_expression("sigmoid", compute_node, node_data, visiting),
        Operation::Normalize => compact_expression("normalize", compute_node, node_data, visiting),
        Operation::Softmax => compact_expression("softmax", compute_node, node_data, visiting),
        Operation::Index => compact_expression("at", compute_node, node_data, visiting),
        Operation::Collect => compact_expression("vector", compute_node, node_data, visiting),
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
        Option<&VectorNode>,
    )>,
    visiting: &mut HashSet<Entity>,
) -> String {
    let left = infix_arg_expression(compute_node, 0, node_data, visiting);
    let right = infix_arg_expression(compute_node, 1, node_data, visiting);
    format!("{left}{operator}{right}")
}

fn infix_arg_expression(
    compute_node: &ComputeNode,
    param_index: usize,
    node_data: &Query<(
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
        Option<&VectorNode>,
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
        Option<&VectorNode>,
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
        Option<&VectorNode>,
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
