use super::*;
use bevy::{
    camera::NormalizedRenderTarget,
    picking::{
        backend::HitData,
        pointer::{Location, PointerButton, PointerId},
    },
    window::WindowRef,
};

fn press(app: &mut App, name: &str) {
    let entity = app
        .world_mut()
        .query::<(Entity, &Name)>()
        .iter(app.world())
        .find(|(_, candidate)| candidate.as_str() == name)
        .unwrap()
        .0;
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<Window>>()
        .single(app.world())
        .unwrap();
    app.world_mut().trigger(Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Entity(window).normalize(None).unwrap(),
            ),
            position: Vec2::new(10., 10.),
        },
        bevy::picking::events::Press {
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
            count: 1,
        },
        entity,
    ));
    app.world_mut().flush();
}

fn has_name(app: &mut App, name: &str) -> bool {
    app.world_mut()
        .query::<&Name>()
        .iter(app.world())
        .any(|candidate| candidate.as_str() == name)
}

#[test]
fn observation_starts_with_an_invitation_then_shows_the_dataset_and_column() {
    let mut app = App::new();
    app.add_observer(reload_sidebar)
        .add_observer(plate_menu::on_open_plate_mapping_menu)
        .add_observer(on_trigger_close_menus);
    app.world_mut().spawn(Window::default());
    let node = app
        .world_mut()
        .spawn((
            GraphNode(1),
            Selected,
            Transform::default(),
            ScalarNode {
                val: 1.,
                name: None,
            },
        ))
        .id();
    let plate = app
        .world_mut()
        .spawn((
            GraphNode(2),
            Plate {
                origin: Vec2::ZERO,
                bounds: PlateBounds {
                    min: Vec2::splat(-10.),
                    max: Vec2::splat(10.),
                },
                data: Dataset {
                    name: "measurements.csv".into(),
                    n: 2,
                    data: HashMap::from([("height".into(), vec![1., 2.])]),
                },
                mapping: HashMap::from([(node, "unobserved".into())]),
            },
        ))
        .id();
    app.world_mut().trigger(ReloadSidebar);
    app.world_mut().flush();
    assert!(has_name(&mut app, "observe_from_data"));
    assert!(!has_name(&mut app, "node_observation_selector"));
    assert!(!has_name(&mut app, "view_column_distribution"));
    press(&mut app, "observe_from_data");
    assert!(has_name(&mut app, "plate mapping selector"));
    press(&mut app, "item-height");
    assert_eq!(
        app.world().get::<Plate>(plate).unwrap().mapping[&node],
        "height"
    );
    assert!(!has_name(&mut app, "observe_from_data"));
    assert!(has_name(&mut app, "node_observation_selector"));
    assert!(has_name(&mut app, "Observed from data"));
    assert!(has_name(&mut app, "view_column_distribution"));
    let labels = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|t| t.0.clone())
        .collect::<Vec<_>>();
    assert!(labels.iter().any(|label| label == "from: measurements.csv"));
    assert!(labels.iter().any(|label| label == "height"));
    assert!(labels.iter().any(|label| label == "View distribution"));
    assert!(
        !labels
            .iter()
            .any(|label| label.contains("measurements.csv:") || label == "unobserved")
    );
    press(&mut app, "node_observation_selector");
    press(&mut app, "item-unobserved");
    assert!(has_name(&mut app, "observe_from_data"));
    assert!(!has_name(&mut app, "node_observation_selector"));
}
