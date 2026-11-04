use super::*;
use crate::bayesian_core::{GraphIR, NodeIR, ParamIR, Value};
fn graph(name: &str) -> (GraphIRResource, u32) {
    let mut g = GraphIR::new();
    let params = if name == "Categorical" {
        g.nodes.insert(
            1,
            NodeIR::Constant {
                value: Value::RealVector(vec![0.25, 0., 0.75].into()),
            },
        );
        vec![ParamIR { from_node: 1 }]
    } else {
        g.nodes.insert(
            1,
            NodeIR::Constant {
                value: Value::Integer(20),
            },
        );
        g.nodes.insert(2, NodeIR::Scalar { value: 0.6 });
        vec![ParamIR { from_node: 1 }, ParamIR { from_node: 2 }]
    };
    g.nodes.insert(
        3,
        NodeIR::Random {
            dist_type: name.into(),
            params,
        },
    );
    (GraphIRResource(g.compile().unwrap()), 3)
}
#[test]
fn categorical_bins_include_unseen_categories_and_center_integer_labels() {
    let (graph, id) = graph("Categorical");
    let samples = vec![
        PosteriorSample {
            draw_index: 0,
            value: 2.,
        },
        PosteriorSample {
            draw_index: 1,
            value: 2.,
        },
    ];
    let histogram = node_histogram(Some(&graph), id, &samples, 20).unwrap();
    assert_eq!(histogram.kind, HistogramKind::Categorical);
    assert_eq!(
        histogram.domain,
        HistogramDomain {
            min: -0.5,
            max: 2.5
        }
    );
    assert_eq!(
        histogram.bins.iter().map(|b| b.count).collect::<Vec<_>>(),
        vec![0., 0., 2.]
    );
    assert_eq!(
        histogram.bins.iter().map(bin_label).collect::<Vec<_>>(),
        vec!["0", "1", "2"]
    );
    let layer = build_weighted_histogram(
        &[WeightedSample {
            value: 2.,
            weight: 0.5,
        }],
        histogram.bins.len(),
        histogram.domain,
    )
    .unwrap();
    assert_eq!(layer.bins[2].count, 0.5);
}
#[test]
fn inference_results_render_discrete_bins_and_semantic_stats() {
    for name in ["Binomial", "Categorical"] {
        let (graph, id) = graph(name);
        let result = graph.0.run_inference(42, 300, 50).unwrap();
        let mut app = App::new();
        app.init_resource::<HistogramBinCount>()
            .add_observer(open_histogram_panel);
        app.world_mut().insert_resource(graph);
        app.world_mut()
            .insert_resource(InferenceResultResource(result));
        app.world_mut().trigger(OpenHistogramPanel {
            subject: HistogramSubject::Node(id),
            clear_toasts: false,
        });
        app.world_mut().flush();
        let mut plots = app.world_mut().query::<&HistogramPlot>();
        let plot = plots.single(app.world()).unwrap();
        assert_eq!(plot.bins.len(), if name == "Categorical" { 3 } else { 21 });
        let mut labels = app.world_mut().query::<&Text>();
        let labels = labels
            .iter(app.world())
            .map(|t| t.0.as_str())
            .collect::<Vec<_>>();
        assert!(labels.iter().any(|t| t.starts_with("Mode:")));
        if name == "Categorical" {
            assert!(
                !labels
                    .iter()
                    .any(|t| t.starts_with("Mean:") || t.starts_with("SD:"))
            );
            assert!(labels.iter().any(|t| t.contains("zero-based")));
        }
    }
}

fn categorical_click(app: &mut App, index: usize) {
    use bevy::{
        camera::NormalizedRenderTarget,
        picking::{
            backend::HitData,
            pointer::{Location, PointerButton, PointerId},
        },
        window::WindowRef,
    };
    let entity = app
        .world_mut()
        .query_filtered::<Entity, With<HistogramPlot>>()
        .single(app.world())
        .unwrap();
    app.world_mut().entity_mut(entity).insert((
        ComputedNode {
            size: Vec2::new(300., 100.),
            inverse_scale_factor: 1.,
            ..default()
        },
        UiGlobalTransform::from_xy(150., 50.),
        ComputedUiRenderTargetInfo::default(),
    ));
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
            position: Vec2::new(index as f32 * 100. + 50., 50.),
        },
        bevy::picking::events::Click {
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0., None, None),
            duration: std::time::Duration::from_millis(50),
            count: 1,
        },
        entity,
    ));
    app.world_mut().flush();
}

#[test]
fn categorical_bin_clicks_toggle_multiple_categories_including_empty_bins() {
    let (graph, id) = graph("Categorical");
    let result = graph.0.run_inference(42, 500, 0).unwrap();
    let mut app = App::new();
    app.init_resource::<HistogramBinCount>()
        .init_resource::<UiScale>()
        .add_observer(open_histogram_panel);
    app.world_mut().spawn(Window::default());
    app.world_mut().insert_resource(graph);
    app.world_mut()
        .insert_resource(InferenceResultResource(result));
    app.world_mut().trigger(OpenHistogramPanel {
        subject: HistogramSubject::Node(id),
        clear_toasts: false,
    });
    app.world_mut().flush();
    categorical_click(&mut app, 0);
    assert_eq!(app.world().resource::<SampleSelections>().entries.len(), 1);
    categorical_click(&mut app, 2);
    assert_eq!(app.world().resource::<SampleSelections>().entries.len(), 2);
    let plot = app
        .world_mut()
        .query::<&HistogramPlot>()
        .single(app.world())
        .unwrap();
    assert_eq!(
        plot.highlighted_bins
            .as_ref()
            .unwrap()
            .iter()
            .map(|b| b.count)
            .sum::<f64>(),
        500.
    );
    categorical_click(&mut app, 1);
    assert_eq!(app.world().resource::<SampleSelections>().entries.len(), 3);
    assert_eq!(
        app.world().resource::<SampleSelections>().entries[2].point_count(),
        0
    );
    categorical_click(&mut app, 0);
    let plot = app
        .world_mut()
        .query::<&HistogramPlot>()
        .single(app.world())
        .unwrap();
    assert_eq!(plot.highlighted_bins.as_ref().unwrap()[0].count, 0.);
    assert_eq!(
        plot.highlighted_bins.as_ref().unwrap()[2].count,
        plot.bins[2].count
    );
    assert_eq!(plot.samples.len(), 500);
    categorical_click(&mut app, 2);
    categorical_click(&mut app, 1);
    assert!(
        app.world()
            .resource::<SampleSelections>()
            .entries
            .is_empty()
    );
    let plot = app
        .world_mut()
        .query::<&HistogramPlot>()
        .single(app.world())
        .unwrap();
    assert!(plot.highlighted_bins.is_none());
}

#[test]
fn categorical_toggling_preserves_other_nodes_and_plate_instance_draws() {
    let mut plot = HistogramPlot {
        kind: HistogramKind::Categorical,
        domain: HistogramDomain {
            min: -0.5,
            max: 1.5,
        },
        bins: vec![
            HistogramBin {
                lower: -0.5,
                upper: 0.5,
                count: 2.,
            },
            HistogramBin {
                lower: 0.5,
                upper: 1.5,
                count: 1.,
            },
        ],
        highlighted_bins: None,
        samples: vec![
            HistogramSample {
                instance: 0,
                draw_index: 0,
                value: 0.,
            },
            HistogramSample {
                instance: 1,
                draw_index: 0,
                value: 0.,
            },
            HistogramSample {
                instance: 1,
                draw_index: 1,
                value: 1.,
            },
        ],
        instance_paths: vec![vec![0], vec![1]],
        plate_ids: vec![10],
        subject: HistogramSubject::Node(3),
    };
    let mut selections = SampleSelections::default();
    plot.subject = HistogramSubject::Node(4);
    toggle_category_bin(&plot, 1, &mut selections);
    plot.subject = HistogramSubject::Node(3);
    toggle_category_bin(&plot, 0, &mut selections);
    assert_eq!(selections.entries[1].context_plate_ids, vec![10]);
    assert_eq!(
        selections.entries[1].draws_by_instance[&vec![0]],
        HashSet::from([0])
    );
    assert_eq!(
        selections.entries[1].draws_by_instance[&vec![1]],
        HashSet::from([0])
    );
    toggle_category_bin(&plot, 0, &mut selections);
    assert_eq!(selections.entries.len(), 1);
    assert_eq!(
        selections.entries[0].source.histogram_range_for(4),
        Some((0.5, 1.5))
    );
}
