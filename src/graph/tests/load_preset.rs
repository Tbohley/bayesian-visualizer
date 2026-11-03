use super::*;
use crate::bayesian_core::{ScalarType, Value, ValueType, graph_checks::ModelResult};
use crate::bevy_to_fugue::TriggerCompilation;

#[test]
fn loading_a_vector_preset_preserves_links_types_and_plate_instances() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<ColorMaterial>>()
        .add_observer(crate::bevy_to_fugue::compilation::compile);
    let preset = GraphPreset {
        id: "vector",
        title: "Vector",
        description: "Vector test",
        study_condition: None,
        nodes: vec![
            PresetNode::Vector {
                id: 1,
                position: [25., 25.],
                elements: VectorElements::Integer(vec![2, 4]),
                name: Some("counts"),
            },
            PresetNode::Scalar {
                id: 2,
                position: [75., 25.],
                value: 2.,
                name: None,
            },
            PresetNode::Compute {
                id: 3,
                position: [50., 75.],
                operation: Operation::Divide,
                parameters: vec![
                    PresetParameter {
                        name: "dividend",
                        source: Some(1),
                    },
                    PresetParameter {
                        name: "divisor",
                        source: Some(2),
                    },
                ],
            },
        ],
        plates: vec![PresetPlate {
            id: 10,
            bounds: PresetBounds {
                min: [0., 0.],
                max: [100., 100.],
            },
            dataset_id: NO_DATASET,
            mapping: vec![],
        }],
    };
    let datasets = Datasets { datasets: vec![] };
    validate_preset(&preset, &datasets).unwrap();
    let mut meshes = app.world_mut().remove_resource::<Assets<Mesh>>().unwrap();
    let mut materials = app
        .world_mut()
        .remove_resource::<Assets<ColorMaterial>>()
        .unwrap();
    spawn_preset(
        &preset,
        &mut app.world_mut().commands(),
        &mut meshes,
        &mut materials,
        &datasets,
    );
    app.world_mut().insert_resource(meshes);
    app.world_mut().insert_resource(materials);
    app.world_mut().flush();
    let mut vectors = app.world_mut().query::<&VectorNode>();
    assert_eq!(
        vectors.single(app.world()).unwrap().name.as_deref(),
        Some("counts")
    );
    let mut links = app.world_mut().query::<&GraphLink>();
    assert_eq!(links.iter(app.world()).count(), 2);
    app.world_mut().trigger(TriggerCompilation);
    app.world_mut().flush();
    let compiled = &app.world().resource::<GraphIRResource>().0;
    assert_eq!(
        compiled.node_shapes()[&1].value_type,
        ValueType::Vector {
            element: ScalarType::Integer,
            length: 2
        }
    );
    assert_eq!(
        compiled.node_shapes()[&3].value_type,
        ValueType::Vector {
            element: ScalarType::Real,
            length: 2
        }
    );
    assert_eq!(compiled.node_shapes()[&3].plate_extents, vec![10]);
    assert_eq!(
        compiled.graph().ancestral_sample().unwrap()[&3],
        ModelResult::Plate(vec![
            ModelResult::Vector(Value::RealVector(
                vec![1., 2.].into()
            ));
            10
        ])
    );
}
