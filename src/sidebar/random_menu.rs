use super::link_params::*;
use super::*;
use crate::constants::*;
use crate::nodes::*;
use bevy::prelude::*;

impl SidebarContent for RandomNode {
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
        add_node_name_field(commands, sidebar_entity, self.name.as_deref());
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

        commands.entity(sidebar_entity).with_child((
            Text::new("Distribution:"),
            text_font(),
            Node {
                margin: px(4).bottom(),
                ..default()
            },
            TextColor(NODE_NAME_COLOR),
        ));

        //spawn context menu
        let context_menu = commands
            .spawn((
                Name::new("distribution_context_menu"),
                Button,
                Node {
                    width: px(SIDEBAR_WIDTH * 0.75),
                    height: px(30),
                    border: UiRect::all(px(5)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::MAX,
                    margin: px(4).bottom(),
                    ..default()
                },
                BorderColor::all(Color::BLACK),
                BackgroundColor(Color::BLACK),
                children![(
                    Pickable::IGNORE,
                    Text::new(self.dist_type.clone()),
                    text_font(),
                    TextColor(Color::WHITE),
                    TextShadow::default(),
                )],
            ))
            .observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
                // by default this event would bubble up further leading to the `CloseContextMenus`
                // event being triggered and undoing the opening of one here right away.
                event.propagate(false);
                println!("Clicked context menu");
                debug!("click: {}", event.pointer_location.position);

                commands.trigger(OpenDistributionMenu {
                    pos: event.pointer_location.position,
                });
            })
            .id();

        commands.entity(sidebar_entity).add_child(context_menu);

        commands.entity(sidebar_entity).with_child(divider());

        //HERE is where you need to add context menus per parameter.

        let mut link_labels = get_ents_and_labels(commands, node_data, &finished_links, node);
        apply_observed_scalar_labels(&mut link_labels, node_data, observed_columns);

        for (i, _param) in self.params.iter().enumerate() {
            build_link_param_selector(
                commands,
                link_labels.clone(),
                self.params.clone(),
                i,
                &sidebar_entity,
            );
        }

        commands.entity(sidebar_entity).with_child(divider());
    }
}

fn on_select_distribution(
    event: On<Pointer<Press>>,
    menu_items: Query<&ContextMenuItem>,
    mut commands: Commands,
    selected: Option<Single<(Entity, &Selected)>>,
    mut random_vars: Query<(&mut RandomNode, &Selected)>,
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
            //set distribution of node to new dist... or maybe on apply?
            println!("Selected distribution {}", item.0);
            let (mut random_var, _selected_comp) = random_vars.get_mut(entity).unwrap();
            random_var.dist_type = item.0.clone();
            random_var.params = distribution_params()
                .get(&random_var.dist_type)
                .unwrap()
                .to_vec();
            //buggy line, can panic, need to set to default params on dist change.
            //refresh_var_dist(&mut random_var, &mut commands);

            commands.trigger(CloseContextMenus);
            commands.trigger(ReloadSidebar);
        }
    }
}

pub fn on_open_distribution_menu(event: On<OpenDistributionMenu>, mut commands: Commands) {
    commands.trigger(CloseContextMenus);
    let pos = event.pos;
    debug!("open context menu at: {pos}");

    commands
        .spawn((
            Name::new("distribution selector"),
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
                context_item("Normal"),
                context_item("LogNormal"),
                context_item("Beta"),
                context_item("Gamma"),
                context_item("Exponential"),
                context_item("Uniform")
            ],
        ))
        .observe(on_select_distribution);
}
