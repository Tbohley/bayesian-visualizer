use super::*;
#[test]
fn inference_hides_toolbar_and_pencil_restores_editing_without_panels() {
    use bevy::picking::{backend::HitData, pointer::PointerButton};
    let mut app = test_app();
    app.add_observer(crate::data_vis::close_histogram_panel);
    let toolbar = app
        .world_mut()
        .query_filtered::<Entity, With<Toolbar>>()
        .single(app.world())
        .unwrap();
    let pencil = button(&mut app, Action::Edit);
    let selected = app
        .world_mut()
        .spawn((
            GraphNode(1),
            Selected,
            Transform::from_xyz(0., 0., 1.),
            ScalarNode {
                val: 1.,
                name: None,
            },
        ))
        .id();
    app.world_mut()
        .spawn((crate::sidebar::LocalSidebar, Node::default()));
    app.world_mut()
        .spawn((crate::data_vis::InferenceHistogramPanel, Node::default()));
    app.world_mut().trigger(InspectModel);
    app.update();
    assert_eq!(
        app.world().get::<Node>(toolbar).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world().get::<Node>(pencil).unwrap().display,
        Display::Flex
    );
    pointer(
        &mut app,
        pencil,
        Vec2::new(SIDEBAR_WIDTH + 20., 20.),
        Click {
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
            duration: std::time::Duration::from_millis(50),
            count: 1,
        },
    );
    app.update();
    assert!(!app.world().resource::<EditorView>().inspecting);
    assert_eq!(
        app.world().get::<Node>(toolbar).unwrap().display,
        Display::Flex
    );
    assert_eq!(
        app.world().get::<Node>(pencil).unwrap().display,
        Display::None
    );
    assert!(app.world().get::<ScalarNode>(selected).is_some());
    assert!(app.world().get::<Selected>(selected).is_none());
    let mut panels = app.world_mut().query_filtered::<Entity, Or<(
        With<crate::sidebar::LocalSidebar>,
        With<crate::data_vis::InferenceHistogramPanel>,
    )>>();
    assert_eq!(panels.iter(app.world()).count(), 0);
}

#[test]
fn automatic_node_inspection_selects_node_and_opens_matching_sidebar() {
    #[derive(Resource, Default)]
    struct OpenedHistogram(Option<crate::data_vis::HistogramSubject>);
    let mut app = test_app();
    app.init_resource::<OpenedHistogram>()
        .add_observer(crate::sidebar::reload_sidebar)
        .add_observer(
            |event: On<crate::data_vis::OpenHistogramPanel>,
             mut opened: ResMut<OpenedHistogram>| {
                opened.0 = Some(event.subject);
            },
        );
    let node = app
        .world_mut()
        .spawn((
            GraphNode(7),
            Transform::from_xyz(0., 0., 1.),
            RandomNode {
                name: Some("theta".into()),
                dist_type: "Normal".into(),
                params: vec![ParamValue("mean", None), ParamValue("std_dev", None)],
            },
        ))
        .id();
    app.world_mut()
        .trigger(crate::nodes::OpenNodeInspection { node_id: 7 });
    app.world_mut().flush();
    assert!(app.world().get::<Selected>(node).is_some());
    let mut sidebars = app
        .world_mut()
        .query_filtered::<Entity, With<crate::sidebar::LocalSidebar>>();
    assert_eq!(sidebars.iter(app.world()).count(), 1);
    assert_eq!(
        app.world().resource::<OpenedHistogram>().0,
        Some(crate::data_vis::HistogramSubject::Node(7))
    );
}

#[test]
fn zoom_keeps_the_pointer_world_position_fixed_even_at_limits() {
    let mut projection = Projection::Orthographic(OrthographicProjection::default_2d());
    let mut t = Transform::from_xyz(120., -35., 0.);
    let viewport = Vec2::new(1200., 800.);
    let pointer = Vec2::new(930., 250.);
    let offset = Vec2::new(pointer.x - viewport.x / 2., viewport.y / 2. - pointer.y);
    let anchor = t.translation.truncate() + offset * camera_scale(&projection);
    for factor in [0.5, 1000., 0.00001, 1.2] {
        apply_zoom(&mut projection, &mut t, viewport, pointer, factor);
        let new_anchor = t.translation.truncate() + offset * camera_scale(&projection);
        assert!(new_anchor.distance(anchor) < 0.001);
        assert!((0.1..=8.).contains(&camera_scale(&projection)));
    }
}
#[test]
fn editor_systems_initialize_and_run_without_query_conflicts() {
    let mut app = test_app();
    assert_eq!(app.world().resource::<Editor>().tool, Tool::Select);
    let mut toolbar = app.world_mut().query_filtered::<&Node, With<Toolbar>>();
    assert_eq!(toolbar.iter(app.world()).count(), 1);
}
fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        EditorPlugin,
    ))
    .init_resource::<Assets<Mesh>>()
    .init_resource::<Assets<ColorMaterial>>()
    .init_asset::<Image>()
    .init_resource::<ReducedView>()
    .init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<InputFocus>()
    .add_message::<MouseWheel>();
    app.world_mut().spawn(Window::default());
    app.world_mut().spawn(Camera2d);
    app.world_mut()
        .spawn((Canvas, NodeMode(NodeType::Random), Transform::default()));
    app.add_observer(crate::sidebar::on_trigger_close_menus);
    app.update();
    app.update();
    app
}
fn pointer<E: std::fmt::Debug + Clone + bevy::reflect::Reflect>(
    app: &mut App,
    entity: Entity,
    position: Vec2,
    event: E,
) {
    use bevy::{
        camera::NormalizedRenderTarget,
        picking::pointer::{Location, PointerId},
        window::WindowRef,
    };
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<Window>>()
        .single(app.world())
        .unwrap();
    let location = Location {
        target: NormalizedRenderTarget::Window(WindowRef::Entity(window).normalize(None).unwrap()),
        position,
    };
    app.world_mut()
        .trigger(Pointer::new(PointerId::Mouse, location, event, entity));
    app.world_mut().flush();
}
fn button(app: &mut App, action: Action) -> Entity {
    let mut q = app.world_mut().query::<(Entity, &ToolButton)>();
    q.iter(app.world())
        .find(|(_, b)| b.action == action)
        .unwrap()
        .0
}
#[test]
fn type_picker_dismisses_hint_and_updates_the_creation_type() {
    use bevy::picking::{backend::HitData, pointer::PointerButton};
    let mut app = test_app();
    let picker = button(&mut app, Action::NodeType);
    *app.world_mut().get_mut::<Interaction>(picker).unwrap() = Interaction::Hovered;
    app.update();
    let mut visibility = app
        .world_mut()
        .query_filtered::<&Visibility, With<ToolbarTip>>();
    assert_eq!(
        *visibility.single(app.world()).unwrap(),
        Visibility::Visible
    );
    let hit = HitData::new(Entity::PLACEHOLDER, 0., None, None);
    pointer(
        &mut app,
        picker,
        Vec2::new(600., 600.),
        Press {
            button: PointerButton::Primary,
            hit: hit.clone(),
            count: 1,
        },
    );
    app.update();
    assert_eq!(*visibility.single(app.world()).unwrap(), Visibility::Hidden);
    app.world_mut().trigger(crate::sidebar::OpenNodeTypeMenu {
        pos: Vec2::new(600., 400.),
    });
    app.world_mut().flush();
    app.update();
    let mut options = app.world_mut().query::<(Entity, &NodeTypeOption)>();
    assert_eq!(options.iter(app.world()).count(), 4);
    let constant = options
        .iter(app.world())
        .find(|(_, o)| o.0 == NodeType::Scalar)
        .unwrap()
        .0;
    pointer(
        &mut app,
        constant,
        Vec2::new(600., 450.),
        Click {
            button: PointerButton::Primary,
            hit,
            duration: std::time::Duration::from_millis(50),
            count: 1,
        },
    );
    let mut mode = app.world_mut().query::<&NodeMode>();
    assert_eq!(mode.single(app.world()).unwrap().0, NodeType::Scalar);
    let mut menus = app
        .world_mut()
        .query_filtered::<Entity, With<NodeTypeMenu>>();
    assert_eq!(menus.iter(app.world()).count(), 0);
}
#[test]
fn dragging_creates_one_default_node_and_escape_cancels_the_next_drag() {
    use bevy::picking::{backend::HitData, pointer::PointerButton};
    let mut app = test_app();
    let mut modes = app.world_mut().query::<&mut NodeMode>();
    modes.single_mut(app.world_mut()).unwrap().0 = NodeType::Scalar;
    let adder = button(&mut app, Action::Tool(Tool::Node));
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<Camera2d>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .get_mut::<Camera>(camera)
        .unwrap()
        .computed
        .target_info = Some(bevy::camera::RenderTargetInfo {
        physical_size: UVec2::new(1280, 720),
        scale_factor: 1.,
    });
    app.world_mut()
        .get_mut::<Camera>(camera)
        .unwrap()
        .computed
        .clip_from_view = Mat4::IDENTITY;
    let start = || DragStart {
        button: PointerButton::Primary,
        hit: HitData::new(camera, 0., None, None),
    };
    let end = || DragEnd {
        button: PointerButton::Primary,
        distance: Vec2::new(200., -200.),
    };
    pointer(&mut app, adder, Vec2::new(600., 650.), start());
    assert_eq!(
        app.world().resource::<NodeDrag>().active,
        Some(NodeType::Scalar)
    );
    pointer(&mut app, adder, Vec2::new(800., 400.), end());
    let mut scalars = app.world_mut().query::<&ScalarNode>();
    assert_eq!(scalars.iter(app.world()).count(), 1);
    assert_eq!(scalars.single(app.world()).unwrap().val, 1.);
    pointer(&mut app, adder, Vec2::new(600., 650.), start());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert_eq!(app.world().resource::<NodeDrag>().active, None);
    pointer(&mut app, adder, Vec2::new(800., 400.), end());
    assert_eq!(scalars.iter(app.world()).count(), 1);
    assert!(!valid_drop(
        Vec2::new(800., 400.),
        Vec2::new(1280., 720.),
        true
    ));
    assert!(!valid_drop(
        Vec2::new(1300., 400.),
        Vec2::new(1280., 720.),
        false
    ));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    modes.single_mut(app.world_mut()).unwrap().0 = NodeType::Vector;
    pointer(&mut app, adder, Vec2::new(600., 650.), start());
    pointer(&mut app, adder, Vec2::new(800., 400.), end());
    let mut vectors = app.world_mut().query::<&VectorNode>();
    assert_eq!(vectors.single(app.world()).unwrap(), &VectorNode::default());
}
#[test]
fn outside_presses_close_the_type_picker_on_canvas_and_toolbar() {
    use bevy::picking::{backend::HitData, pointer::PointerButton};
    let mut app = test_app();
    let canvas = app
        .world_mut()
        .query_filtered::<Entity, With<Canvas>>()
        .single(app.world())
        .unwrap();
    let zoom = button(&mut app, Action::ZoomIn);
    let widget = app.world_mut().spawn(Node::default()).id();
    for target in [canvas, zoom, widget] {
        app.world_mut().trigger(crate::sidebar::OpenNodeTypeMenu {
            pos: Vec2::new(600., 400.),
        });
        app.world_mut().flush();
        let mut menus = app
            .world_mut()
            .query_filtered::<Entity, With<NodeTypeMenu>>();
        assert_eq!(menus.iter(app.world()).count(), 1);
        let option = app
            .world_mut()
            .query_filtered::<Entity, With<NodeTypeOption>>()
            .iter(app.world())
            .next()
            .unwrap();
        pointer(
            &mut app,
            option,
            Vec2::new(620., 420.),
            Press {
                button: PointerButton::Primary,
                hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
                count: 1,
            },
        );
        assert_eq!(
            menus.iter(app.world()).count(),
            1,
            "pressing inside should keep the menu open"
        );
        pointer(
            &mut app,
            target,
            Vec2::new(900., 300.),
            Press {
                button: PointerButton::Primary,
                hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
                count: 1,
            },
        );
        assert_eq!(
            menus.iter(app.world()).count(),
            0,
            "outside press should dismiss the menu"
        );
    }
}

#[test]
fn toolbar_docks_above_panels_and_dragging_stays_within_canvas() {
    let mut app = test_app();
    let palette = app
        .world_mut()
        .query_filtered::<Entity, With<Toolbar>>()
        .single(app.world())
        .unwrap();
    let window = app
        .world_mut()
        .query_filtered::<&Window, ()>()
        .single(app.world())
        .unwrap();
    let size = Vec2::new(window.width(), window.height());
    assert!(matches!(app.world().get::<Node>(palette).unwrap().top,Val::Px(y) if y>12.));
    app.world_mut().resource_mut::<PalettePosition>().offset = Vec2::splat(50.);
    let sidebar = app
        .world_mut()
        .spawn((
            crate::sidebar::LocalSidebar,
            Node {
                width: px(SIDEBAR_WIDTH),
                ..default()
            },
        ))
        .id();
    app.update();
    assert_eq!(app.world().get::<Node>(palette).unwrap().top, px(12));
    let histogram = app
        .world_mut()
        .spawn((
            crate::data_vis::InferenceHistogramPanel,
            Node {
                height: px(crate::data_vis::HISTOGRAM_PANEL_HEIGHT),
                ..default()
            },
        ))
        .id();
    app.update();
    app.world_mut().resource_mut::<PalettePosition>().offset = Vec2::splat(10000.);
    app.update();
    let node = app.world().get::<Node>(palette).unwrap();
    assert!(matches!(node.left,Val::Px(x) if x>=SIDEBAR_WIDTH+12. && x<=size.x-SIDEBAR_WIDTH-12.));
    assert!(
        matches!(node.top,Val::Px(y) if y>=12. && y+58.<=size.y-crate::data_vis::HISTOGRAM_PANEL_HEIGHT-12.)
    );
    app.world_mut().despawn(sidebar);
    app.world_mut().despawn(histogram);
    app.update();
    assert!(matches!(app.world().get::<Node>(palette).unwrap().top,Val::Px(y) if y>12.));
}
#[test]
fn shift_alone_neither_starts_links_nor_shows_the_link_cursor() {
    use bevy::{
        picking::{backend::HitData, pointer::PointerButton},
        window::CursorIcon,
    };
    let mut app = test_app();
    let source = app
        .world_mut()
        .spawn((
            GraphNode(1),
            ScalarNode {
                val: 1.,
                name: None,
            },
            Transform::from_xyz(0., 0., 1.),
        ))
        .observe(crate::nodes::on_node_click)
        .id();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    app.update();
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<Window>>()
        .single(app.world())
        .unwrap();
    assert!(matches!(
        app.world().get::<CursorIcon>(window),
        Some(CursorIcon::System(SystemCursorIcon::Default))
    ));
    let click = || Click {
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
        duration: std::time::Duration::from_millis(50),
        count: 1,
    };
    pointer(&mut app, source, Vec2::new(700., 300.), click());
    let mut links = app
        .world_mut()
        .query_filtered::<Entity, With<UnfinishedLink>>();
    assert_eq!(links.iter(app.world()).count(), 0);
    app.world_mut().resource_mut::<Editor>().tool = Tool::Edge;
    pointer(&mut app, source, Vec2::new(700., 300.), click());
    assert_eq!(links.iter(app.world()).count(), 1);
}
#[test]
fn fit_reserves_identical_space_with_panels_closed_and_open() {
    use bevy::picking::{backend::HitData, pointer::PointerButton};
    let mut app = test_app();
    let fit = button(&mut app, Action::Fit);
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<Camera2d>>()
        .single(app.world())
        .unwrap();
    app.world_mut().spawn((
        GraphNode(1),
        ScalarNode {
            val: 1.,
            name: None,
        },
        Transform::from_xyz(100., 200., 1.),
    ));
    let click = || Click {
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
        duration: std::time::Duration::from_millis(50),
        count: 1,
    };
    pointer(&mut app, fit, Vec2::new(900., 650.), click());
    let closed = (
        *app.world().get::<Transform>(camera).unwrap(),
        camera_scale(app.world().get::<Projection>(camera).unwrap()),
    );
    app.world_mut().spawn((
        crate::sidebar::LocalSidebar,
        Node {
            width: px(SIDEBAR_WIDTH),
            ..default()
        },
    ));
    app.world_mut().spawn((
        crate::data_vis::InferenceHistogramPanel,
        Node {
            height: px(crate::data_vis::HISTOGRAM_PANEL_HEIGHT),
            ..default()
        },
    ));
    pointer(&mut app, fit, Vec2::new(900., 650.), click());
    let open = (
        *app.world().get::<Transform>(camera).unwrap(),
        camera_scale(app.world().get::<Projection>(camera).unwrap()),
    );
    assert_eq!(closed, open);
}
#[test]
fn fit_centers_and_contains_model_in_the_reserved_canvas() {
    let mut projection = OrthographicProjection::default_2d();
    let mut transform = Transform::from_xyz(0., 0., 7.);
    let window = Vec2::new(1280., 800.);
    let min = Vec2::new(-200., -100.);
    let max = Vec2::new(400., 200.);
    fit_model(&mut projection, &mut transform, min, max, window);
    let screen = |point: Vec2| {
        let relative = (point - transform.translation.truncate()) / projection.scale;
        window / 2. + Vec2::new(relative.x, -relative.y)
    };
    assert!(screen((min + max) / 2.).distance(Vec2::new(640., 270.)) < 0.001);
    for point in [min, max] {
        let p = screen(point);
        assert!(p.x >= 340. - 0.001 && p.x <= 940. + 0.001);
        assert!(p.y >= 40. - 0.001 && p.y <= 500. + 0.001);
    }
    assert_eq!(transform.translation.z, 7.);
}
