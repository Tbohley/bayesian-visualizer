use super::*;
use crate::{
    graph::{PlateBounds, Selected},
    nodes::OpenNodeInspection,
};
#[test]
fn live_histogram_updates_do_not_reopen_panels_after_returning_to_editing() {
    use bevy::ecs::system::RunSystemOnce;
    #[derive(Resource, Default)]
    struct OpenedNodes(Vec<u32>);
    let mut app = App::new();
    app.init_resource::<OpenedNodes>().add_observer(
        |event: On<OpenNodeInspection>, mut opened: ResMut<OpenedNodes>| {
            opened.0.push(event.node_id);
        },
    );
    app.world_mut()
        .run_system_once(
            |mut commands: Commands, selected: Query<&GraphNode, With<Selected>>| {
                reopen_selected_histogram(&mut commands, &selected, None, Some(7), false);
            },
        )
        .unwrap();
    assert!(app.world().resource::<OpenedNodes>().0.is_empty());
    app.world_mut()
        .run_system_once(
            |mut commands: Commands, selected: Query<&GraphNode, With<Selected>>| {
                reopen_selected_histogram(&mut commands, &selected, None, Some(7), true);
            },
        )
        .unwrap();
    assert_eq!(app.world().resource::<OpenedNodes>().0, vec![7]);
}

#[test]
fn compilation_errors_extract_stable_node_ids() {
    assert_eq!(
        node_ids_in_compilation_error(
            "invalid dependency from node 12 to node 4 at node#12[plate#3=0]"
        ),
        vec![4, 12]
    );
    assert!(node_ids_in_compilation_error("plate 3 has no dataset").is_empty());
}

#[test]
fn plate_ir_records_direct_nested_contents() {
    let plates = vec![
        (
            1,
            PlateBounds::from_points(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0)),
            10,
        ),
        (
            2,
            PlateBounds::from_points(Vec2::new(20.0, 20.0), Vec2::new(80.0, 80.0)),
            15,
        ),
    ];
    let nodes = vec![
        (1, Vec2::new(10.0, 10.0)),
        (2, Vec2::new(50.0, 50.0)),
        (3, Vec2::new(120.0, 120.0)),
    ];

    let result = compile_plate_irs(&plates, &nodes).unwrap();

    assert_eq!(result[&1].nodes, vec![1, 2]);
    assert_eq!(result[&1].plates, vec![2]);
    assert_eq!(result[&2].nodes, vec![2]);
    assert!(result[&2].plates.is_empty());
}

#[test]
fn plate_ir_accepts_partial_overlap_with_complete_membership() {
    let plates = vec![
        (
            10,
            PlateBounds::from_points(Vec2::ZERO, Vec2::new(100.0, 100.0)),
            3,
        ),
        (
            11,
            PlateBounds::from_points(Vec2::new(50.0, 50.0), Vec2::new(150.0, 150.0)),
            4,
        ),
    ];

    let nodes = vec![(1, Vec2::new(75.0, 75.0))];
    let result = compile_plate_irs(&plates, &nodes).unwrap();
    assert_eq!(result[&10].nodes, vec![1]);
    assert_eq!(result[&11].nodes, vec![1]);
}

#[test]
fn plate_ir_allows_touching_sibling_borders() {
    let plates = vec![
        (
            10,
            PlateBounds::from_points(Vec2::ZERO, Vec2::new(50.0, 50.0)),
            3,
        ),
        (
            11,
            PlateBounds::from_points(Vec2::new(50.0, 0.0), Vec2::new(100.0, 50.0)),
            4,
        ),
    ];

    assert!(compile_plate_irs(&plates, &[]).is_ok());
}

#[test]
fn plate_ir_includes_node_on_every_touching_border() {
    let plates = vec![
        (
            10,
            PlateBounds::from_points(Vec2::ZERO, Vec2::new(50.0, 50.0)),
            3,
        ),
        (
            11,
            PlateBounds::from_points(Vec2::new(50.0, 0.0), Vec2::new(100.0, 50.0)),
            4,
        ),
    ];
    let nodes = vec![(1, Vec2::new(50.0, 25.0))];

    let result = compile_plate_irs(&plates, &nodes).unwrap();
    assert_eq!(result[&10].nodes, vec![1]);
    assert_eq!(result[&11].nodes, vec![1]);
}

#[test]
fn node_motion_invalidates_only_when_complete_membership_changes() {
    let plates = vec![
        (
            10,
            PlateBounds::from_points(Vec2::ZERO, Vec2::new(100.0, 100.0)),
            2,
        ),
        (
            20,
            PlateBounds::from_points(Vec2::new(50.0, 0.0), Vec2::new(150.0, 100.0)),
            3,
        ),
    ];
    let before = compile_plate_irs(&plates, &[(1, Vec2::new(60.0, 20.0))]).unwrap();
    let within = compile_plate_irs(&plates, &[(1, Vec2::new(80.0, 80.0))]).unwrap();
    let crossed = compile_plate_irs(&plates, &[(1, Vec2::new(120.0, 80.0))]).unwrap();
    assert_eq!(before[&10].nodes, within[&10].nodes);
    assert_eq!(before[&20].nodes, within[&20].nodes);
    assert_ne!(before[&10].nodes, crossed[&10].nodes);
    assert_eq!(before[&20].nodes, crossed[&20].nodes);
}
