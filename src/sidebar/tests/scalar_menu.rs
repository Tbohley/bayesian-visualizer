use super::*;
use crate::ui::history::{HistoryPlugin, HistoryRequest};
#[test]
fn integer_type_toggle_rejects_fractional_values_and_survives_undo_redo() {
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
    .add_observer(toggle_scalar_type)
    .add_observer(reload_sidebar);
    let entity = app
        .world_mut()
        .spawn((
            GraphNode(1),
            Transform::default(),
            ScalarNode {
                val: 0.5,
                name: None,
            },
        ))
        .id();
    app.update();
    app.world_mut().trigger(ToggleScalarType(entity));
    app.world_mut().flush();
    assert!(app.world().get::<IntegerScalar>(entity).is_none());
    app.world_mut().get_mut::<ScalarNode>(entity).unwrap().val = 20.;
    app.update();
    app.world_mut().trigger(ToggleScalarType(entity));
    app.world_mut().flush();
    app.update();
    assert!(app.world().get::<IntegerScalar>(entity).is_some());
    app.world_mut().resource_mut::<HistoryRequest>().0 = Some(false);
    app.update();
    let mut nodes = app.world_mut().query::<(Entity, &ScalarNode)>();
    let restored = nodes.single(app.world()).unwrap().0;
    assert!(app.world().get::<IntegerScalar>(restored).is_none());
    app.world_mut().resource_mut::<HistoryRequest>().0 = Some(true);
    app.update();
    let mut nodes = app.world_mut().query::<(Entity, &ScalarNode)>();
    let (restored, value) = nodes.single(app.world()).unwrap();
    assert_eq!(value.val, 20.);
    assert!(app.world().get::<IntegerScalar>(restored).is_some());
}
