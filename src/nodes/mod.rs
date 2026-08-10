pub mod random_node;
pub mod compute_node;
pub mod scalar_node;
use fugue::Distribution;
use rand::thread_rng;
pub use random_node::*;
pub use compute_node::*;
pub use scalar_node::*;
use crate::constants::*;
use bevy::prelude::*;
use crate::graph::*;
use crate::sidebar::*;
use crate::ui::*;
use crate::bevy_to_fugue::InferenceResultResource;
use crate::data_vis::{CloseHistogramPanel, OpenHistogramPanel};

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
    pub val: f64
}

#[derive(Component)]
pub struct SelectedIndicator;

#[derive(Component)]
pub struct ObservedNode;

pub fn update_node_observation_colors(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    plates: Query<&Plate, Without<PlateDraft>>,
    labels: Query<(Entity, &NodeLabel, &ChildOf)>,
    nodes: Query<(
        Entity,
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Has<ObservedNode>,
    ), With<RandomNode>>,
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

        if is_observed == was_observed {
            continue;
        }

        let color = match (random, scalar, is_observed) {
            (Some(_), None, true) => RANDOM_NODE_COLOR,
            (None, Some(_), true) => SCALAR_NODE_COLOR,
            (Some(_), None, false) | (None, Some(_), false) => CANVAS_COLOR,
            _ => continue,
        };
        for (child_of, mut material) in &mut interiors {
            if child_of.parent() == entity {
                material.0 = materials.add(color);
            }
        }

        if is_observed {
            commands.entity(entity).insert(ObservedNode);
            if scalar.is_some() {
                for (label_entity, _, child_of) in &labels {
                    if child_of.parent() == entity {
                        commands.entity(label_entity).despawn();
                    }
                }
            }
        } else {
            commands.entity(entity).remove::<ObservedNode>();
            if let Some(scalar) = scalar {
                replace_node_label(
                    &mut commands,
                    entity,
                    format!("{:.1}", scalar.val),
                    &labels,
                    None,
                );
            }
        }
    }
}

impl NodeDisplay for ScalarNode{
    fn label(&self) -> String{
        format!["{:.2}", self.val]
    }
}

//create a node on canvas
pub fn on_background_click(
    event: On<Pointer<Click>>,
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
    }
    
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
){
    //if there is an unfinished GraphLink, complete it.
    if let Ok((unfinished_ent, mut ends)) = unfinished_link.single_mut() {

        commands.entity(unfinished_ent).remove::<UnfinishedLink>();

        //if user tries to create a link from a node to itself
        if ends.from == event.event_target() { 
            commands.entity(unfinished_ent).despawn();
            return;
        }
        ends.to = Some(event.event_target());
        commands.trigger(ReloadSidebar);
        println!("Completed a GraphLink");

        //add arrow
        if let Some((arrow_transform, arrow_mesh)) = link_transform_helper(&ends, &transforms, &mut meshes) {
            commands.entity(unfinished_ent).insert((
                arrow_mesh,
                MeshMaterial2d(materials.add(ARROW_COLOR)),
                arrow_transform,
            ));
        }
    //otherwise, create an invisible UnfinishedLink
    }else if input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]){ 
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
            commands.entity(event.event_target()).insert(
                Selected
            ).with_child((
                    SelectedIndicator,
                    Pickable::IGNORE,
                    Mesh2d(meshes.add(selection_indicator(RANDOM_NODE_RAD))),
                    MeshMaterial2d(materials.add(SELECTION_INDICATOR_COLOR)),
                    Transform::from_xyz(0.0, 0.0, 1.)));

            commands.trigger(ReloadSidebar);

            if let Ok(node) = node_ids.get(event.event_target()) {
                if inference_results.is_some() {
                    commands.trigger(OpenHistogramPanel { node_id: node.0 });
                } else {
                    commands.trigger(CloseHistogramPanel);
                }
            }


        }
    }
}
