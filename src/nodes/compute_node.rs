use bevy::prelude::*;
use super::*;

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
