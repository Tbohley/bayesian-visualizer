use super::*;
use crate::{constants::*, graph::on_node_drag};
use bevy::prelude::*;

#[derive(Component)]
pub struct ComputeNode {
    pub operation: Operation,
    pub params: Vec<ParamValue>,
}

impl NodeDisplay for ComputeNode {
    fn label(&self) -> String {
        self.operation.symbol().to_string()
    }
}

pub fn new_compute(
    commands: &mut Commands,
    loc: Vec3,
    node_num: u32,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    spawn_compute(
        commands,
        loc,
        node_num,
        ComputeNode {
            operation: Operation::Add,
            params: vec![ParamValue("first", None), ParamValue("second", None)],
        },
        &mut meshes,
        &mut materials,
    );
}

pub fn spawn_compute(
    commands: &mut Commands,
    loc: Vec3,
    node_num: u32,
    compute_node: ComputeNode,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) -> Entity {
    let label = compute_node.label();
    commands
        .spawn((
            GraphNode(node_num),
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
            Mesh2d(meshes.add(Circle::new(COMPUTE_NODE_RAD))),
            MeshMaterial2d(materials.add(COMPUTE_NODE_COLOR)),
            Transform::from_xyz(loc.x, loc.y, 1.0),
            compute_node,
        ))
        .with_child((
            NodeLabel,
            Text2d::new(label.clone()),
            TextFont {
                font_size: match &label.len() {
                    n if *n > 1 => px(NODE_LABEL_FONT_SIZE_SMALL).into(),
                    _ => px(NODE_LABEL_FONT_SIZE).into(),
                },
                ..text_font()
            },
            TextColor(NODE_NAME_COLOR),
            Pickable::IGNORE,
            Transform::from_xyz(0.0, 0.0, 2.0),
        ))
        .observe(crate::ui::editor::on_canvas_drag_start)
        .observe(crate::ui::editor::on_canvas_drag)
        .observe(crate::ui::editor::on_canvas_drag_end)
        .observe(on_node_drag)
        .observe(on_node_click)
        .id()
}

pub fn compute_params(operation: &Operation) -> Vec<ParamValue> {
    match operation {
        Operation::Add | Operation::Subtract | Operation::Multiply => {
            vec![ParamValue("first", None), ParamValue("second", None)]
        }
        Operation::Divide => vec![ParamValue("dividend", None), ParamValue("divisor", None)],
        Operation::Power => vec![ParamValue("base", None), ParamValue("exponent", None)],
        Operation::Index => vec![ParamValue("values", None), ParamValue("index", None)],
        Operation::Collect => vec![ParamValue("element", None), ParamValue("element", None)],
        operation if operation.requires_vector() => vec![ParamValue("values", None)],
        _ => vec![ParamValue("input", None)],
    }
}
