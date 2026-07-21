//use core::f32;
use bevy::prelude::*;
mod nodes;
mod sidebar;
mod graph;
mod ui;
mod constants;
mod bayesian_core;
mod bevy_to_fugue;
use bevy_to_fugue::compilation::compile;
use bevy_to_fugue::compilation::global_sample;
use bevy_to_fugue::compilation::sample_popup;
use bevy_to_fugue::compilation::tick_sample_popups;
pub use constants::*;
use sidebar::compute_menu::on_open_operation_menu;
use sidebar::global::load_global_sidebar;
use sidebar::global::on_open_node_type_menu;
use sidebar::link_params::on_open_param_link_menu;
use sidebar::random_menu::on_open_distribution_menu;
use crate::sidebar::*;
use crate::ui::*;
use crate::nodes::*;

fn setup (
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: ResMut<AssetServer>
) {
    commands.spawn(Camera2d);

    //spawn clickable background
    commands.spawn((
        Canvas,
        Mesh2d(meshes.add(Rectangle::new(CANVAS_WIDTH, CANVAS_HEIGHT))),
        MeshMaterial2d(materials.add(CANVAS_COLOR)),
        NodeMode(NodeType::Random)
    ))
    .observe(on_background_click);

    //load custom cursor resources
    commands.insert_resource(CursorAssets {
        shift_held: asset_server.load("cursors/shift_held.png"),
        finish_link: asset_server.load("cursors/finish_link.png"),
    });

}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MeshPickingPlugin))
        .add_observer(on_open_distribution_menu)
        .add_observer(on_open_node_type_menu)
        .add_observer(on_trigger_close_menus)
        .add_observer(on_open_operation_menu)
        .add_observer(on_open_param_link_menu)
        .add_observer(throw_err)
        .add_observer(compile)
        .add_observer(sample_popup)
        .add_observer(reload_sidebar)
        .add_systems(Startup, (setup, load_global_sidebar))
        .add_systems(Update, (
            on_keypress, 
            on_enter_clicked,
            tick_error_toasts, 
            tick_sample_popups,
            click_error_toasts,
            spin_selection_indicators,
            update_graph_cursor))
        .run();
}


// PROGRESS
/*

------------------Next steps--------------------

Dragging nodes                              DONE
Shiftclick to create an arrow               DONE


-----------------Goals for 7/2------------------

Arrowhead (custom mesh?)                    DONE
Arrows on drag                              DONE
Arrows disappear on node deletion           DONE

-----------------Goals for 7/7------------------

Basic fugue scaffolding w/ normal dists     DONE
Simple sampling?                            DONE
Plates, parameters


-----------------Goals for 7/10-----------------

Node sidebar{
    random vs parameter                     
    dist. params{
        change distribution button          DONE
        apply changes button
    }
}
Plate dragging creation


-----------------Future goals-------------------

Global sidebar{
    drag n drop construction
    dummy node/param/plate?
    update button{
        plate logic and implementation      
    }
}



Single click allows node name editing,
eventually will be -> popup with 
distribution/property editing               DONE

Various distribution options

Single sampling/forward sampling

Plot viewing

Crosslink, brushing interaction

WASM support and CI/CD

Interval type checking

Abs() operation


-----------------Optional/stretch goals-----------------

Ghost arrow after shift-clicking a node
that tracks cursor until end node clicked   

Different color schemes

Rewrap all uses of .unwrap()

Delete link buttons in incoming links menu

Make arrows touch non-random nodes



-------------------Bug tracker------------------

Deletion of a node in an UnfinishedLink     
leads to panic                             FIXED

Smashing keys on rename interacts with
a despawned entity (probably NamedNode)
and panics

Dragging a node, dropping it and then
clicking registers as a double click
and deletes it

*/