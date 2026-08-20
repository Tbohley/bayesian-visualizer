//use core::f32;
use bevy::prelude::*;
mod nodes;
mod sidebar;
mod graph;
mod ui;
mod constants;
mod data_vis;
mod bayesian_core;
mod bevy_to_fugue;
use bevy_to_fugue::compilation::compile;
use bevy_to_fugue::compilation::poll_inference_job;
use bevy_to_fugue::compilation::sample_popup;
use bevy_to_fugue::compilation::tick_sample_popups;
use bevy_to_fugue::compilation::update_inference_progress;
pub use constants::*;
use sidebar::compute_menu::on_open_operation_menu;
use sidebar::global::load_global_sidebar;
use sidebar::global::invalidate_compilation_on_graph_change;
use sidebar::global::on_open_node_type_menu;
use sidebar::global::set_inference_controls_enabled;
use sidebar::global::set_posterior_sample_enabled;
use sidebar::global::update_random_seed_placeholder;
use sidebar::link_params::on_open_param_link_menu;
use sidebar::plate_menu::{on_open_dataset_menu, on_open_plate_mapping_menu};
use sidebar::random_menu::on_open_distribution_menu;
use sidebar::scalar_menu::on_enter_clicked;
use crate::sidebar::*;
use crate::ui::*;
use crate::nodes::*;
use crate::graph::*;
use crate::data_vis::{
    apply_typed_histogram_bin_count,
    close_histogram_panel,
    open_joint_distribution_view,
    open_histogram_panel,
    update_histogram_selection_controls,
};

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
    .observe(on_background_click)
    .observe(on_plate_drag_start)
    .observe(on_plate_drag)
    .observe(on_plate_drag_end);

    //load custom cursor resources
    commands.insert_resource(CursorAssets {
        shift_held: asset_server.load("cursors/shift_held.png"),
        finish_link: asset_server.load("cursors/finish_link.png"),
    });

    commands.insert_resource(Datasets {
        datasets: vec![Dataset::from_csv("assets/data/SATandGPA.csv").expect("prefilled data should be valid"),
        Dataset::from_csv("assets/data/poly_reg.csv").expect("prefilled data should be valid")
        ]
    });

}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MeshPickingPlugin))
        .init_resource::<ReducedView>()
        .add_observer(on_open_distribution_menu)
        .add_observer(on_open_dataset_menu)
        .add_observer(on_open_plate_mapping_menu)
        .add_observer(on_open_node_type_menu)
        .add_observer(on_open_preset_menu)
        .add_observer(on_request_preset_confirmation)
        .add_observer(on_load_preset)
        .add_observer(on_trigger_close_menus)
        .add_observer(on_open_operation_menu)
        .add_observer(on_open_param_link_menu)
        .add_observer(autofill_next_param)
        .add_observer(set_node_name)
        .add_observer(toggle_reduced_view)
        .add_observer(throw_err)
        .add_observer(clear_toasts)
        .add_observer(compile)
        .add_observer(set_inference_controls_enabled)
        .add_observer(set_posterior_sample_enabled)
        .add_observer(sample_popup)
        .add_observer(reload_sidebar)
        .add_observer(open_histogram_panel)
        .add_observer(open_joint_distribution_view)
        .add_observer(close_histogram_panel)
        .add_systems(Startup, (setup, load_global_sidebar))
        .add_systems(Update, (
            on_enter_clicked,
            refresh_reduced_view,
            apply_typed_histogram_bin_count,
            update_histogram_selection_controls,
            poll_inference_job,
            update_inference_progress,
            invalidate_compilation_on_graph_change,
            update_random_seed_placeholder,
            tick_error_toasts, 
            tick_sample_popups,
            click_error_toasts,
            update_node_observation_colors,
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
