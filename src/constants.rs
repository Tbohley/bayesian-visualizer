use bevy::prelude::*;

use crate::{graph::{GraphPreset, PresetBounds, PresetMapping, PresetNode, PresetParameter, PresetPlate}, nodes::Operation};

pub const CANVAS_HEIGHT: f32 = 1000.0;
pub const CANVAS_WIDTH: f32 = 2000.0;
pub const SIDEBAR_WIDTH: f32 = 300.0;

pub const RANDOM_NODE_RAD: f32 = 20.0;
pub const COMPUTE_NODE_RAD: f32 = 18.0;
pub const SCALAR_NODE_RAD: f32 = 7.0;
pub const NODE_BORDER_WEIGHT: f32 = 4.0;
pub const MAX_NODE_NAME_CHARS: usize = 10;
pub const RANDOM_NODE_NAME_ADVANCE: f32 = 9.0;

pub const ARROW_THICKNESS: f32 = 2.0;
pub const ARROW_TIP_WIDTH_RATIO: f32 = 10.0;
pub const ARROW_TIP_LENGTH: f32 = 10.0;

pub const NODE_LABEL_FONT_SIZE_SMALL: i32 = 12;
pub const NODE_LABEL_FONT_SIZE: i32 = 20;

/// The shared font style for every text entity in the application.
///
/// Keep font sizes at each call site; this only centralizes the font face and
/// other style defaults so they can be changed application-wide later.
pub fn text_font() -> TextFont {
    TextFont::default()
}

pub const CURSOR_DEFAULT: &str = "cursors/default.png";
pub const CURSOR_SHIFT_HELD: &str = "cursors/shift_held.png";
pub const CURSOR_FINISH_LINK: &str = "cursors/finish_link.png";

pub const PLATE_Z: f32 = 0.5;
pub const MIN_PLATE_EXTENT: f32 = 8.0;
pub const PLATE_BORDER_THICKNESS: f32 = 7.0;

//colors
pub const CANVAS_COLOR: Color = Color::WHITE; // white
pub const SIDEBAR_COLOR: Color = Color::srgb(0.827, 0.827, 0.827); //light grey
pub const NODE_NAME_COLOR: Color = Color::BLACK;
pub const BUTTON_COLOR: Color = Color::BLACK;
pub const RANDOM_NODE_COLOR: Color = Color::srgb(1.0, 0., 0.); //red
pub const COMPUTE_NODE_COLOR: Color = Color::srgb(0.77, 0.89, 0.86); //dull teal
pub const SCALAR_NODE_COLOR: Color = Color::srgb(0.65, 0.51, 0.57); //lavendar
pub const ARROW_COLOR: Color = Color::BLACK; //light yellow-ish
pub const ERR_COLOR: Color = Color::srgb(0.45, 0.05, 0.05); //red
pub const SAMPLE_COLOR: Color = Color::srgb(0.05, 0.05, 0.45); //blue
pub const ERR_BORDER_COLOR: Color = Color::srgb(0.9, 0.15, 0.15); //bright red
pub const SELECTION_INDICATOR_COLOR: Color = Color::srgb(123./255., 130./255., 76./255.); //army green
pub const PLATE_COLOR: Color = Color::srgb(0.04, 0.20, 0.48);


#[derive(Component)]
pub struct Canvas;

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
            PresetNode::Scalar { id: 10, position: [-186.27344, 81.90624], value: 2.0, name: None },
            PresetNode::Scalar { id: 11, position: [-181.6836, 9.242197], value: 1.0, name: None },
            PresetNode::Scalar { id: 12, position: [-182.0586, 277.2695], value: 500.0, name: None },
            PresetNode::Scalar { id: 13, position: [-54.433563, 279.99606], value: 200.0, name: None },
            PresetNode::Scalar { id: 14, position: [56.71095, 256.05075], value: 800.0, name: None },
            PresetNode::Scalar { id: 15, position: [155.64842, 252.37497], value: 300.0, name: None },
            PresetNode::Scalar { id: 16, position: [226.14844, 315.80856], value: 80.0, name: None },
            PresetNode::Scalar { id: 17, position: [315.4297, 192.332], value: 1.0, name: None }
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
            PresetNode::Scalar { id: 12, position: [-238.10156, 19.761707], value: 0.0, name: None },
            PresetNode::Scalar { id: 13, position: [-237.98047, -39.82811], value: 1.5, name: None },
            PresetNode::Scalar { id: 14, position: [-65.402336, 338.67575], value: 0.0, name: None },
            PresetNode::Scalar { id: 15, position: [-9.644539, 334.3984], value: 5.0, name: None },
            PresetNode::Scalar { id: 16, position: [190.13672, 321.22653], value: 5.0, name: None },
            PresetNode::Scalar { id: 17, position: [145.04297, 312.98434], value: 0.0, name: None },
            PresetNode::Scalar { id: 18, position: [-236.3086, 127.69531], value: 2.0, name: None },
            PresetNode::Compute { id: 19, position: [58.835907, 86.98436], operation: Operation::Add, parameters: vec![PresetParameter { name: "first", source: Some(10) }, PresetParameter { name: "second", source: Some(9) }] }
        ],
        plates: vec![
            PresetPlate { id: 1, bounds: PresetBounds { min: [-182.67578, -38.87111], max: [271.9453, 191.82811] }, dataset_id: "poly_reg.csv", mapping: vec![PresetMapping { node: 6, column: "sample_x" }, PresetMapping { node: 7, column: "sample_y" }] }
        ],
    },
    
    GraphPreset {
        id: "sleep_react",
        title: "Reasoning Task",
        description: "description",
        study_condition: None,
        nodes: vec![
            PresetNode::Random { id: 2, position: [-75.14453, 49.527332], name: Some("sleep"), distribution: "Normal", parameters: vec![PresetParameter { name: "mean", source: Some(10) }, PresetParameter { name: "std_dev", source: Some(11) }] },
            PresetNode::Random { id: 3, position: [227.60548, 46.960938], name: Some("react_time"), distribution: "Normal", parameters: vec![PresetParameter { name: "mean", source: Some(8) }, PresetParameter { name: "std_dev", source: Some(4) }] },
            PresetNode::Random { id: 4, position: [252.86328, 173.88278], name: Some("noise"), distribution: "LogNormal", parameters: vec![PresetParameter { name: "mean", source: Some(16) }, PresetParameter { name: "std_dev", source: Some(9) }] },
            PresetNode::Random { id: 5, position: [-106.093765, 187.28903], name: Some("slope"), distribution: "Normal", parameters: vec![PresetParameter { name: "mean", source: Some(12) }, PresetParameter { name: "std_dev", source: Some(13) }] },
            PresetNode::Random { id: 6, position: [89.31254, 182.79688], name: Some("intercept"), distribution: "Normal", parameters: vec![PresetParameter { name: "mean", source: Some(14) }, PresetParameter { name: "std_dev", source: Some(15) }] },
            PresetNode::Compute { id: 7, position: [1.957016, 48.085926], operation: Operation::Multiply, parameters: vec![PresetParameter { name: "first", source: Some(5) }, PresetParameter { name: "second", source: Some(2) }] },
            PresetNode::Compute { id: 8, position: [109.83594, 43.75391], operation: Operation::Add, parameters: vec![PresetParameter { name: "first", source: Some(6) }, PresetParameter { name: "second", source: Some(7) }] },
            PresetNode::Scalar { id: 9, position: [309.12888, 219.29686], value: 1.0, name: None },
            PresetNode::Scalar { id: 10, position: [-186.27344, 81.90624], value: 6.5, name: None },
            PresetNode::Scalar { id: 11, position: [-181.6836, 9.242197], value: 2.0, name: None },
            PresetNode::Scalar { id: 12, position: [-182.0586, 277.2695], value: 20.0, name: None },
            PresetNode::Scalar { id: 13, position: [-54.433563, 279.99606], value: 10.0, name: None },
            PresetNode::Scalar { id: 14, position: [56.71095, 256.05075], value: 300.0, name: None },
            PresetNode::Scalar { id: 15, position: [155.64842, 252.37497], value: 50.0, name: None },
            PresetNode::Scalar { id: 16, position: [245.2539, 273.4609], value: 1.9, name: None }
        ],
        plates: vec![
            PresetPlate { id: 1, bounds: PresetBounds { min: [-138.90625, -2.8125076], max: [310.1289, 107.65624] }, dataset_id: "sim_sleep_react.csv", mapping: vec![PresetMapping { node: 2, column: "sleep_hours" }, PresetMapping { node: 3, column: "reaction_time_ms" }] }
        ],
    }]
}
