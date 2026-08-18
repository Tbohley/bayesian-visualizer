use bevy::prelude::*;

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
