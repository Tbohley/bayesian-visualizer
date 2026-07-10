use bevy::prelude::*;

pub const CANVAS_HEIGHT: f32 = 500.0;
pub const CANVAS_WIDTH: f32 = 800.0;
pub const NODE_RAD: f32 = 20.0;
pub const CANVAS_COLOR: Color = Color::srgb(0.173, 0.227, 0.278);
pub const SIDEBAR_COLOR: Color = Color::srgb(0.827, 0.827, 0.827);
pub const NODE_NAME_COLOR: Color = Color::BLACK;
pub const NODE_COLOR: Color = Color::srgb(0.992, 0.447, 0.447);
pub const ARROW_COLOR: Color = Color::srgb(0.973, 0.937, 0.729);
pub const ARROW_THICKNESS: f32 = 2.0;
pub const ARROW_TIP_WIDTH_RATIO: f32 = 10.0;
pub const ARROW_TIP_LENGTH: f32 = 10.0;
pub const SIDEBAR_WIDTH: f32 = CANVAS_WIDTH / 4.;
pub const ERR_COLOR: Color = Color::srgb(0.45, 0.05, 0.05);
pub const SAMPLE_COLOR: Color = Color::srgb(0.05, 0.05, 0.45);

#[derive(Component)]
pub struct Canvas;

