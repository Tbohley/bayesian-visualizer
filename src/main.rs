use bevy::prelude::*;
mod bayesian_core;
mod bevy_to_fugue;
mod constants;
mod data_vis;
mod graph;
mod nodes;
mod sidebar;
mod ui;
use crate::data_vis::{
    HistogramBinCount, apply_typed_histogram_bin_count, close_histogram_panel,
    open_histogram_panel, open_joint_distribution_view, update_histogram_selection_controls,
};
use crate::graph::*;
use crate::nodes::*;
use crate::sidebar::*;
use crate::ui::*;
use bevy_to_fugue::compilation::compile;
use bevy_to_fugue::compilation::poll_inference_job;
use bevy_to_fugue::compilation::update_inference_progress;
pub use constants::*;
use sidebar::compute_menu::on_open_operation_menu;
use sidebar::global::invalidate_compilation_on_graph_change;
use sidebar::global::load_global_sidebar;
use sidebar::global::on_request_graph_change_confirmation;
use sidebar::global::set_inference_controls_enabled;
use sidebar::global::update_random_seed_placeholder;
use sidebar::link_params::on_open_param_link_menu;
use sidebar::plate_menu::{on_open_dataset_menu, on_open_plate_mapping_menu};
use sidebar::random_menu::on_open_distribution_menu;
use sidebar::scalar_menu::on_enter_clicked;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    //spawn clickable background
    commands
        .spawn((
            Canvas,
            Mesh2d(meshes.add(Rectangle::new(1., 1.))),
            MeshMaterial2d(materials.add(CANVAS_COLOR)),
            NodeMode(NodeType::Random),
            Transform::from_xyz(0., 0., -100.),
        ))
        .observe(on_background_click)
        .observe(ui::editor::on_canvas_drag_start)
        .observe(ui::editor::on_canvas_drag)
        .observe(ui::editor::on_canvas_drag_end)
        .observe(on_plate_drag_start)
        .observe(on_plate_drag)
        .observe(on_plate_drag_end);

    commands.insert_resource(Datasets {
        datasets: vec![
            Dataset::from_csv(
                "SATandGPA.csv",
                include_bytes!("../assets/data/SATandGPA.csv"),
            )
            .expect("prefilled data should be valid"),
            Dataset::from_csv(
                "poly_reg.csv",
                include_bytes!("../assets/data/poly_reg.csv"),
            )
            .expect("prefilled data should be valid"),
            Dataset::from_csv(
                "sim_sleep_react.csv",
                include_bytes!("../assets/data/sim_sleep_react.csv"),
            )
            .expect("prefilled data should be valid"),
        ],
    });
}

fn main() {
    App::new()
        .add_plugins((
            ui::editor::EditorPlugin,
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Bayesian Visualizer".to_string(),
                    canvas: cfg!(target_family = "wasm")
                        .then(|| "#bayesian-visualizer".to_string()),
                    fit_canvas_to_parent: cfg!(target_family = "wasm"),
                    ..default()
                }),
                ..default()
            }),
            MeshPickingPlugin,
        ))
        .init_resource::<ReducedView>()
        .init_resource::<HistogramBinCount>()
        .add_observer(on_open_distribution_menu)
        .add_observer(on_open_dataset_menu)
        .add_observer(on_open_plate_mapping_menu)
        .add_observer(on_open_preset_menu)
        .add_observer(on_request_preset_confirmation)
        .add_observer(on_request_graph_change_confirmation)
        .add_observer(on_load_preset)
        .add_observer(on_trigger_close_menus)
        .add_observer(on_open_operation_menu)
        .add_observer(on_open_param_link_menu)
        .add_observer(autofill_next_param)
        .add_observer(set_node_name)
        .add_observer(toggle_reduced_view)
        .add_observer(throw_err)
        .add_observer(show_compilation_error_markers)
        .add_observer(clear_toasts)
        .add_observer(compile)
        .add_observer(set_inference_controls_enabled)
        .add_observer(delete_selected_node)
        .add_observer(reload_sidebar)
        .add_observer(open_histogram_panel)
        .add_observer(open_joint_distribution_view)
        .add_observer(close_histogram_panel)
        .add_systems(Startup, (setup, load_global_sidebar))
        .add_systems(
            Update,
            (
                on_enter_clicked,
                delete_selected_node_on_backspace,
                refresh_links_for_resized_random_nodes,
                refresh_reduced_view,
                apply_typed_histogram_bin_count,
                update_histogram_selection_controls,
                poll_inference_job,
                update_inference_progress,
                invalidate_compilation_on_graph_change,
                relabel_plates,
                update_plate_handle_colors,
                update_random_seed_placeholder,
                tick_error_toasts,
                tick_compilation_error_markers,
                click_error_toasts,
                update_node_observation_colors,
            ),
        )
        .run();
}
