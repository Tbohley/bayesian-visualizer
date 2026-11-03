use super::*;
use crate::{
    bayesian_core::{NodeIR, ScalarType, Value, ValueType},
    bevy_to_fugue::{GraphIRResource, TriggerCompilation},
    ui::history::{HistoryPlugin, HistoryRequest},
};

#[derive(Resource, Default)]
struct Errors(Vec<String>);

fn test_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        HistoryPlugin,
    ))
    .init_resource::<Assets<Mesh>>()
    .init_resource::<Assets<ColorMaterial>>()
    .init_resource::<InputFocus>()
    .init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<Errors>()
    .add_observer(reload_sidebar)
    .add_observer(apply_vector_values)
    .add_observer(set_node_name)
    .add_observer(crate::bevy_to_fugue::compilation::compile)
    .add_observer(
        |event: On<crate::ui::ErrorToast>, mut errors: ResMut<Errors>| {
            errors.0.push(event.text.clone());
        },
    )
    .add_systems(
        Update,
        (
            on_vector_enter,
            global::invalidate_compilation_on_graph_change,
        ),
    );
    let vector = app
        .world_mut()
        .spawn((
            GraphNode(1),
            Transform::from_xyz(30., 30., 1.),
            VectorNode::default(),
            Selected,
        ))
        .id();
    app.world_mut()
        .spawn((ChildOf(vector), NodeLabel, Text2d::new("initial")));
    app.update();
    app.world_mut().trigger(ReloadSidebar);
    app.world_mut().flush();
    (app, vector)
}

fn apply(app: &mut App, entity: Entity, text: &str, element: ScalarType) {
    app.world_mut().trigger(ApplyVectorValues {
        node: entity,
        text: text.into(),
        element,
    });
    app.world_mut().flush();
    app.update();
}

#[test]
fn sidebar_edits_resize_compile_and_invalidate_the_model() {
    let (mut app, vector) = test_app();
    apply(
        &mut app,
        vector,
        "2, -3, 9007199254740993, 4",
        ScalarType::Integer,
    );
    app.world_mut().trigger(SetNodeName {
        entity: vector,
        name: "weights".into(),
    });
    app.world_mut().flush();
    app.update();
    assert_eq!(
        app.world().get::<VectorNode>(vector).unwrap().label(),
        "weights"
    );
    let mut fields = app
        .world_mut()
        .query::<(&VectorValuesTextbox, &EditableText)>();
    let (field, text) = fields.single(app.world()).unwrap();
    assert_eq!(field.element, ScalarType::Integer);
    assert_eq!(text.value(), "2, -3, 9007199254740993, 4");
    app.world_mut().trigger(TriggerCompilation);
    app.world_mut().flush();
    let compiled = &app.world().resource::<GraphIRResource>().0;
    assert_eq!(
        compiled.node_shapes()[&1].value_type,
        ValueType::Vector {
            element: ScalarType::Integer,
            length: 4
        }
    );
    let NodeIR::Constant { value } = &compiled.graph().nodes[&1] else {
        panic!("expected typed constant")
    };
    assert_eq!(
        value,
        &Value::IntegerVector(vec![2, -3, 9_007_199_254_740_993, 4].into())
    );
    apply(&mut app, vector, "0.25, 0.75", ScalarType::Real);
    assert!(!app.world().contains_resource::<GraphIRResource>());
    assert_eq!(
        app.world().get::<VectorNode>(vector).unwrap().elements,
        VectorElements::Real(vec![0.25, 0.75])
    );
}

#[test]
fn invalid_edits_preserve_values_and_compilation() {
    let (mut app, vector) = test_app();
    app.world_mut().trigger(TriggerCompilation);
    app.world_mut().flush();
    for (text, element) in [
        ("", ScalarType::Real),
        ("1, NaN", ScalarType::Real),
        ("1, inf", ScalarType::Real),
        ("1,", ScalarType::Real),
        ("[1, 2]", ScalarType::Real),
        ("1.5, 2", ScalarType::Integer),
        ("9223372036854775808", ScalarType::Integer),
    ] {
        apply(&mut app, vector, text, element);
        assert_eq!(
            app.world().get::<VectorNode>(vector).unwrap(),
            &VectorNode::default()
        );
        assert!(app.world().contains_resource::<GraphIRResource>());
    }
    assert!(
        app.world()
            .resource::<Errors>()
            .0
            .iter()
            .any(|text| text.contains("Element 2"))
    );
}

#[test]
fn enter_applies_only_the_focused_vector_editor() {
    let (mut app, vector) = test_app();
    let mut fields = app
        .world_mut()
        .query_filtered::<Entity, With<VectorValuesTextbox>>();
    let field = fields.single(app.world()).unwrap();
    *app.world_mut().get_mut::<EditableText>(field).unwrap() = EditableText::new("0.2, 0.3, 0.5");
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(field, bevy::input_focus::FocusCause::Navigated);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    assert_eq!(
        app.world().get::<VectorNode>(vector).unwrap().elements,
        VectorElements::Real(vec![0.2, 0.3, 0.5])
    );
}

#[test]
fn undo_redo_preserves_vector_types_names_and_link_references() {
    let (mut app, vector) = test_app();
    let compute = app
        .world_mut()
        .spawn((
            GraphNode(2),
            Transform::from_xyz(100., 30., 1.),
            ComputeNode {
                operation: Operation::Add,
                params: vec![
                    ParamValue("first", Some(vector)),
                    ParamValue("second", Some(vector)),
                ],
            },
        ))
        .id();
    app.world_mut().spawn(GraphLink {
        from: vector,
        to: Some(compute),
    });
    app.update();
    app.world_mut().trigger(SetNodeName {
        entity: vector,
        name: "alpha".into(),
    });
    app.world_mut().flush();
    apply(&mut app, vector, "2, 3", ScalarType::Integer);
    app.world_mut().resource_mut::<HistoryRequest>().0 = Some(false);
    app.update();
    let mut vectors = app.world_mut().query::<(Entity, &VectorNode)>();
    let (restored, node) = vectors.single(app.world()).unwrap();
    assert_eq!(node.elements, VectorElements::Real(vec![1.0; 3]));
    assert!(app.world().get::<ComputeNode>(compute).is_none());
    let mut computes = app.world_mut().query::<&ComputeNode>();
    assert!(
        computes
            .single(app.world())
            .unwrap()
            .params
            .iter()
            .all(|p| p.1 == Some(restored))
    );
    app.world_mut().resource_mut::<HistoryRequest>().0 = Some(true);
    app.update();
    let mut vectors = app.world_mut().query::<&VectorNode>();
    let node = vectors.single(app.world()).unwrap();
    assert_eq!(node.elements, VectorElements::Integer(vec![2, 3]));
    assert_eq!(node.name.as_deref(), Some("alpha"));
}
#[test]
fn changing_length_revalidates_linked_operations_on_compilation() {
    let (mut app, vector) = test_app();
    let other = app
        .world_mut()
        .spawn((
            GraphNode(2),
            Transform::from_xyz(100., 30., 1.),
            VectorNode::default(),
        ))
        .id();
    app.world_mut().spawn((
        GraphNode(3),
        Transform::from_xyz(65., 80., 1.),
        ComputeNode {
            operation: Operation::Multiply,
            params: vec![
                ParamValue("first", Some(vector)),
                ParamValue("second", Some(other)),
            ],
        },
    ));
    app.update();
    app.world_mut().trigger(TriggerCompilation);
    app.world_mut().flush();
    assert!(app.world().contains_resource::<GraphIRResource>());
    apply(&mut app, vector, "1, 2", ScalarType::Real);
    app.world_mut().trigger(TriggerCompilation);
    app.world_mut().flush();
    assert!(!app.world().contains_resource::<GraphIRResource>());
    assert!(
        app.world().resource::<Errors>().0.iter().any(
            |message| message.contains("node 3") && message.contains("matching vector lengths")
        )
    );
    apply(&mut app, other, "3, 4", ScalarType::Real);
    app.world_mut().trigger(TriggerCompilation);
    app.world_mut().flush();
    assert!(app.world().contains_resource::<GraphIRResource>());
}
