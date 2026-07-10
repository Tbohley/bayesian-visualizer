//use core::f32;

use bevy::prelude::*;
mod nodes;
mod sidebar;
mod graph;
mod ui;
mod constants;
pub use constants::*;
use crate::sidebar::*;
use crate::ui::*;
use crate::nodes::*;

fn setup (
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    //spawn clickable background
    commands.spawn((
        Canvas,
        Mesh2d(meshes.add(Rectangle::new(CANVAS_WIDTH, CANVAS_HEIGHT))),
        MeshMaterial2d(materials.add(CANVAS_COLOR))
    ))
    .observe(on_background_click);

    commands.spawn((
        Text2d::new("Click to create a new node.\n\
                        Shift click a parent and then a child\n\
                        node to create a link between them.\n\
                        Double-click a node to delete it.\n\
                        Click a node, then a letter key,\n\
                        to assign it a one-letter name."),
        Transform{
            translation: vec3(-450.,300.0,1.0),
            scale: vec3(0.7,0.7,1.0),
            rotation: Quat::from_rotation_z(0.0)
        }
    ));
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MeshPickingPlugin))
        .add_observer(on_open_context_menu)
        .add_observer(on_trigger_close_menus)
        .add_observer(throw_err)
        .add_observer(reload_sidebar)
        .add_systems(Startup, setup)
        .add_systems(Update, (
            on_keypress, 
            on_enter_clicked,
            tick_error_toasts, 
            click_error_toasts))
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


-----------------Optional goals-----------------

Ghost arrow after shift-clicking a node
that tracks cursor until end node clicked   

Different color schemes

Rewrap all uses of .unwrap()



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