use super::*;
use bevy::prelude::*;

impl NodeDisplay for ComputeNode {
    fn label(&self) -> String {
        match self.operation {
            Operation::Add => "+".to_string(),
            Operation::Subtract => "-".to_string(),
            Operation::Multiply => "*".to_string(),
            Operation::Divide => "/".to_string(),
            Operation::Exponential => "exp".to_string(),
            Operation::Logarithm => "log".to_string(),
            Operation::Power => "^".to_string(),
            Operation::Sum => "∑".to_string(),
            Operation::Product => "prod".to_string(),
        }
    }
}

impl Operation {
    pub fn evaluate(&self, params: &[f64]) -> Result<f64, String> {
        let unary = || {
            params
                .first()
                .copied()
                .filter(|_| params.len() == 1)
                .ok_or_else(|| "operation expected 1 parameter".to_string())
        };

        let binary = || {
            params
                .first()
                .zip(params.get(1))
                .map(|(&a, &b)| (a, b))
                .filter(|_| params.len() == 2)
                .ok_or_else(|| "operation expected 2 parameters".to_string())
        };

        match self {
            Operation::Add => {
                let (a, b) = binary()?;
                Ok(a + b)
            }
            Operation::Subtract => {
                let (a, b) = binary()?;
                Ok(a - b)
            }
            Operation::Multiply => {
                let (a, b) = binary()?;
                Ok(a * b)
            }
            Operation::Divide => {
                let (a, b) = binary()?;

                if b == 0.0 {
                    return Err("division by zero".into());
                }

                Ok(a / b)
            }
            Operation::Exponential => Ok(unary()?.exp()),
            Operation::Logarithm => {
                let value = unary()?;

                if value <= 0.0 {
                    return Err("logarithm input must be positive".into());
                }

                Ok(value.ln())
            }
            Operation::Power => {
                let (base, exponent) = binary()?;
                let result = base.powf(exponent);

                if !result.is_finite() {
                    return Err("invalid power operation".into());
                }

                Ok(result)
            }
            Operation::Product => Err("Not implemented yet".to_string()),
            Operation::Sum => Err("Not implemented yet".to_string()),
        }
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
        .observe(on_node_drag)
        .observe(on_node_click)
        .id()
}

pub fn compute_params(operation: &Operation) -> Vec<ParamValue> {
    match operation {
        Operation::Add => vec![ParamValue("first", None), ParamValue("second", None)],
        Operation::Subtract => vec![ParamValue("first", None), ParamValue("second", None)],
        Operation::Multiply => vec![ParamValue("first", None), ParamValue("second", None)],
        Operation::Divide => vec![ParamValue("dividend", None), ParamValue("divisor", None)],
        Operation::Power => vec![ParamValue("base", None), ParamValue("exponent", None)],
        Operation::Exponential => vec![ParamValue("input", None)],
        Operation::Logarithm => vec![ParamValue("input", None)],
        Operation::Sum => vec![ParamValue("values", None)],
        Operation::Product => vec![ParamValue("values", None)],
    }
}
