use std::{
    collections::{HashMap, HashSet},
    error::Error,
};

use super::*;

impl Dataset {
    pub fn from_csv(name: impl Into<String>, bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        let name = name.into();
        let mut reader = csv::Reader::from_reader(bytes);
        let headers: Vec<String> = reader.headers()?.iter().map(String::from).collect();

        let mut data: HashMap<String, Vec<f64>> =
            headers.iter().map(|h| (h.clone(), Vec::new())).collect();

        let mut n = 0;
        for result in reader.records() {
            let record = result?;
            for (i, field) in record.iter().enumerate() {
                let value: f64 = field.parse()?;
                data.get_mut(&headers[i]).unwrap().push(value);
            }
            n += 1;
        }

        Ok(Dataset { n, name, data })
    }
}

impl Plate {
    pub fn build(
        &mut self,
        commands: &mut Commands,
        sidebar_entity: Entity,
        plate_entity: Entity,
        nodes: &Query<
            (
                Entity,
                &GraphNode,
                &Transform,
                Option<&RandomNode>,
                Option<&ScalarNode>,
            ),
            Or<(With<RandomNode>, With<ScalarNode>)>,
        >,
    ) {
        let mut contents = nodes
            .iter()
            .filter(|(_, _, transform, _, _)| {
                self.bounds.contains_point(transform.translation.truncate())
            })
            .collect::<Vec<_>>();
        contents.sort_by_key(|(_, graph_node, _, _, _)| graph_node.0);

        let contained_entities = contents
            .iter()
            .map(|(entity, _, _, _, _)| *entity)
            .collect::<HashSet<_>>();
        self.mapping
            .retain(|entity, _| contained_entities.contains(entity));
        for (entity, _, _, _, _) in &contents {
            self.mapping
                .entry(*entity)
                .or_insert_with(|| "unobserved".to_string());
        }

        commands.entity(sidebar_entity).with_child(divider());

        commands
            .entity(sidebar_entity)
            .with_child(dropdown::label("Dataset:"));

        //spawn context menu
        let context_menu = commands
            .spawn(dropdown::selector("dataset_context_menu", &self.data.name))
            .observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
                event.propagate(false);
                println!("Clicked context menu");
                debug!("click: {}", event.pointer_location.position);

                commands.trigger(OpenDatasetMenu {
                    pos: event.pointer_location.position,
                });
            })
            .id();
        commands.entity(sidebar_entity).add_child(context_menu);
        commands.entity(sidebar_entity).with_child(divider());

        if self.data.data.is_empty() {
            let extent = commands
                .spawn((
                    Node {
                        width: percent(100.),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(1.),
                        margin: px(8.).bottom(),
                        ..default()
                    },
                    children![
                        dropdown::label("Index size"),
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
                            BackgroundColor(dropdown::FIELD_SURFACE),
                            EditableText::new(self.data.n.to_string()),
                            text_font(),
                            TextColor(Color::WHITE),
                            TextLayout::no_wrap(),
                            TextCursorStyle::default(),
                            TabIndex(1),
                            Name::new("plate_extent_textbox"),
                        )
                    ],
                ))
                .id();
            commands.entity(sidebar_entity).add_child(extent);
            commands.entity(sidebar_entity).with_child(divider());
            return;
        }

        for (entity, graph_node, _, random, scalar) in contents {
            let label = match (random, scalar) {
                (Some(random), None) => match &random.name {
                    Some(_) => random.label(),
                    None => format!("var id {} ~ {}", graph_node.0, random.dist_type),
                },
                (None, Some(_)) => format!("scalar {}", graph_node.0),
                _ => continue,
            };
            commands
                .entity(sidebar_entity)
                .with_child(dropdown::label(label));

            let mapping = self
                .mapping
                .get(&entity)
                .expect("contained nodes should have a plate mapping")
                .clone();
            let mapping_menu = commands
                .spawn(dropdown::selector("plate_mapping_context_menu", &mapping))
                .observe(
                    move |mut event: On<Pointer<Press>>, mut commands: Commands| {
                        event.propagate(false);
                        commands.trigger(OpenPlateMappingMenu {
                            pos: event.pointer_location.position,
                            node: entity,
                            plate: plate_entity,
                        });
                    },
                )
                .id();
            commands.entity(sidebar_entity).add_child(mapping_menu);
        }

        commands.entity(sidebar_entity).with_child(divider());
    }
}

pub(super) fn selector_button<'a>(
    commands: &'a mut Commands,
    name: &'static str,
    value: &str,
) -> EntityCommands<'a> {
    commands.spawn(dropdown::button(name, value))
}

fn on_select_dataset(
    event: On<Pointer<Press>>,
    menu_items: Query<&ContextMenuItem>,
    mut commands: Commands,
    selected: Option<Single<Entity, With<Selected>>>,
    mut plates: Query<&mut Plate>,
    datasets: Res<Datasets>,
    inference_results: Option<Res<crate::bevy_to_fugue::InferenceResultResource>>,
) {
    let Some(selected) = selected else {
        return;
    };
    let Ok(item) = menu_items.get(event.original_event_target()) else {
        return;
    };
    let Ok(mut plate) = plates.get_mut(*selected) else {
        return;
    };
    if crate::sidebar::global::graph_change_requires_confirmation(
        &mut commands,
        inference_results.as_deref(),
        Some(event.pointer_location.position),
    ) {
        return;
    }

    if item.0 == "No dataset" {
        plate.data.name = item.0.clone();
        plate.data.data.clear();
        plate
            .mapping
            .values_mut()
            .for_each(|mapping| *mapping = "unobserved".into());
        commands.trigger(CloseContextMenus);
        commands.trigger(ReloadSidebar);
        return;
    }
    let Some(dataset) = datasets
        .datasets
        .iter()
        .find(|dataset| dataset.name == item.0)
    else {
        warn!("Selected dataset '{}' is no longer available", item.0);
        return;
    };

    println!("Selected dataset {}", dataset.name);
    plate.data = dataset.clone();
    for mapping in plate.mapping.values_mut() {
        *mapping = "unobserved".to_string();
    }
    commands.trigger(CloseContextMenus);
    commands.trigger(ReloadSidebar);
}

pub fn on_open_dataset_menu(
    event: On<OpenDatasetMenu>,
    mut commands: Commands,
    datasets: Res<Datasets>,
) {
    commands.trigger(CloseContextMenus);
    let pos = event.pos;
    debug!("open dataset context menu at: {pos}");

    let menu = commands
        .spawn((
            Name::new("dataset selector"),
            crate::sidebar::dropdown::panel(pos),
        ))
        .observe(super::scroll::scroll_panel)
        .observe(on_select_dataset)
        .id();

    commands.entity(menu).with_children(|parent| {
        parent.spawn(context_item("No dataset"));
        for dataset in &datasets.datasets {
            parent.spawn(context_item(&dataset.name));
        }
    });
}

fn on_select_plate_mapping(
    event: On<Pointer<Press>>,
    menu_items: Query<&ContextMenuItem>,
    mut commands: Commands,
    mut plates: Query<&mut Plate>,
    mapping_menus: Query<&PlateMappingMenu>,
    inference_results: Option<Res<crate::bevy_to_fugue::InferenceResultResource>>,
) {
    let Ok(item) = menu_items.get(event.original_event_target()) else {
        return;
    };
    let Ok(mapping_menu) = mapping_menus.get(event.event_target()) else {
        return;
    };
    let Ok(mut plate) = plates.get_mut(mapping_menu.plate) else {
        return;
    };

    if crate::sidebar::global::graph_change_requires_confirmation(
        &mut commands,
        inference_results.as_deref(),
        Some(event.pointer_location.position),
    ) {
        return;
    }

    plate.mapping.insert(mapping_menu.node, item.0.clone());
    commands.trigger(CloseContextMenus);
    commands.trigger(ReloadSidebar);
}

#[derive(Component)]
struct PlateMappingMenu {
    plate: Entity,
    node: Entity,
}

pub fn on_open_plate_mapping_menu(
    event: On<OpenPlateMappingMenu>,
    mut commands: Commands,
    plates: Query<&Plate>,
) {
    commands.trigger(CloseContextMenus);
    let Ok(plate) = plates.get(event.plate) else {
        return;
    };

    let mut columns = plate.data.data.keys().cloned().collect::<Vec<_>>();
    columns.sort();

    let menu = commands
        .spawn((
            Name::new("plate mapping selector"),
            PlateMappingMenu {
                node: event.node,
                plate: event.plate,
            },
            crate::sidebar::dropdown::panel(event.pos),
        ))
        .observe(super::scroll::scroll_panel)
        .observe(on_select_plate_mapping)
        .id();

    commands.entity(menu).with_children(|parent| {
        parent.spawn(context_item("unobserved"));
        for column in columns {
            parent.spawn(context_item(&column));
        }
    });
}

#[cfg(test)]
#[path = "tests/plate_menu.rs"]
mod tests;
