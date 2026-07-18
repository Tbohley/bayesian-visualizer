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

use fugue::*;


//on all node entities
#[derive(Component)]
pub struct GraphNode(pub u32);

#[derive(Component)]
pub struct NodeLabel(pub String);

pub enum NodeType{
    Random,
    Compute,
    Scalar
}

#[derive(Component)]
pub struct NodeMode(pub NodeType);

#[derive(Debug)]
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

#[derive(Debug)]
pub struct ParamValue (pub &'static str, pub f64);          //TODO: change from f64 to GraphLink

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

impl NodeDisplay for ScalarNode{
    fn label(&self) -> String{
        format!["{:.2}", self.val]
    }
}

//create a node on canvas
pub fn on_background_click(
    mut event: On<Pointer<Click>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    current_nodes: Query<&GraphNode>,
    selected: Option<Single<(Entity, &mut Selected)>>,
    node_mode: Single<&NodeMode>,
    selection_indicators: Query<(Entity, &ChildOf), With<SelectedIndicator>>,
) {
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
    distributions: Query<&RandomNode>,
    selection_indicators: Query<(Entity, &ChildOf), With<SelectedIndicator>>,
){
    //if it is a shift click:
    if input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]){

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
        }else{ 
            commands.spawn((
                GraphLink{
                    from: event.event_target(),
                    to: None
                },
                UnfinishedLink
            ));
            println!("Created an UnfinishedLink");
        }
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
                    Mesh2d(meshes.add(selection_indicator(NODE_RAD))),
                    MeshMaterial2d(materials.add(SELECTION_INDICATOR_COLOR)),
                    Transform::from_xyz(0.0, 0.0, -0.1)));

            commands.trigger(ReloadSidebar);

            let selected_dist_box = distributions.get(event.event_target());
            match selected_dist_box {
                Err(_e) => println!("Selected node has no associated distrbution"),
                Ok(dist) => {
                    let mut rng = thread_rng();
                    println!("Selected node distribution: {:?}", dist.dist);
                    println!("Sample from node: {}", dist.dist.sample(&mut rng))
                }
            };

        }
    }
}