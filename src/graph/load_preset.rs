use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use fugue::Normal;

use super::*;
use crate::bevy_to_fugue::{GraphIRResource, InferenceResultResource, SamplePopup};
use crate::constants::*;
use crate::data_vis::{CloseHistogramPanel, InferenceHistogramPanel};
use crate::nodes::*;
use crate::sidebar::{
    CloseContextMenus, ContextMenu, Datasets, LocalSidebar, ReloadSidebar,
    SetInferenceControlsEnabled, SetPosteriorSampleEnabled,
};
use crate::ui::{ErrorToast, ErrorToastBox};

const NO_DATASET: &str = "No dataset";

/// A bundled, declarative graph description. Node references use stable IDs,
/// never Bevy entities, so catalog entries remain reproducible across runs.
pub struct GraphPreset {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub study_condition: Option<&'static str>,
    pub nodes: Vec<PresetNode>,
    pub plates: Vec<PresetPlate>,
}

#[allow(dead_code)] // The bundled catalog intentionally starts empty.
pub enum PresetNode {
    Random {
        id: u32,
        position: [f32; 2],
        name: Option<&'static str>,
        distribution: &'static str,
        parameters: Vec<PresetParameter>,
    },
    Compute {
        id: u32,
        position: [f32; 2],
        operation: Operation,
        parameters: Vec<PresetParameter>,
    },
    Scalar {
        id: u32,
        position: [f32; 2],
        value: f64,
    },
}

impl PresetNode {
    fn id(&self) -> u32 {
        match self {
            Self::Random { id, .. } | Self::Compute { id, .. } | Self::Scalar { id, .. } => *id,
        }
    }

    fn position(&self) -> [f32; 2] {
        match self {
            Self::Random { position, .. }
            | Self::Compute { position, .. }
            | Self::Scalar { position, .. } => *position,
        }
    }

    fn parameters(&self) -> &[PresetParameter] {
        match self {
            Self::Random { parameters, .. } | Self::Compute { parameters, .. } => parameters,
            Self::Scalar { .. } => &[],
        }
    }
}

pub struct PresetParameter {
    pub name: &'static str,
    pub source: Option<u32>,
}

pub struct PresetPlate {
    pub id: u32,
    pub bounds: PresetBounds,
    pub dataset_id: &'static str,
    pub mapping: Vec<PresetMapping>,
}

pub struct PresetBounds {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

pub struct PresetMapping {
    pub node: u32,
    pub column: &'static str,
}

/// Add study presets to this vector. Pressing Compile prints the current graph
/// in the exact `GraphPreset { ... }` form accepted here.
pub fn bundled_presets() -> Vec<GraphPreset> {
    vec![GraphPreset {
        id: "lin_reg",
        title: "Linear Regression",
        description: "Simple linear regression between SAT scores and GPA",
        study_condition: None,
        nodes: vec![
            PresetNode::Random { id: 2, position: [-75.14453, 49.648426], name: Some("x"), distribution: "Normal", parameters: vec![PresetParameter { name: "mean", source: Some(10) }, PresetParameter { name: "std_dev", source: Some(11) }] },
            PresetNode::Random { id: 3, position: [244.4922, 50.23828], name: Some("y"), distribution: "Normal", parameters: vec![PresetParameter { name: "mean", source: Some(8) }, PresetParameter { name: "std_dev", source: Some(4) }] },
            PresetNode::Random { id: 4, position: [252.98438, 173.88278], name: Some("s"), distribution: "LogNormal", parameters: vec![PresetParameter { name: "mean", source: Some(9) }, PresetParameter { name: "std_dev", source: Some(17) }] },
            PresetNode::Random { id: 5, position: [-106.093765, 187.28903], name: Some("a"), distribution: "Normal", parameters: vec![PresetParameter { name: "mean", source: Some(12) }, PresetParameter { name: "std_dev", source: Some(13) }] },
            PresetNode::Random { id: 6, position: [99.6016, 204.73828], name: Some("b"), distribution: "Normal", parameters: vec![PresetParameter { name: "mean", source: Some(14) }, PresetParameter { name: "std_dev", source: Some(15) }] },
            PresetNode::Compute { id: 7, position: [1.957016, 48.085926], operation: Operation::Multiply, parameters: vec![PresetParameter { name: "first", source: Some(5) }, PresetParameter { name: "second", source: Some(2) }] },
            PresetNode::Compute { id: 8, position: [109.83594, 43.75391], operation: Operation::Add, parameters: vec![PresetParameter { name: "first", source: Some(6) }, PresetParameter { name: "second", source: Some(7) }] },
            PresetNode::Compute { id: 9, position: [241.4375, 251.81248], operation: Operation::Logarithm, parameters: vec![PresetParameter { name: "input", source: Some(16) }] },
            PresetNode::Scalar { id: 10, position: [-186.27344, 81.90624], value: 2.0 },
            PresetNode::Scalar { id: 11, position: [-181.6836, 9.242197], value: 1.0 },
            PresetNode::Scalar { id: 12, position: [-182.0586, 277.2695], value: 500.0 },
            PresetNode::Scalar { id: 13, position: [-54.433563, 279.99606], value: 200.0 },
            PresetNode::Scalar { id: 14, position: [56.71095, 256.05075], value: 800.0 },
            PresetNode::Scalar { id: 15, position: [155.64842, 252.37497], value: 300.0 },
            PresetNode::Scalar { id: 16, position: [226.14844, 315.80856], value: 80.0 },
            PresetNode::Scalar { id: 17, position: [315.4297, 192.332], value: 1.0 }
        ],
        plates: vec![
            PresetPlate { id: 1, bounds: PresetBounds { min: [-125.51563, -2.8125076], max: [323.51953, 107.65624] }, dataset_id: "SATandGPA.csv", mapping: vec![PresetMapping { node: 2, column: "GPA" }, PresetMapping { node: 3, column: "SAT" }] }
        ],
    },
    
    GraphPreset {
        id: "poly_reg",
        title: "Quadratic Regression",
        description: "Replace me",
        study_condition: None,
        nodes: vec![
            PresetNode::Random { id: 2, position: [-87.89453, 265.24216], name: Some("a"), distribution: "Normal", parameters: vec![PresetParameter { name: "mean", source: Some(14) }, PresetParameter { name: "std_dev", source: Some(15) }] },
            PresetNode::Random { id: 3, position: [-12.3828125, 269.91794], name: Some("b"), distribution: "Normal", parameters: vec![PresetParameter { name: "mean", source: Some(14) }, PresetParameter { name: "std_dev", source: Some(15) }] },
            PresetNode::Random { id: 4, position: [118.75778, 273.32813], name: Some("c"), distribution: "Normal", parameters: vec![PresetParameter { name: "mean", source: Some(14) }, PresetParameter { name: "std_dev", source: Some(15) }] },
            PresetNode::Random { id: 5, position: [199.23828, 257.17575], name: Some("s"), distribution: "LogNormal", parameters: vec![PresetParameter { name: "mean", source: Some(17) }, PresetParameter { name: "std_dev", source: Some(16) }] },
            PresetNode::Random { id: 6, position: [-131.46484, -0.30860138], name: Some("x"), distribution: "Normal", parameters: vec![PresetParameter { name: "mean", source: Some(12) }, PresetParameter { name: "std_dev", source: Some(13) }] },
            PresetNode::Random { id: 7, position: [212.51172, 87.63281], name: Some("y"), distribution: "Normal", parameters: vec![PresetParameter { name: "mean", source: Some(11) }, PresetParameter { name: "std_dev", source: Some(5) }] },
            PresetNode::Compute { id: 8, position: [-138.64063, 129.58594], operation: Operation::Power, parameters: vec![PresetParameter { name: "base", source: Some(6) }, PresetParameter { name: "exponent", source: Some(18) }] },
            PresetNode::Compute { id: 9, position: [-78.57812, 133.83202], operation: Operation::Multiply, parameters: vec![PresetParameter { name: "first", source: Some(8) }, PresetParameter { name: "second", source: Some(2) }] },
            PresetNode::Compute { id: 10, position: [-0.417984, -1.2109375], operation: Operation::Multiply, parameters: vec![PresetParameter { name: "first", source: Some(6) }, PresetParameter { name: "second", source: Some(3) }] },
            PresetNode::Compute { id: 11, position: [126.77736, 87.031235], operation: Operation::Add, parameters: vec![PresetParameter { name: "first", source: Some(19) }, PresetParameter { name: "second", source: Some(4) }] },
            PresetNode::Scalar { id: 12, position: [-238.10156, 19.761707], value: 0.0 },
            PresetNode::Scalar { id: 13, position: [-237.98047, -39.82811], value: 1.5 },
            PresetNode::Scalar { id: 14, position: [-65.402336, 338.67575], value: 0.0 },
            PresetNode::Scalar { id: 15, position: [-9.644539, 334.3984], value: 5.0 },
            PresetNode::Scalar { id: 16, position: [190.13672, 321.22653], value: 5.0 },
            PresetNode::Scalar { id: 17, position: [145.04297, 312.98434], value: 0.0 },
            PresetNode::Scalar { id: 18, position: [-236.3086, 127.69531], value: 2.0 },
            PresetNode::Compute { id: 19, position: [58.835907, 86.98436], operation: Operation::Add, parameters: vec![PresetParameter { name: "first", source: Some(10) }, PresetParameter { name: "second", source: Some(9) }] }
        ],
        plates: vec![
            PresetPlate { id: 1, bounds: PresetBounds { min: [-182.67578, -38.87111], max: [271.9453, 191.82811] }, dataset_id: "poly_reg.csv", mapping: vec![PresetMapping { node: 6, column: "sample_x" }, PresetMapping { node: 7, column: "sample_y" }] }
        ],
    }]
}

#[derive(Event)]
pub struct OpenPresetMenu {
    pub pos: Vec2,
}

#[derive(Clone)]
enum PresetTarget {
    Preset(String),
    Clear,
}

#[derive(Component)]
struct PresetMenuItem(PresetTarget);

#[derive(Component)]
struct PresetConfirmation(PresetTarget);

#[derive(Component)]
enum ConfirmationChoice {
    Confirm,
    Cancel,
}

#[derive(Event)]
pub struct RequestPresetConfirmation {
    target: PresetTarget,
    pos: Vec2,
}

#[derive(Event)]
pub struct LoadPreset(PresetTarget);

pub fn on_open_preset_menu(event: On<OpenPresetMenu>, mut commands: Commands) {
    commands.trigger(CloseContextMenus);
    let menu = commands
        .spawn((
            Name::new("preset selector"),
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
        .observe(on_choose_preset)
        .id();

    commands.entity(menu).with_children(|parent| {
        for preset in bundled_presets() {
            parent.spawn((
                PresetMenuItem(PresetTarget::Preset(preset.id.to_string())),
                menu_item(preset.title),
            ));
        }
        parent.spawn((
            PresetMenuItem(PresetTarget::Clear),
            menu_item("Clear canvas"),
        ));
    });
}

fn on_choose_preset(
    mut event: On<Pointer<Press>>,
    mut commands: Commands,
    choices: Query<&PresetMenuItem>,
) {
    let Ok(choice) = choices.get(event.original_event_target()) else {
        return;
    };
    event.propagate(false);
    commands.trigger(CloseContextMenus);
    commands.trigger(RequestPresetConfirmation {
        target: choice.0.clone(),
        pos: event.pointer_location.position,
    });
}

pub fn on_request_preset_confirmation(
    event: On<RequestPresetConfirmation>,
    mut commands: Commands,
) {
    let (prompt, action) = match &event.target {
        PresetTarget::Preset(id) => {
            let title = bundled_presets()
                .into_iter()
                .find(|preset| preset.id == id)
                .map(|preset| preset.title)
                .unwrap_or("this preset");
            (format!("Replace graph with {title}?"), "Load")
        }
        PresetTarget::Clear => ("Clear the current graph?".to_string(), "Clear"),
    };
    let menu = commands
        .spawn((
            Name::new("preset confirmation"),
            PresetConfirmation(event.target.clone()),
            ContextMenu,
            ZIndex(1000),
            Node {
                position_type: PositionType::Absolute,
                left: px(event.pos.x),
                top: px(event.pos.y),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(4)),
                padding: px(5).all(),
                ..default()
            },
            BorderColor::all(Color::BLACK),
            BackgroundColor(Color::linear_rgb(0.1, 0.1, 0.1)),
        ))
        .observe(on_confirm_preset)
        .id();
    commands.entity(menu).with_children(|parent| {
        parent.spawn((
            Pickable::IGNORE,
            Text::new(prompt),
            TextColor(Color::WHITE),
            Node {
                padding: px(5).all(),
                ..default()
            },
        ));
        parent.spawn((ConfirmationChoice::Confirm, menu_item(action)));
        parent.spawn((ConfirmationChoice::Cancel, menu_item("Cancel")));
    });
}

fn on_confirm_preset(
    mut event: On<Pointer<Press>>,
    mut commands: Commands,
    choices: Query<&ConfirmationChoice>,
    confirmations: Query<&PresetConfirmation>,
) {
    let Ok(choice) = choices.get(event.original_event_target()) else {
        return;
    };
    event.propagate(false);
    let Ok(confirmation) = confirmations.get(event.event_target()) else {
        return;
    };
    let target = confirmation.0.clone();
    commands.trigger(CloseContextMenus);
    if matches!(choice, ConfirmationChoice::Confirm) {
        commands.trigger(LoadPreset(target));
    }
}

fn menu_item(text: &str) -> impl Bundle {
    (
        Button,
        Node {
            padding: px(5).all(),
            ..default()
        },
        children![(
            Pickable::IGNORE,
            Text::new(text.to_string()),
            TextColor(Color::WHITE),
        )],
    )
}

#[allow(clippy::too_many_arguments)]
pub fn on_load_preset(
    event: On<LoadPreset>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    datasets: Res<Datasets>,
    graph_nodes: Query<Entity, With<GraphNode>>,
    links: Query<Entity, With<GraphLink>>,
    drafts: Query<Entity, With<PlateDraft>>,
    local_sidebars: Query<Entity, With<LocalSidebar>>,
    sample_popups: Query<Entity, With<SamplePopup>>,
    histogram_panels: Query<Entity, With<InferenceHistogramPanel>>,
    error_toasts: Query<Entity, With<ErrorToastBox>>,
) {
    let preset = match &event.0 {
        PresetTarget::Preset(id) => {
            let Some(preset) = bundled_presets().into_iter().find(|preset| preset.id == id) else {
                commands.trigger(ErrorToast {
                    text: format!("Preset '{id}' is no longer available."),
                    color: ERR_COLOR,
                });
                return;
            };
            if let Err(error) = validate_preset(&preset, &datasets) {
                commands.trigger(ErrorToast {
                    text: format!("Could not load preset '{}': {error}", preset.title),
                    color: ERR_COLOR,
                });
                return;
            }
            Some(preset)
        }
        PresetTarget::Clear => None,
    };

    clear_editable_graph(
        &mut commands,
        &graph_nodes,
        &links,
        &drafts,
        &local_sidebars,
        &sample_popups,
        &histogram_panels,
        &error_toasts,
    );

    if let Some(preset) = preset {
        spawn_preset(
            &preset,
            &mut commands,
            &mut meshes,
            &mut materials,
            &datasets,
        );
        println!(
            "Loaded preset '{}' ({}) — {} [study condition: {}]",
            preset.title,
            preset.id,
            preset.description,
            preset.study_condition.unwrap_or("none"),
        );
    } else {
        println!("Cleared graph");
    }
    commands.trigger(ReloadSidebar);
}

#[allow(clippy::too_many_arguments)]
fn clear_editable_graph(
    commands: &mut Commands,
    graph_nodes: &Query<Entity, With<GraphNode>>,
    links: &Query<Entity, With<GraphLink>>,
    drafts: &Query<Entity, With<PlateDraft>>,
    local_sidebars: &Query<Entity, With<LocalSidebar>>,
    sample_popups: &Query<Entity, With<SamplePopup>>,
    histogram_panels: &Query<Entity, With<InferenceHistogramPanel>>,
    error_toasts: &Query<Entity, With<ErrorToastBox>>,
) {
    let mut entities = HashSet::new();
    entities.extend(graph_nodes.iter());
    entities.extend(links.iter());
    entities.extend(drafts.iter());
    entities.extend(local_sidebars.iter());
    entities.extend(sample_popups.iter());
    entities.extend(histogram_panels.iter());
    entities.extend(error_toasts.iter());
    for entity in entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<GraphIRResource>();
    commands.remove_resource::<InferenceResultResource>();
    commands.trigger(SetInferenceControlsEnabled(false));
    commands.trigger(SetPosteriorSampleEnabled(false));
    commands.trigger(CloseHistogramPanel);
}

fn validate_preset(preset: &GraphPreset, datasets: &Datasets) -> Result<(), String> {
    let mut ids = HashSet::new();
    for node in &preset.nodes {
        if !ids.insert(node.id()) {
            return Err(format!("duplicate graph ID {}", node.id()));
        }
        let [x, y] = node.position();
        if !x.is_finite() || !y.is_finite() {
            return Err(format!("node {} has a non-finite position", node.id()));
        }
        if let PresetNode::Scalar { value, .. } = node
            && !value.is_finite()
        {
            return Err(format!("node {} has a non-finite scalar value", node.id()));
        }
    }
    for plate in &preset.plates {
        if !ids.insert(plate.id) {
            return Err(format!("duplicate graph ID {}", plate.id));
        }
        let bounds = preset_bounds(plate);
        if !bounds.min.is_finite() || !bounds.max.is_finite() || !bounds.is_substantial() {
            return Err(format!("plate {} has invalid bounds", plate.id));
        }
        if plate.dataset_id != NO_DATASET
            && !datasets
                .datasets
                .iter()
                .any(|dataset| dataset.name == plate.dataset_id)
        {
            return Err(format!("dataset '{}' was not found", plate.dataset_id));
        }
        for mapping in &plate.mapping {
            if !preset.nodes.iter().any(|node| node.id() == mapping.node) {
                return Err(format!(
                    "plate {} maps missing node {}",
                    plate.id, mapping.node
                ));
            }
        }
    }
    for node in &preset.nodes {
        for parameter in node.parameters() {
            if let Some(source) = parameter.source
                && !preset.nodes.iter().any(|node| node.id() == source)
            {
                return Err(format!(
                    "node {} references missing node {source}",
                    node.id()
                ));
            }
        }
    }
    Ok(())
}

fn spawn_preset(
    preset: &GraphPreset,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    datasets: &Datasets,
) {
    let mut entities = HashMap::new();
    let mut positions = HashMap::new();

    // Pass one: create every node, independent of dependency order.
    for node in &preset.nodes {
        let [x, y] = node.position();
        let position = Vec3::new(x, y, 1.0);
        let entity = match node {
            PresetNode::Random {
                id,
                name,
                distribution,
                ..
            } => spawn_random(
                commands,
                position,
                *id,
                RandomNode {
                    name: name.map(str::to_string),
                    dist_type: (*distribution).to_string(),
                    dist: Box::new(Normal::new(0.0, 1.0).expect("valid placeholder distribution")),
                    params: Vec::new(),
                },
                meshes,
                materials,
            ),
            PresetNode::Compute { id, operation, .. } => spawn_compute(
                commands,
                position,
                *id,
                ComputeNode {
                    operation: *operation,
                    params: Vec::new(),
                },
                meshes,
                materials,
            ),
            PresetNode::Scalar { id, value, .. } => spawn_scalar(
                commands,
                position,
                *id,
                ScalarNode { val: *value },
                meshes,
                materials,
            ),
        };
        entities.insert(node.id(), entity);
        positions.insert(entity, position);
    }

    // Pass two: resolve stable IDs into entity-valued parameters and links.
    let mut link_pairs = HashSet::new();
    for node in &preset.nodes {
        let target = entities[&node.id()];
        let params = node
            .parameters()
            .iter()
            .map(|parameter| {
                let source = parameter.source.map(|id| entities[&id]);
                if let Some(source) = source {
                    link_pairs.insert((source, target));
                }
                ParamValue(parameter.name, source)
            })
            .collect::<Vec<_>>();
        match node {
            PresetNode::Random { .. } => {
                commands
                    .entity(target)
                    .entry::<RandomNode>()
                    .and_modify(move |mut random| {
                        random.params = params;
                    });
            }
            PresetNode::Compute { .. } => {
                commands
                    .entity(target)
                    .entry::<ComputeNode>()
                    .and_modify(move |mut compute| {
                        compute.params = params;
                    });
            }
            PresetNode::Scalar { .. } => {}
        }
    }
    for (from, to) in link_pairs {
        spawn_finished_link(
            commands,
            from,
            to,
            positions[&from],
            positions[&to],
            meshes,
            materials,
        );
    }
    for plate in &preset.plates {
        load_plate(plate, commands, meshes, materials, datasets, &entities);
    }
}

fn operation_variant(operation: &Operation) -> &'static str {
    match operation {
        Operation::Add => "Add",
        Operation::Subtract => "Subtract",
        Operation::Multiply => "Multiply",
        Operation::Divide => "Divide",
        Operation::Exponential => "Exponential",
        Operation::Logarithm => "Logarithm",
        Operation::Power => "Power",
        Operation::Sum => "Sum",
        Operation::Product => "Product",
    }
}

fn load_plate(
    preset: &PresetPlate,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    datasets: &Datasets,
    entities: &HashMap<u32, Entity>,
) {
    let bounds = preset_bounds(preset);
    let data = datasets
        .datasets
        .iter()
        .find(|dataset| dataset.name == preset.dataset_id)
        .cloned()
        .unwrap_or_else(|| Dataset {
            name: NO_DATASET.to_string(),
            n: 10,
            data: HashMap::new(),
        });
    let mapping = preset
        .mapping
        .iter()
        .map(|mapping| (entities[&mapping.node], mapping.column.to_string()))
        .collect();
    spawn_completed_plate(
        commands,
        preset.id,
        Plate {
            origin: bounds.center(),
            bounds,
            data,
            mapping,
        },
        meshes,
        materials,
    );
}

fn preset_bounds(plate: &PresetPlate) -> PlateBounds {
    PlateBounds::from_points(
        Vec2::from_array(plate.bounds.min),
        Vec2::from_array(plate.bounds.max),
    )
}

pub fn print_graph_preset(
    random_nodes: &Query<(Entity, &RandomNode), (Without<ComputeNode>, Without<ScalarNode>)>,
    compute_nodes: &Query<(Entity, &ComputeNode), (Without<RandomNode>, Without<ScalarNode>)>,
    scalar_nodes: &Query<(Entity, &ScalarNode), (Without<RandomNode>, Without<ComputeNode>)>,
    node_ids: &Query<(Entity, &GraphNode)>,
    node_positions: &Query<(&GraphNode, &Transform), Without<Plate>>,
    plates: &Query<(&GraphNode, &Plate)>,
) {
    let entity_ids = node_ids
        .iter()
        .map(|(entity, id)| (entity, id.0))
        .collect::<HashMap<_, _>>();
    let positions = node_positions
        .iter()
        .map(|(id, transform)| (id.0, transform.translation.truncate()))
        .collect::<HashMap<_, _>>();
    let parameter_text = |params: &[ParamValue]| {
        params
            .iter()
            .map(|parameter| {
                let source = parameter
                    .1
                    .and_then(|entity| entity_ids.get(&entity).copied());
                format!(
                    "PresetParameter {{ name: {:?}, source: {:?} }}",
                    parameter.0, source
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut nodes = Vec::new();
    for (entity, random) in random_nodes {
        let Some(&id) = entity_ids.get(&entity) else {
            continue;
        };
        let position = positions.get(&id).copied().unwrap_or(Vec2::ZERO);
        nodes.push((id, format!(
            "        PresetNode::Random {{ id: {id}, position: [{:?}, {:?}], name: {:?}, distribution: {:?}, parameters: vec![{}] }}",
            position.x, position.y, random.name.as_deref(), random.dist_type, parameter_text(&random.params)
        )));
    }
    for (entity, compute) in compute_nodes {
        let Some(&id) = entity_ids.get(&entity) else {
            continue;
        };
        let position = positions.get(&id).copied().unwrap_or(Vec2::ZERO);
        nodes.push((id, format!(
            "        PresetNode::Compute {{ id: {id}, position: [{:?}, {:?}], operation: Operation::{}, parameters: vec![{}] }}",
            position.x, position.y, operation_variant(&compute.operation), parameter_text(&compute.params)
        )));
    }
    for (entity, scalar) in scalar_nodes {
        let Some(&id) = entity_ids.get(&entity) else {
            continue;
        };
        let position = positions.get(&id).copied().unwrap_or(Vec2::ZERO);
        nodes.push((
            id,
            format!(
                "        PresetNode::Scalar {{ id: {id}, position: [{:?}, {:?}], value: {:?} }}",
                position.x, position.y, scalar.val
            ),
        ));
    }
    nodes.sort_by_key(|(id, _)| *id);

    let mut plate_text = plates.iter().map(|(id, plate)| {
        let mut mapping = plate.mapping.iter().filter_map(|(entity, column)| {
            entity_ids.get(entity).map(|node| (*node, format!("PresetMapping {{ node: {node}, column: {column:?} }}")))
        }).collect::<Vec<_>>();
        mapping.sort_by_key(|(id, _)| *id);
        (id.0, format!(
            "        PresetPlate {{ id: {}, bounds: PresetBounds {{ min: [{:?}, {:?}], max: [{:?}, {:?}] }}, dataset_id: {:?}, mapping: vec![{}] }}",
            id.0, plate.bounds.min.x, plate.bounds.min.y, plate.bounds.max.x, plate.bounds.max.y,
            plate.data.name, mapping.into_iter().map(|(_, text)| text).collect::<Vec<_>>().join(", ")
        ))
    }).collect::<Vec<_>>();
    plate_text.sort_by_key(|(id, _)| *id);

    println!(
        "Copy into bundled_presets():\nGraphPreset {{\n    id: \"replace_me\",\n    title: \"Replace me\",\n    description: \"Replace me\",\n    study_condition: None,\n    nodes: vec![\n{}\n    ],\n    plates: vec![\n{}\n    ],\n}},",
        nodes
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            .join(",\n"),
        plate_text
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            .join(",\n"),
    );
}
