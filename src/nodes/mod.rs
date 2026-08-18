pub mod random_node;
pub mod compute_node;
pub mod scalar_node;
use fugue::Distribution;
pub use random_node::*;
pub use compute_node::*;
pub use scalar_node::*;
use crate::constants::*;
use bevy::prelude::*;
use crate::graph::*;
use crate::sidebar::*;
use crate::ui::*;
use crate::bevy_to_fugue::InferenceResultResource;
use crate::data_vis::{CloseHistogramPanel, OpenHistogramPanel, DEFAULT_HISTOGRAM_BINS};

//on all node entities
#[derive(Component)]
pub struct GraphNode(pub u32);

#[derive(Component)]
pub struct NodeLabel;

#[derive(Component)]
pub struct NodeInterior;

pub enum NodeType{
    Random,
    Compute,
    Scalar
}

#[derive(Component)]
pub struct NodeMode(pub NodeType);

#[derive(Debug, Clone, Copy)]
pub enum Operation{
    Add,
    Subtract,
    Multiply,
    Divide,
    Exponential,
    Logarithm,
    Power,
    Sum,
    Product
}

#[derive(Debug, Clone)]
pub struct ParamValue (pub &'static str, pub Option<Entity>);          //TODO: change from f64 to GraphLink

#[derive(Event)]
pub struct AutofillNextParam {
    pub node: Entity,
    pub linked_node: Entity,
}

#[derive(Event)]
pub struct SetNodeName {
    pub entity: Entity,
    pub name: String,
}

pub fn autofill_next_param(
    event: On<AutofillNextParam>,
    mut random_nodes: Query<&mut RandomNode>,
    mut compute_nodes: Query<&mut ComputeNode>,
) {
    let params = if let Ok(node) = random_nodes.get_mut(event.node) {
        node.into_inner().params.as_mut_slice()
    } else if let Ok(node) = compute_nodes.get_mut(event.node) {
        node.into_inner().params.as_mut_slice()
    } else {
        return;
    };

    if let Some(param) = params.iter_mut().find(|param| param.1.is_none()) {
        param.1 = Some(event.linked_node);
        println!("Autofilled parameter '{}' from completed link", param.0);
    }
}

pub fn set_node_name(
    event: On<SetNodeName>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut random_nodes: Query<
        (&GraphNode, &mut RandomNode, &mut Mesh2d),
        (Without<ScalarNode>, Without<NodeInterior>, Without<SelectedIndicator>),
    >,
    mut scalar_nodes: Query<&mut ScalarNode, Without<RandomNode>>,
    mut interiors: Query<
        (&ChildOf, &mut Mesh2d),
        (With<NodeInterior>, Without<RandomNode>, Without<SelectedIndicator>),
    >,
    mut indicators: Query<
        (&ChildOf, &mut Mesh2d),
        (With<SelectedIndicator>, Without<RandomNode>, Without<NodeInterior>),
    >,
    mut labels: Query<(&ChildOf, &mut Text2d), With<NodeLabel>>,
    plates: Query<&Plate, Without<PlateDraft>>,
) {
    let name = event
        .name
        .chars()
        .take(MAX_NODE_NAME_CHARS)
        .collect::<String>();
    let name = (!name.is_empty()).then_some(name);

    if let Ok((node_id, mut random, mut exterior_mesh)) = random_nodes.get_mut(event.entity) {
        random.name = name;
        let label = random_node_label(&random, node_id.0);
        exterior_mesh.0 = meshes.add(random_node_mesh(RANDOM_NODE_RAD, &label));
        for (child_of, mut mesh) in &mut interiors {
            if child_of.parent() == event.entity {
                mesh.0 = meshes.add(random_node_mesh(
                    RANDOM_NODE_RAD - NODE_BORDER_WEIGHT,
                    &label,
                ));
            }
        }
        for (child_of, mut text) in &mut labels {
            if child_of.parent() == event.entity {
                text.0 = label.clone();
            }
        }
        for (child_of, mut mesh) in &mut indicators {
            if child_of.parent() == event.entity {
                mesh.0 = meshes.add(random_selection_mesh(&label));
            }
        }
        commands.trigger(ReloadSidebar);
        return;
    }

    if let Ok(mut scalar) = scalar_nodes.get_mut(event.entity) {
        scalar.name = name;
        let label = scalar_display_label(event.entity, &scalar, &plates);
        for (child_of, mut text) in &mut labels {
            if child_of.parent() == event.entity {
                text.0 = label.clone();
            }
        }
        commands.trigger(ReloadSidebar);
    }
}

pub fn scalar_display_label(
    entity: Entity,
    scalar: &ScalarNode,
    plates: &Query<&Plate, Without<PlateDraft>>,
) -> String {
    scalar.name.clone().or_else(|| {
        plates.iter().find_map(|plate| {
            plate
                .mapping
                .get(&entity)
                .filter(|column| column.as_str() != "unobserved")
                .cloned()
        })
    }).unwrap_or_else(|| format!("{:.1}", scalar.val))
}

pub trait DistributionDebug<T>: Distribution<T> + std::fmt::Debug {}
impl<T, D: Distribution<T> + std::fmt::Debug> DistributionDebug<T> for D {}

pub trait NodeDisplay{
    fn label(&self) -> String;
}

//on random variable nodes
#[derive(Component)]
pub struct RandomNode{
    pub name: Option<String>,
    pub dist_type: String,
    pub dist: Box<dyn DistributionDebug<f64>>,
    pub params: Vec<ParamValue>
}

impl NodeDisplay for RandomNode{
    fn label(&self) -> String{
        format!["{}{}", match self.name.clone() {
            Some(n) => n + " ~ ",
            None => "var ~ ".to_string()
        }, self.dist_type]
    }
}

#[derive(Component)]
pub struct ComputeNode{
    pub operation: Operation,
    pub params: Vec<ParamValue>
}

#[derive(Component)]
pub struct ScalarNode{
    pub val: f64,
    pub name: Option<String>,
}

#[derive(Component)]
pub struct SelectedIndicator;

#[derive(Component)]
pub struct ObservedNode;

pub fn update_node_observation_colors(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    plates: Query<&Plate, Without<PlateDraft>>,
    mut labels: Query<(&ChildOf, &mut Text2d), With<NodeLabel>>,
    nodes: Query<(
        Entity,
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Has<ObservedNode>,
    ), Or<(With<RandomNode>, With<ScalarNode>)>>,
    mut interiors: Query<
        (&ChildOf, &mut MeshMaterial2d<ColorMaterial>),
        With<NodeInterior>,
    >,
) {
    for (entity, random, scalar, was_observed) in &nodes {
        let is_observed = plates.iter().any(|plate| {
            plate
                .mapping
                .get(&entity)
                .is_some_and(|column| column != "unobserved")
        });

        if random.is_some() && is_observed != was_observed {
            let color = if is_observed {
                RANDOM_NODE_COLOR
            } else {
                CANVAS_COLOR
            };
            for (child_of, mut material) in &mut interiors {
                if child_of.parent() == entity {
                    material.0 = materials.add(color);
                }
            }
        }

        if is_observed != was_observed {
            if is_observed {
                commands.entity(entity).insert(ObservedNode);
            } else {
                commands.entity(entity).remove::<ObservedNode>();
            }
        }

        if let Some(scalar) = scalar {
            let label = scalar.name.clone().or_else(|| {
                plates.iter().find_map(|plate| {
                    plate
                        .mapping
                        .get(&entity)
                        .filter(|column| column.as_str() != "unobserved")
                        .cloned()
                })
            }).unwrap_or_else(|| format!("{:.1}", scalar.val));
            for (child_of, mut text) in &mut labels {
                if child_of.parent() == entity && text.0 != label {
                    text.0 = label.clone();
                }
            }
        }
    }
}

impl NodeDisplay for ScalarNode{
    fn label(&self) -> String{
        self.name.clone().unwrap_or_else(|| format!["{:.2}", self.val])
    }
}

//create a node on canvas
pub fn on_background_click(
    event: On<Pointer<Click>>,
    reduced_view: Res<ReducedView>,
    mut commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    current_nodes: Query<&GraphNode>,
    selected: Option<Single<(Entity, &mut Selected)>>,
    node_mode: Single<&NodeMode>,
    selection_indicators: Query<(Entity, &ChildOf), With<SelectedIndicator>>,
    mut unfinished_links: Query<Entity, With<UnfinishedLink>>,
    plate_drafts: Query<&Plate, With<PlateDraft>>,
) {
    commands.trigger(CloseHistogramPanel);
    // Bevy emits Click before DragEnd when a drag is released. Do not treat
    // a real plate gesture as a normal background click. Tiny pointer jitter
    // remains a click; its undersized draft is removed by DragEnd.
    if plate_drafts
        .iter()
        .any(|plate| plate.bounds.is_substantial())
    {
        return;
    }

    for entity in unfinished_links.iter_mut() {
        commands.entity(entity).despawn();
    }
    if let Some(single) = selected{
        let (entity, _selected_comp) = single.into_inner();
        //deselect currently selected node + close context menus
        commands.entity(entity).remove::<Selected>();
        for (indicator_entity, child_of) in selection_indicators.iter() {
            if child_of.parent() == entity {
                commands.entity(indicator_entity).despawn();
            }
        }
        commands.trigger(CloseContextMenus);
        commands.trigger(ReloadSidebar);
        return;
    }
    if reduced_view.active {
        return;
    }
    let mut node_num = 1;
    //finds the lowest unused node in the least efficient way possible
    while current_nodes.iter().any(|node| node.0 == node_num) { 
        node_num += 1;
    }
    println!("Created node #{}", node_num);

    let loc = event.hit.position.unwrap();

    match node_mode.into_inner().0 {
        NodeType::Random => new_random(&mut commands, loc, node_num, meshes, materials),
        NodeType::Compute => new_compute(&mut commands, loc, node_num, meshes, materials),
        NodeType::Scalar => new_scalar(&mut commands, loc, node_num, meshes, materials)
    };
    
}


//multifunctional: single click to edit a node, shift click two nodes consecutively to create a link
pub fn on_node_click(
    event: On<Pointer<Click>>,
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    mut unfinished_link: Query<(Entity, &mut GraphLink), With<UnfinishedLink>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    transforms: Query<&mut Transform>,
    selected: Option<Single<(Entity, &mut Selected)>>,
    selection_indicators: Query<(Entity, &ChildOf), With<SelectedIndicator>>,
    node_ids: Query<&GraphNode>,
    inference_results: Option<Res<InferenceResultResource>>,
    reduced_view: Res<ReducedView>,
    random_nodes: Query<&RandomNode>,
    compute_nodes: Query<&ComputeNode>,
    scalar_nodes: Query<&ScalarNode>,
){
    //if there is an unfinished GraphLink, complete it.
    if !reduced_view.active && let Ok((unfinished_ent, mut ends)) = unfinished_link.single_mut() {

        commands.entity(unfinished_ent).remove::<UnfinishedLink>();

        //if user tries to create a link from a node to itself
        if ends.from == event.event_target() { 
            commands.entity(unfinished_ent).despawn();
            return;
        }
        let target = event.event_target();
        ends.to = Some(target);
        commands.trigger(AutofillNextParam {
            node: target,
            linked_node: ends.from,
        });
        commands.trigger(ReloadSidebar);
        println!("Completed a GraphLink");

        //add arrow
        let from_radius = endpoint_radius(
            ends.from,
            &random_nodes,
            &compute_nodes,
            &scalar_nodes,
        );
        let to_radius = endpoint_radius(
            target,
            &random_nodes,
            &compute_nodes,
            &scalar_nodes,
        );
        if let Some((arrow_transform, arrow_mesh)) = link_transform_helper(
            &ends,
            &transforms,
            &mut meshes,
            from_radius,
            to_radius,
        ) {
            commands.entity(unfinished_ent).insert((
                arrow_mesh,
                MeshMaterial2d(materials.add(ARROW_COLOR)),
                arrow_transform,
            ));
        }
    //otherwise, create an invisible UnfinishedLink
    }else if !reduced_view.active && input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]){
        commands.spawn((
            GraphLink{
                from: event.event_target(),
                to: None
            },
            UnfinishedLink
        ));
        println!("Created an UnfinishedLink");
    //normal click, select the node
    }else{
        //println!("Node click event");
        if event.duration.as_millis() < 200 && event.count == 1 {
            println!("Selected a node");

            if let Some(single) = selected{
                let (entity, _selected_comp) = single.into_inner();
                //deselect currently selected node
                commands.entity(entity).remove::<Selected>();

                for (indicator_entity, child_of) in selection_indicators.iter() {
                    if child_of.parent() == entity {
                        commands.entity(indicator_entity).despawn();
                    }
                }
                
            }
            //select this node
            let selection_mesh = if let Ok(random) = random_nodes.get(event.event_target()) {
                let node_id = node_ids
                    .get(event.event_target())
                    .map(|node| node.0)
                    .unwrap_or_default();
                random_selection_mesh(&random_node_label(random, node_id))
            } else if scalar_nodes.contains(event.event_target()) {
                selection_indicator(SCALAR_NODE_RAD)
            } else if compute_nodes.contains(event.event_target()) {
                selection_indicator(COMPUTE_NODE_RAD)
            } else {
                selection_indicator(RANDOM_NODE_RAD)
            };
            commands.entity(event.event_target()).insert(
                Selected
            ).with_child((
                    SelectedIndicator,
                    Pickable::IGNORE,
                    Mesh2d(meshes.add(selection_mesh)),
                    MeshMaterial2d(materials.add(SELECTION_INDICATOR_COLOR)),
                    Transform::from_xyz(0.0, 0.0, 1.)));

            commands.trigger(ReloadSidebar);

            if let Ok(node) = node_ids.get(event.event_target()) {
                if inference_results.is_some() {
                    commands.trigger(OpenHistogramPanel {
                        node_id: node.0,
                        bin_count: DEFAULT_HISTOGRAM_BINS,
                    });
                } else {
                    commands.trigger(CloseHistogramPanel);
                }
            }


        }
    }
}
