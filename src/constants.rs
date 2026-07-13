use bevy::prelude::*;

pub const CANVAS_HEIGHT: f32 = 500.0;
pub const CANVAS_WIDTH: f32 = 800.0;
pub const NODE_RAD: f32 = 20.0;
pub const CANVAS_COLOR: Color = Color::srgb(0.173, 0.227, 0.278); //navy-ish blue
pub const SIDEBAR_COLOR: Color = Color::srgb(0.827, 0.827, 0.827); //light grey
pub const AVAILABLE_LINKS_COLOR: Color = Color::srgb(123./255., 130./255., 76./255.); //army green
pub const NODE_NAME_COLOR: Color = Color::BLACK;
pub const NODE_COLOR: Color = Color::srgb(0.992, 0.447, 0.447); //salmon-y red
pub const ARROW_COLOR: Color = Color::srgb(0.973, 0.937, 0.729); //light yellow-ish
pub const ARROW_THICKNESS: f32 = 2.0;
pub const ARROW_TIP_WIDTH_RATIO: f32 = 10.0;
pub const ARROW_TIP_LENGTH: f32 = 10.0;
pub const SIDEBAR_WIDTH: f32 = CANVAS_WIDTH / 3.5;
pub const ERR_COLOR: Color = Color::srgb(0.45, 0.05, 0.05); //red
pub const SAMPLE_COLOR: Color = Color::srgb(0.05, 0.05, 0.45); //blue
pub const ERR_BORDER_COLOR: Color = Color::srgb(0.9, 0.15, 0.15); //bright red

#[derive(Component)]
pub struct Canvas;

