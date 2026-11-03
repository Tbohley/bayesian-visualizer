use super::*;
fn model(value: f64) -> Snapshot {
    Snapshot {
        nodes: vec![(1, Vec3::ZERO, Value::Scalar(value, None))],
        links: vec![],
    }
}
#[test]
fn undo_redo_and_branching() {
    let mut h = History::default();
    h.commit(model(1.));
    h.commit(model(2.));
    assert_eq!(h.step(false), Some(model(1.)));
    assert_eq!(h.step(true), Some(model(2.)));
    h.step(false);
    h.commit(model(3.));
    assert!(!h.can_redo());
    assert_eq!(h.step(false), Some(model(1.)));
}
#[test]
fn no_op_does_not_consume_history_or_clear_redo() {
    let mut h = History::default();
    h.commit(model(1.));
    h.commit(model(2.));
    h.step(false);
    h.commit(model(1.));
    assert_eq!(h.past.len(), 1);
    assert!(h.can_redo());
}
fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        HistoryPlugin,
    ))
    .init_resource::<Assets<Mesh>>()
    .init_resource::<Assets<ColorMaterial>>()
    .init_resource::<InputFocus>()
    .init_resource::<ButtonInput<MouseButton>>();
    app
}
#[test]
fn restores_entity_references_in_parameters_links_and_observations() {
    let mut app = test_app();
    let a = app
        .world_mut()
        .spawn((
            GraphNode(1),
            Transform::from_xyz(10., 10., 1.),
            ScalarNode {
                val: 2.,
                name: None,
            },
        ))
        .id();
    let b = app
        .world_mut()
        .spawn((
            GraphNode(2),
            Transform::from_xyz(50., 10., 1.),
            ComputeNode {
                operation: Operation::Add,
                params: vec![ParamValue("a", Some(a)), ParamValue("b", Some(a))],
            },
        ))
        .id();
    app.world_mut().spawn(GraphLink {
        from: a,
        to: Some(b),
    });
    app.world_mut().spawn((
        GraphNode(3),
        Transform::from_xyz(30., 10., crate::PLATE_Z),
        Plate {
            origin: Vec2::ZERO,
            bounds: PlateBounds::from_points(Vec2::ZERO, Vec2::splat(80.)),
            data: Dataset {
                name: "example".into(),
                n: 1,
                data: HashMap::from([("x".into(), vec![4.])]),
            },
            mapping: HashMap::from([(a, "x".into())]),
        },
    ));
    app.update();
    let expected = capture(app.world_mut());
    app.world_mut().resource_mut::<HistoryRequest>().0 = Some(false);
    app.update();
    assert!(capture(app.world_mut()).nodes.is_empty());
    app.world_mut().resource_mut::<HistoryRequest>().0 = Some(true);
    app.update();
    assert_eq!(capture(app.world_mut()), expected);
    assert!(app.world().get_entity(a).is_err());
    // A quiet frame after restoration must preserve redo/undo and stable IDs.
    app.update();
    assert_eq!(app.world().resource::<History>().past.len(), 1);
}
#[test]
fn one_drag_is_one_undo_and_camera_moves_are_excluded() {
    let mut app = test_app();
    let a = app
        .world_mut()
        .spawn((
            GraphNode(1),
            Transform::default(),
            ScalarNode {
                val: 2.,
                name: None,
            },
        ))
        .id();
    let camera = app.world_mut().spawn(Transform::default()).id();
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    for x in [10., 20., 30.] {
        app.world_mut()
            .get_mut::<Transform>(a)
            .unwrap()
            .translation
            .x = x;
        app.update();
    }
    assert_eq!(app.world().resource::<History>().past.len(), 1);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    assert_eq!(app.world().resource::<History>().past.len(), 2);
    app.world_mut()
        .get_mut::<Transform>(camera)
        .unwrap()
        .translation
        .x = 100.;
    app.update();
    assert_eq!(app.world().resource::<History>().past.len(), 2);
    app.world_mut().resource_mut::<HistoryRequest>().0 = Some(false);
    app.update();
    let snapshot = capture(app.world_mut());
    assert_eq!(snapshot.nodes[0].1, Vec3::ZERO);
    assert_eq!(
        app.world().get::<Transform>(camera).unwrap().translation.x,
        100.
    );
}
