use std::{collections::{HashMap, HashSet}, error::Error, path::Path};

use super::*;

impl Dataset {
    pub fn from_csv<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let mut reader = csv::Reader::from_path(path)?;
        let headers: Vec<String> = reader.headers()?.iter().map(String::from).collect();

        let mut data: HashMap<String, Vec<f64>> = headers
            .iter()
            .map(|h| (h.clone(), Vec::new()))
            .collect();

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

impl Plate{
    pub fn build(
        &mut self,
        commands: &mut Commands,
        sidebar_entity: Entity,
        nodes: &Query<
            (Entity, &GraphNode, &Transform, Option<&RandomNode>, Option<&ScalarNode>),
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

        commands.entity(sidebar_entity).with_child((
            Text::new("Dataset:"),
            text_font(),
            Node {
                margin: px(4).bottom(),
                ..default()
            },
            TextColor(NODE_NAME_COLOR),
        ));

    //spawn context menu
    let context_menu = commands.spawn((
        Name::new("dataset_context_menu"),
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
            Text::new(self.data.name.clone()),
            text_font(),
            TextColor(Color::WHITE),
            TextShadow::default(),
        )],
    )).observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
        event.propagate(false);
        println!("Clicked context menu");
        debug!("click: {}", event.pointer_location.position);

        commands.trigger(OpenDatasetMenu {
            pos: event.pointer_location.position,
        });
    }).id();
    commands.entity(sidebar_entity).add_child(context_menu);
    commands.entity(sidebar_entity).with_child(divider());

        for (entity, graph_node, _, random, scalar) in contents {
            let label = match (random, scalar) {
                (Some(random), None) => match &random.name {
                    Some(_) => random.label(),
                    None => format!("var id {} ~ {}", graph_node.0, random.dist_type),
                },
                (None, Some(_)) => format!("scalar {}", graph_node.0),
                _ => continue,
            };
            commands.entity(sidebar_entity).with_child((
                Text::new(label),
                text_font(),
                Node {
                    margin: px(4).bottom(),
                    ..default()
                },
                TextColor(NODE_NAME_COLOR),
            ));

            let mapping = self
                .mapping
                .get(&entity)
                .expect("contained nodes should have a plate mapping")
                .clone();
            let mapping_menu = selector_button(commands, "plate_mapping_context_menu", &mapping)
                .observe(move |mut event: On<Pointer<Press>>, mut commands: Commands| {
                    event.propagate(false);
                    commands.trigger(OpenPlateMappingMenu {
                        pos: event.pointer_location.position,
                        node: entity,
                    });
                })
                .id();
            commands.entity(sidebar_entity).add_child(mapping_menu);
        }
        
        commands.entity(sidebar_entity).with_child(divider());


    }
}

fn selector_button<'a>(
    commands: &'a mut Commands,
    name: &'static str,
    value: &str,
) -> EntityCommands<'a> {
    commands.spawn((
        Name::new(name),
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
            Text::new(value),
            text_font(),
            TextColor(Color::WHITE),
            TextShadow::default(),
        )],
    ))
}

fn on_select_dataset(
    event: On<Pointer<Press>>,
    menu_items: Query<&ContextMenuItem>,
    mut commands: Commands,
    selected: Option<Single<Entity, With<Selected>>>,
    mut plates: Query<&mut Plate>,
    datasets: Res<Datasets>,
) {
    let Some(selected) = selected else {
        return;
    };
    let Ok(item) = menu_items.get(event.original_event_target()) else {
        return;
    };
    let Some(dataset) = datasets.datasets.iter().find(|dataset| dataset.name == item.0) else {
        warn!("Selected dataset '{}' is no longer available", item.0);
        return;
    };
    let Ok(mut plate) = plates.get_mut(*selected) else {
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
        .observe(on_select_dataset)
        .id();

    commands.entity(menu).with_children(|parent| {
        for dataset in &datasets.datasets {
            parent.spawn(context_item(&dataset.name));
        }
    });
}

fn on_select_plate_mapping(
    event: On<Pointer<Press>>,
    menu_items: Query<&ContextMenuItem>,
    mut commands: Commands,
    selected: Option<Single<Entity, With<Selected>>>,
    mut plates: Query<&mut Plate>,
    mapping_menus: Query<&PlateMappingMenu>,
) {
    let Some(selected) = selected else {
        return;
    };
    let Ok(item) = menu_items.get(event.original_event_target()) else {
        return;
    };
    let Ok(mapping_menu) = mapping_menus.get(event.event_target()) else {
        return;
    };
    let Ok(mut plate) = plates.get_mut(*selected) else {
        return;
    };
    let Some(mapping) = plate.mapping.get_mut(&mapping_menu.node) else {
        return;
    };

    *mapping = item.0.clone();
    commands.trigger(CloseContextMenus);
    commands.trigger(ReloadSidebar);
}

#[derive(Component)]
struct PlateMappingMenu {
    node: Entity,
}

pub fn on_open_plate_mapping_menu(
    event: On<OpenPlateMappingMenu>,
    mut commands: Commands,
    selected_plate: Option<Single<&Plate, With<Selected>>>,
) {
    commands.trigger(CloseContextMenus);
    let Some(plate) = selected_plate else {
        return;
    };

    let mut columns = plate.data.data.keys().cloned().collect::<Vec<_>>();
    columns.sort();

    let menu = commands
        .spawn((
            Name::new("plate mapping selector"),
            PlateMappingMenu { node: event.node },
            ContextMenu,
            ZIndex(999),
            Node {
                position_type: PositionType::Absolute,
                left: px(event.pos.x),
                top: px(event.pos.y),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BorderColor::all(Color::BLACK),
            BackgroundColor(Color::linear_rgb(0.1, 0.1, 0.1)),
        ))
        .observe(on_select_plate_mapping)
        .id();

    commands.entity(menu).with_children(|parent| {
        parent.spawn(context_item("unobserved"));
        for column in columns {
            parent.spawn(context_item(&column));
        }
    });
}
