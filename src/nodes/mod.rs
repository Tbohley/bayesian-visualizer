pub mod random_var;
use fugue::Distribution;
pub use random_var::*;
use crate::constants::*;
use bevy::prelude::*;
use crate::graph::*;
use crate::sidebar::*;
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

impl NodeDisplay for ComputeNode{
    fn label(&self) -> String{
        match self.operation{
            Operation::Add => "+".to_string(),
            Operation::Subtract => "-".to_string(),
            Operation::Multiply => "*".to_string(),
            Operation::Divide => "/".to_string(),
            Operation::Exponential => "exp".to_string(),
            Operation::Logarithm => "log".to_string(),
            Operation::Power => "^".to_string(),
            Operation::Sum => "∑".to_string(),
            Operation::Product => "prod".to_string()
        }
    }
}

#[derive(Component)]
pub struct ScalarNode{
    pub val: f64
}

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
    node_mode: Single<&NodeMode>
) {
    if let Some(single) = selected{
        let (entity, _selected_comp) = single.into_inner();
        //deselect currently selected node + close context menus
        commands.entity(entity).remove::<Selected>();
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

pub fn new_compute(
    commands: &mut Commands,
    loc: Vec3,
    node_num: u32,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
){
    commands.spawn((
        GraphNode(node_num),
        Pickable{should_block_lower: true, is_hoverable: true},
        Mesh2d(meshes.add(Circle::new(NODE_RAD*0.75))),
        MeshMaterial2d(materials.add(COMPUTE_NODE_COLOR)),
        Transform::from_xyz(
            loc.x,
            loc.y,
            1.0),
        ComputeNode{      //TODO: move to global sidebar
            operation: Operation::Add,
            params: vec![ParamValue("first", 2.),ParamValue("second",2.)]
        }
    )).with_child((
        NodeLabel("+".to_string()),
        Text2d::new("+"),
        TextColor(NODE_NAME_COLOR),
        Pickable::IGNORE,
        Transform::from_xyz(0.0,0.0,2.0)
    ))
    .observe(on_node_drag)
    .observe(on_node_click);
}

pub fn new_scalar(
    commands: &mut Commands,
    loc: Vec3,
    node_num: u32,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
){
    commands.spawn((
        GraphNode(node_num),
        Pickable{should_block_lower: true, is_hoverable: true},
        Mesh2d(meshes.add(Circle::new(NODE_RAD*0.5))),
        MeshMaterial2d(materials.add(SCALAR_NODE_COLOR)),
        Transform::from_xyz(
            loc.x,
            loc.y,
            1.0),
        ScalarNode{      //TODO: move to global sidebar
            val: 1.
        }
    )).with_child((
        NodeLabel("1".to_string()),
        Text2d::new("1"),
        TextFont{
            font_size: px(12).into(),
            ..default()
        },
        TextColor(NODE_NAME_COLOR),
        Pickable::IGNORE,
        Transform::from_xyz(0.0,0.0,2.0)
    ))
    .observe(on_node_drag)
    .observe(on_node_click);
}


pub fn replace_node_label(
    commands: &mut Commands,
    node_entity: Entity,
    label_text: impl Into<String>,
    labels: &Query<(Entity, &NodeLabel, &ChildOf)>,
) {
    let label_text = label_text.into();

    for (label_entity, _, child_of) in labels.iter() {
        if child_of.parent() == node_entity {
            commands.entity(label_entity).despawn();
        }
    }

    commands.entity(node_entity).with_child((
        NodeLabel(label_text.clone()),
        Text2d::new(label_text.clone()),
        TextColor(NODE_NAME_COLOR),
        TextFont{
            font_size: match &label_text.len() {
                n if *n > 1 => px(NODE_LABEL_FONT_SIZE_SMALL).into(),
                _ => px(NODE_LABEL_FONT_SIZE).into()
            },
            ..default()
        },
        Pickable::IGNORE,
        Transform::from_xyz(0.0, 0.0, 2.0),
    ));
}
