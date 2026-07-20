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
        Mesh2d(meshes.add(Circle::new(COMPUTE_NODE_RAD))),
        MeshMaterial2d(materials.add(COMPUTE_NODE_COLOR)),
        Transform::from_xyz(
            loc.x,
            loc.y,
            1.0),
        ComputeNode{
            operation: Operation::Add,
            params: vec![ParamValue("first", None),ParamValue("second", None)]
        }
    )).with_child((
        NodeLabel,
        Text2d::new("+"),
        TextColor(NODE_NAME_COLOR),
        Pickable::IGNORE,
        Transform::from_xyz(0.0,0.0,2.0)
    ))
    .observe(on_node_drag)
    .observe(on_node_click);
}

pub fn compute_params(operation: &Operation) -> Vec<ParamValue> {
    match operation {
        Operation::Add => vec![ParamValue("first", None), ParamValue("second", None),],
        Operation::Subtract => vec![ParamValue("first", None),ParamValue("second", None),],
        Operation::Multiply => vec![ParamValue("first", None),ParamValue("second", None),],
        Operation::Divide => vec![ParamValue("dividend", None),ParamValue("divisor", None),],
        Operation::Power => vec![ParamValue("base", None),ParamValue("exponent", None),],
        Operation::Exponential => vec![ParamValue("input", None),],
        Operation::Logarithm => vec![ParamValue("input", None),],
        Operation::Sum => vec![ParamValue("values", None),],
        Operation::Product => vec![ParamValue("values", None),],
    }
}