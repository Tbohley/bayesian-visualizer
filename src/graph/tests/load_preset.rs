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

#[test]
fn discrete_presets_load_compile_and_complete_inference() {
    for preset in crate::graph::discrete_presets() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<ColorMaterial>>()
            .add_observer(crate::bevy_to_fugue::compilation::compile);
        let datasets = Datasets {
            datasets: crate::graph::discrete_datasets(),
        };
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
        app.world_mut().trigger(TriggerCompilation);
        app.world_mut().flush();
        let compiled = &app.world().resource::<GraphIRResource>().0;
        let result = compiled.run_inference(42, 6000, 1500).unwrap();
        assert_eq!(result.n_samples, 6000, "{}", preset.id);
        assert!(
            result
                .traces
                .iter()
                .all(|t| t.total_log_weight().is_finite())
        );
        if preset.id == "coin_binomial" {
            let mean = result.samples_by_node[&3].iter().map(|d| d[0]).sum::<f64>() / 6000.;
            assert!(
                (mean - 15. / 22.).abs() < 0.045,
                "coin posterior mean {mean}"
            );
        } else {
            assert_eq!(result.node_extents[&13], vec![12]);
            let mean = result.samples_by_node[&3].iter().map(|d| d[0]).sum::<f64>() / 6000.;
            assert!((0.1..0.65).contains(&mean), "fault rate {mean}");
            let last = &result.samples_by_node[&13];
            let hot = last.iter().map(|d| d[4] + d[6] + d[8] + d[11]).sum::<f64>() / (6000. * 4.);
            let cool = last
                .iter()
                .map(|d| d[0] + d[1] + d[2] + d[3] + d[5] + d[7] + d[9] + d[10])
                .sum::<f64>()
                / (6000. * 8.);
            assert!(hot > 0.9 && cool < 0.15, "hot {hot}, cool {cool}");
        }
    }
}
