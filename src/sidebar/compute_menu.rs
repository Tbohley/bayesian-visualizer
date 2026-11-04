use super::{
    link_params::{apply_observed_scalar_labels, build_link_param_selector, get_ents_and_labels},
    *,
};
use crate::nodes::replace_node_label;
use bevy::prelude::*;

impl SidebarContent for ComputeNode {
    fn build(
        &self,
        mut commands: &mut Commands,
        sidebar_entity: Entity,
        node_data: &Query<(
            Option<&RandomNode>,
            Option<&ScalarNode>,
            Option<&ComputeNode>,
            Option<&VectorNode>,
        )>,
        finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
        node: Entity,
        _observed: bool,
        observed_columns: &std::collections::HashMap<Entity, String>,
    ) {
        commands.entity(sidebar_entity).with_child(divider());
        available_links(
            &mut commands,
            &node_data,
            &finished_links,
            sidebar_entity,
            node,
            observed_columns,
        );
        commands.entity(sidebar_entity).with_child(divider());

        commands
            .entity(sidebar_entity)
            .with_child(dropdown::label("Operation:"));
        let context_menu = commands
            .spawn(dropdown::selector(
                "operation_context_menu",
                self.operation.name(),
            ))
            .observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
                event.propagate(false);
                println!("Clicked context menu");
                debug!("click: {}", event.pointer_location.position);

                commands.trigger(OpenOperationMenu {
                    pos: event.pointer_location.position,
                });
            })
            .id();
        commands.entity(sidebar_entity).add_child(context_menu);
        commands.entity(sidebar_entity).with_child(divider());
        let mut link_labels = get_ents_and_labels(commands, node_data, &finished_links, node);
        apply_observed_scalar_labels(&mut link_labels, node_data, observed_columns);
        for (i, _param) in self.params.iter().enumerate() {
            let row = build_link_param_selector(
                commands,
                link_labels.clone(),
                self.params.clone(),
                i,
                &sidebar_entity,
            );
            if self.operation == Operation::Collect {
                collect_menu::build_input_actions(commands, row, node, i, self.params.len());
            }
        }
        if self.operation == Operation::Collect {
            collect_menu::build_add_input(commands, sidebar_entity, node);
        }
        commands.entity(sidebar_entity).with_child(divider());
    }
}

pub fn on_open_operation_menu(event: On<OpenOperationMenu>, mut commands: Commands) {
    commands.trigger(CloseContextMenus);
    commands
        .spawn((
            Name::new("operation selector"),
            crate::sidebar::dropdown::panel(event.pos),
        ))
        .observe(scroll::scroll_panel)
        .observe(on_select_operation)
        .with_children(|menu| {
            for operation in Operation::ALL.iter().filter(|op| !vector_only(**op)) {
                menu.spawn(context_item(operation.name()));
            }
            menu.spawn(dropdown::section("Vector nodes only"));
            for operation in Operation::ALL.iter().filter(|op| vector_only(**op)) {
                menu.spawn(context_item(operation.name()));
            }
        });
}

fn on_select_operation(
    event: On<Pointer<Press>>,
    menu_items: Query<&ContextMenuItem>,
    mut commands: Commands,
    selected: Option<Single<(Entity, &Selected)>>,
    mut compute_vars: Query<(&mut ComputeNode, &Selected)>,
    labels: Query<(Entity, &NodeLabel, &ChildOf)>,
    inference_results: Option<Res<crate::bevy_to_fugue::InferenceResultResource>>,
) {
    let target = event.original_event_target();
    if let Some(single) = selected {
        let (entity, _selected_comp) = single.into_inner();

        if let Ok(item) = menu_items.get(target) {
            if crate::sidebar::global::graph_change_requires_confirmation(
                &mut commands,
                inference_results.as_deref(),
                Some(event.pointer_location.position),
            ) {
                return;
            }
            //set operation/label of compute node
            println!("Selected operation {}", item.0);
            let (mut compute_var, _selected_comp) = compute_vars.get_mut(entity).unwrap();
            let Some(operation) = Operation::ALL
                .iter()
                .copied()
                .find(|operation| operation.name() == item.0)
            else {
                return;
            };
            compute_var.operation = operation;

            compute_var.params = compute_params(&compute_var.operation);
            replace_node_label(
                &mut commands,
                entity,
                compute_var.operation.symbol(),
                &labels,
            );

            commands.trigger(CloseContextMenus);
            commands.trigger(ReloadSidebar);
        }
    }
}

fn vector_only(operation: Operation) -> bool {
    operation.requires_vector() || operation == Operation::Index
}
