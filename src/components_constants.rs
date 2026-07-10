use bevy::prelude::*;
use fugue::*;

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

//on all node entities
#[derive(Component)]
pub struct GraphNode(pub u32);

//on the text child entity of a named node
#[derive(Component)]
pub struct NamedNode(pub String);

//on the text child of a default node
#[derive(Component)]    
pub struct UnnamedNode;

#[derive(Component)]
pub struct Canvas;

//on links between nodes
#[derive(Component)]    
pub struct GraphLink{
    pub from: Entity,
    pub to: Option<Entity>
}

#[derive(Component)]
pub struct Sidebar;

pub trait DistributionDebug<T>: Distribution<T> + std::fmt::Debug {}
impl<T, D: Distribution<T> + std::fmt::Debug> DistributionDebug<T> for D {}

#[derive(Debug)]
pub struct ParamValue (pub &'static str, pub f64);

//on random variable nodes
#[derive(Component)]
pub struct RandomVar{
    pub dist_type: String,
    pub dist: Box<dyn DistributionDebug<f64>>,
    pub params: Vec<ParamValue>
}

//on unfinished (invisible) arrows
#[derive(Component)]
pub struct UnfinishedLink;

//on currently selected node
#[derive(Component)]
pub struct Selected;

#[derive(Component)]
pub struct ParamTextbox(pub usize);

/// event opening a new context menu at position `pos`
#[derive(Event)]
pub struct OpenContextMenu {
    pub pos: Vec2,
}

/// event will be sent to close currently open context menus
#[derive(Event)]
pub struct CloseContextMenus;

#[derive(Event)]
pub struct ReloadSidebar;

/// marker component identifying root of a context menu
#[derive(Component)]
pub struct ContextMenu;

/// context menu item data storing what background color `Srgba` it activates
#[derive(Component)]
pub struct ContextMenuItem(pub String);

#[derive(Event)]
pub struct ErrorToast {
    pub text: String,
    pub color: Color
}

#[derive(Component)]
pub struct ErrorToastBox {
    pub timer: Timer,
}