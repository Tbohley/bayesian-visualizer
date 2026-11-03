use super::*;
use crate::bayesian_core::{ScalarType, Value};
use crate::{constants::*, graph::on_node_drag};

/// Only one-dimensional vectors can be stored in a vector constant.
#[derive(Clone, Debug, PartialEq)]
pub enum VectorElements {
    Real(Vec<f64>),
    Integer(Vec<i64>),
}

impl VectorElements {
    pub fn element_type(&self) -> ScalarType {
        match self {
            Self::Real(_) => ScalarType::Real,
            Self::Integer(_) => ScalarType::Integer,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Real(values) => values.len(),
            Self::Integer(values) => values.len(),
        }
    }

    pub fn value(&self) -> Value {
        match self {
            Self::Real(values) => Value::RealVector(values.clone().into()),
            Self::Integer(values) => Value::IntegerVector(values.clone().into()),
        }
    }

    pub fn text(&self) -> String {
        match self {
            Self::Real(values) => values.iter().map(ToString::to_string).collect::<Vec<_>>(),
            Self::Integer(values) => values.iter().map(ToString::to_string).collect::<Vec<_>>(),
        }
        .join(", ")
    }

    pub fn parse(text: &str, element: ScalarType) -> Result<Self, String> {
        if text.trim().is_empty() {
            return Err("A vector needs at least one element.".into());
        }
        let items = text.split(',').map(str::trim).collect::<Vec<_>>();
        let values = match element {
            ScalarType::Real => Self::Real(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        item.parse::<f64>()
                            .ok()
                            .filter(|value| value.is_finite())
                            .ok_or_else(|| {
                                format!("Element {} must be a finite real number.", index + 1)
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            ScalarType::Integer => Self::Integer(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        item.parse::<i64>().map_err(|_| {
                            format!(
                                "Element {} must be an integer between {} and {}.",
                                index + 1,
                                i64::MIN,
                                i64::MAX
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        };
        Ok(values)
    }
}

#[derive(Component, Clone, Debug, PartialEq)]
pub struct VectorNode {
    pub elements: VectorElements,
    pub name: Option<String>,
}

impl Default for VectorNode {
    fn default() -> Self {
        Self {
            elements: VectorElements::Real(vec![1.0; 3]),
            name: None,
        }
    }
}

impl NodeDisplay for VectorNode {
    fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| {
            let items = match &self.elements {
                VectorElements::Real(values) => values
                    .iter()
                    .take(3)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                VectorElements::Integer(values) => values
                    .iter()
                    .take(3)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            };
            let text = format!(
                "[{}{}]",
                items.join(", "),
                if self.elements.len() > 3 { ", …" } else { "" }
            );
            if text.chars().count() > 30 {
                format!("vector[{}]", self.elements.len())
            } else {
                text
            }
        })
    }
}

pub fn new_vector(
    commands: &mut Commands,
    loc: Vec3,
    node_num: u32,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    spawn_vector(
        commands,
        loc,
        node_num,
        VectorNode::default(),
        &mut meshes,
        &mut materials,
    );
}

pub fn spawn_vector(
    commands: &mut Commands,
    loc: Vec3,
    node_num: u32,
    vector: VectorNode,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) -> Entity {
    let label = vector.label();
    commands
        .spawn((
            GraphNode(node_num),
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
            Mesh2d(meshes.add(Circle::new(SCALAR_NODE_RAD))),
            MeshMaterial2d(materials.add(SCALAR_NODE_COLOR)),
            Transform::from_xyz(loc.x, loc.y, 1.0),
            vector,
        ))
        .with_child((
            NodeLabel,
            Text2d::new(label),
            TextColor(NODE_NAME_COLOR),
            TextFont {
                font_size: px(NODE_LABEL_FONT_SIZE_SMALL).into(),
                ..text_font()
            },
            Pickable::IGNORE,
            Transform::from_xyz(0.0, SCALAR_NODE_RAD + 10.0, 2.0),
        ))
        .observe(crate::ui::editor::on_canvas_drag_start)
        .observe(crate::ui::editor::on_canvas_drag)
        .observe(crate::ui::editor::on_canvas_drag_end)
        .observe(on_node_drag)
        .observe(on_node_click)
        .id()
}
