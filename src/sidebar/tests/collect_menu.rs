use super::*;
use crate::{
    bayesian_core::{ScalarType, Value, ValueType, graph_checks::ModelResult},
    bevy_to_fugue::{GraphIRResource, TriggerCompilation},
    ui::history::{HistoryPlugin, HistoryRequest},
};
use bevy::{
    camera::NormalizedRenderTarget,
    input_focus::InputFocus,
    picking::{
        backend::HitData,
        events::{Click, Drag, DragEnd, DragStart, Press},
        pointer::{Location, PointerButton, PointerId},
    },
    window::WindowRef,
};

#[derive(Resource, Default)]
struct Errors(Vec<String>);

fn test_app() -> (App, Entity, Vec<Entity>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        HistoryPlugin,
    ))
    .init_resource::<Assets<Mesh>>()
    .init_resource::<Assets<ColorMaterial>>()
    .init_resource::<InputFocus>()
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<Errors>()
    .add_observer(edit_collect_inputs)
    .add_observer(reload_sidebar)
    .add_observer(on_trigger_close_menus)
    .add_observer(compute_menu::on_open_operation_menu)
    .add_observer(link_params::on_open_param_link_menu)
    .add_observer(crate::bevy_to_fugue::compilation::compile)
    .add_observer(
        |event: On<crate::ui::ErrorToast>, mut errors: ResMut<Errors>| {
            errors.0.push(event.text.clone())
        },
    )
    .add_systems(
        Update,
        (global::invalidate_compilation_on_graph_change, animate_drag),
    );
    app.world_mut().spawn(Window::default());
    let constants = (1..=3)
        .map(|id| {
            app.world_mut()
                .spawn((
                    GraphNode(id),
                    Transform::from_xyz(id as f32 * 60., 40., 1.),
                    ScalarNode {
                        val: id as f64,
                        name: None,
                    },
                ))
                .id()
        })
        .collect::<Vec<_>>();
    let collect = app
        .world_mut()
        .spawn((
            GraphNode(4),
            Transform::from_xyz(100., 100., 1.),
            Selected,
            ComputeNode {
                operation: Operation::Collect,
                params: constants
                    .iter()
                    .map(|&entity| ParamValue("element", Some(entity)))
                    .collect(),
            },
        ))
        .id();
    app.world_mut()
        .spawn((NodeLabel, Text2d::new("vec"), ChildOf(collect)));
    for &source in &constants {
        app.world_mut().spawn(GraphLink {
            from: source,
            to: Some(collect),
        });
    }
    app.update();
    app.world_mut().trigger(ReloadSidebar);
    app.world_mut().flush();
    (app, collect, constants)
}

fn pointer<E: std::fmt::Debug + Clone + bevy::reflect::Reflect>(
    app: &mut App,
    entity: Entity,
    event: E,
) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<Window>>()
        .single(app.world())
        .unwrap();
    let location = Location {
        target: NormalizedRenderTarget::Window(WindowRef::Entity(window).normalize(None).unwrap()),
        position: Vec2::new(20., 20.),
    };
    app.world_mut()
        .trigger(Pointer::new(PointerId::Mouse, location, event, entity));
    app.world_mut().flush();
}
fn click(app: &mut App, name: &str) {
    let mut names = app.world_mut().query::<(Entity, &Name)>();
    let button = names
        .iter(app.world())
        .find(|(_, candidate)| candidate.as_str() == name)
        .unwrap()
        .0;
    pointer(
        app,
        button,
        Click {
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
            duration: std::time::Duration::from_millis(50),
            count: 1,
        },
    );
    app.update();
}
fn layout_cards(app: &mut App) {
    let cards = app
        .world_mut()
        .query::<(Entity, &CollectInputRow)>()
        .iter(app.world())
        .map(|(entity, row)| (entity, row.index))
        .collect::<Vec<_>>();
    for (entity, index) in cards {
        app.world_mut().entity_mut(entity).insert((
            ComputedNode {
                size: Vec2::new(268., 92.),
                inverse_scale_factor: 1.,
                ..default()
            },
            UiGlobalTransform::from_xy(150., 100. + index as f32 * 100.),
        ));
    }
}

fn drag_input(app: &mut App, from: usize, to: usize) {
    layout_cards(app);
    let handle = app
        .world_mut()
        .query::<(Entity, &Name)>()
        .iter(app.world())
        .find(|(_, n)| n.as_str() == format!("collect_drag_{from}"))
        .unwrap()
        .0;
    let distance = Vec2::new(0., (to as f32 - from as f32) * 100.);
    pointer(
        app,
        handle,
        DragStart {
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
        },
    );
    pointer(
        app,
        handle,
        Drag {
            button: PointerButton::Primary,
            distance,
            delta: distance,
        },
    );
    app.update();
    pointer(
        app,
        handle,
        DragEnd {
            button: PointerButton::Primary,
            distance,
        },
    );
    app.update();
}

fn compile(app: &mut App) {
    app.world_mut().trigger(TriggerCompilation);
    app.world_mut().flush();
}
fn output(app: &App) -> Value {
    let values = app
        .world()
        .resource::<GraphIRResource>()
        .0
        .graph()
        .ancestral_sample()
        .unwrap();
    let ModelResult::Vector(value) = values[&4].clone() else {
        panic!("expected vector")
    };
    value
}
fn sources(app: &mut App) -> Vec<u32> {
    let mut nodes = app.world_mut().query::<(&GraphNode, &ComputeNode)>();
    let node = nodes.iter(app.world()).find(|(id, _)| id.0 == 4).unwrap().1;
    node.params
        .iter()
        .map(|param| app.world().get::<GraphNode>(param.1.unwrap()).unwrap().0)
        .collect()
}

#[test]
fn collect_sidebar_adds_reorders_links_and_removes_elements() {
    let (mut app, collect, constants) = test_app();
    compile(&mut app);
    assert_eq!(output(&app), Value::RealVector(vec![1., 2., 3.].into()));
    drag_input(&mut app, 0, 1);
    assert!(!app.world().contains_resource::<GraphIRResource>());
    compile(&mut app);
    assert_eq!(output(&app), Value::RealVector(vec![2., 1., 3.].into()));
    click(&mut app, "add_collect_input");
    compile(&mut app);
    assert!(!app.world().contains_resource::<GraphIRResource>());
    assert!(
        app.world()
            .resource::<Errors>()
            .0
            .iter()
            .any(|error| error.contains("node 4") && error.contains("parameter 4"))
    );
    app.world_mut().trigger(OpenParamLinkMenu {
        pos: Vec2::ZERO,
        param_num: 3,
    });
    app.world_mut().flush();
    let mut items = app.world_mut().query::<(Entity, &ParamMenuItem)>();
    let item = items
        .iter(app.world())
        .find(|(_, item)| item.entity == constants[0])
        .unwrap()
        .0;
    pointer(
        &mut app,
        item,
        Press {
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
            count: 1,
        },
    );
    app.update();
    compile(&mut app);
    assert_eq!(output(&app), Value::RealVector(vec![2., 1., 3., 1.].into()));
    assert_eq!(
        app.world().resource::<GraphIRResource>().0.node_shapes()[&4].value_type,
        ValueType::Vector {
            element: ScalarType::Real,
            length: 4
        }
    );
    click(&mut app, "collect_Remove_1");
    let mut links = app.world_mut().query::<&GraphLink>();
    assert!(
        links
            .iter(app.world())
            .any(|link| link.from == constants[0] && link.to == Some(collect))
    );
    click(&mut app, "collect_Remove_2");
    assert!(
        !links
            .iter(app.world())
            .any(|link| link.from == constants[0] && link.to == Some(collect))
    );
    compile(&mut app);
    assert_eq!(output(&app), Value::RealVector(vec![2., 3.].into()));
}

#[test]
fn undo_redo_preserves_collect_order_and_restores_entity_references() {
    let (mut app, _, _) = test_app();
    drag_input(&mut app, 2, 1);
    assert_eq!(sources(&mut app), vec![1, 3, 2]);
    app.world_mut().resource_mut::<HistoryRequest>().0 = Some(false);
    app.update();
    assert_eq!(sources(&mut app), vec![1, 2, 3]);
    app.world_mut().resource_mut::<HistoryRequest>().0 = Some(true);
    app.update();
    assert_eq!(sources(&mut app), vec![1, 3, 2]);
    compile(&mut app);
    assert_eq!(output(&app), Value::RealVector(vec![1., 3., 2.].into()));
}

#[test]
fn operation_picker_selects_all_new_operations_and_configures_inputs() {
    let (mut app, collect, _) = test_app();
    for operation in [
        Operation::Sum,
        Operation::Product,
        Operation::Logit,
        Operation::InverseLogit,
        Operation::Normalize,
        Operation::Softmax,
        Operation::Collect,
    ] {
        app.world_mut()
            .trigger(OpenOperationMenu { pos: Vec2::ZERO });
        app.world_mut().flush();
        let mut items = app.world_mut().query::<(Entity, &ContextMenuItem)>();
        assert_eq!(items.iter(app.world()).count(), Operation::ALL.len());
        let item = items
            .iter(app.world())
            .find(|(_, item)| item.0 == operation.name())
            .unwrap()
            .0;
        pointer(
            &mut app,
            item,
            Press {
                button: PointerButton::Primary,
                hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
                count: 1,
            },
        );
        app.update();
        let node = app.world().get::<ComputeNode>(collect).unwrap();
        assert_eq!(node.operation, operation);
        assert_eq!(
            node.params.len(),
            if operation == Operation::Collect {
                2
            } else {
                1
            }
        );
    }
}

#[test]
fn collect_never_removes_its_last_input() {
    let (mut app, collect, _) = test_app();
    for _ in 0..4 {
        app.world_mut().trigger(EditCollectInputs {
            node: collect,
            edit: CollectInputEdit::Remove(0),
        });
        app.world_mut().flush();
    }
    assert_eq!(
        app.world()
            .get::<ComputeNode>(collect)
            .unwrap()
            .params
            .len(),
        1
    );
    compile(&mut app);
    assert_eq!(output(&app), Value::RealVector(vec![3.].into()));
}

#[test]
fn dragging_across_multiple_elements_preserves_order_and_links() {
    let (mut app, _, _) = test_app();
    drag_input(&mut app, 0, 2);
    assert_eq!(sources(&mut app), vec![2, 3, 1]);
    drag_input(&mut app, 2, 0);
    assert_eq!(sources(&mut app), vec![1, 2, 3]);
    let mut links = app.world_mut().query::<&GraphLink>();
    assert_eq!(links.iter(app.world()).count(), 3);
    compile(&mut app);
    assert_eq!(output(&app), Value::RealVector(vec![1., 2., 3.].into()));
}

#[test]
fn releasing_a_drag_outside_the_list_does_not_change_inputs() {
    let (mut app, _, _) = test_app();
    layout_cards(&mut app);
    let handle = app
        .world_mut()
        .query::<(Entity, &Name)>()
        .iter(app.world())
        .find(|(_, n)| n.as_str() == "collect_drag_0")
        .unwrap()
        .0;
    pointer(
        &mut app,
        handle,
        DragStart {
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
        },
    );
    pointer(
        &mut app,
        handle,
        DragEnd {
            button: PointerButton::Primary,
            distance: Vec2::new(1000., 200.),
        },
    );
    app.update();
    assert_eq!(sources(&mut app), vec![1, 2, 3]);
}

#[test]
fn dragged_card_follows_pointer_while_neighbors_slide_without_changing_the_model() {
    let (mut app, _, _) = test_app();
    layout_cards(&mut app);
    let handle = app
        .world_mut()
        .query::<(Entity, &Name)>()
        .iter(app.world())
        .find(|(_, n)| n.as_str() == "collect_drag_0")
        .unwrap()
        .0;
    pointer(
        &mut app,
        handle,
        DragStart {
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
        },
    );
    pointer(
        &mut app,
        handle,
        Drag {
            button: PointerButton::Primary,
            distance: Vec2::new(15., 180.),
            delta: Vec2::new(15., 180.),
        },
    );
    app.update();
    assert_eq!(sources(&mut app), vec![1, 2, 3]);
    let drag = app.world().resource::<CollectDrag>();
    assert_eq!(drag.to, 1);
    let source = drag.slots[0].entity;
    let neighbor = drag.slots[1].entity;
    let transform = app.world().get::<UiTransform>(source).unwrap();
    assert_eq!(transform.translation, Val2::px(15., 180.));
    assert_eq!(app.world().get::<GlobalZIndex>(source).unwrap().0, 1200);
    assert!(
        matches!(app.world().get::<UiTransform>(neighbor).unwrap().translation.y, Val::Px(y) if y < 0.)
    );
    pointer(
        &mut app,
        handle,
        DragEnd {
            button: PointerButton::Primary,
            distance: Vec2::new(15., 180.),
        },
    );
    app.update();
    assert_eq!(sources(&mut app), vec![2, 1, 3]);
    assert!(!app.world().contains_resource::<CollectDrag>());
}
