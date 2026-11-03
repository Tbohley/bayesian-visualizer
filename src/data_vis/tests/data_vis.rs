use super::joint::{joint_selection_cells, selected_cell_boundary};
use super::*;

fn sample(draw_index: usize, value: f64) -> PosteriorSample {
    PosteriorSample { draw_index, value }
}

#[test]
fn bins_samples() {
    let samples = vec![
        sample(0, 0.0),
        sample(1, 0.25),
        sample(2, 0.75),
        sample(3, 1.0),
    ];

    let histogram = build_histogram(&samples, 2).unwrap();

    assert_eq!(histogram.domain, HistogramDomain { min: 0.0, max: 1.0 });
    assert_eq!(histogram.bins[0].count, 2.0);
    assert_eq!(histogram.bins[1].count, 2.0);
    assert_eq!(histogram.max_count, 2.0);
}

#[test]
fn constant_samples_get_a_renderable_domain() {
    let histogram = build_histogram(&[sample(0, 4.0), sample(1, 4.0)], 5).unwrap();

    assert!(histogram.domain.min < 4.0);
    assert!(histogram.domain.max > 4.0);
    assert_eq!(histogram.bins.iter().map(|bin| bin.count).sum::<f64>(), 2.0);
}

#[test]
fn pooling_preserves_every_plate_point_and_its_draw() {
    let instances = vec![
        NodeInstanceSamples {
            indices: vec![0],
            samples: vec![sample(0, 1.0), sample(1, 2.0)],
        },
        NodeInstanceSamples {
            indices: vec![1],
            samples: vec![sample(0, 10.0), sample(1, 20.0)],
        },
    ];

    assert_eq!(
        pool_instance_samples(&instances),
        vec![
            sample(0, 1.0),
            sample(1, 2.0),
            sample(0, 10.0),
            sample(1, 20.0),
        ]
    );
}

#[test]
fn rejects_invalid_inputs() {
    assert!(build_histogram(&[], 10).is_err());
    assert!(build_histogram(&[sample(0, 1.0)], 0).is_err());
    assert!(build_histogram(&[sample(0, f64::NAN)], 10).is_err());
}

#[test]
fn plot_coordinates_map_back_to_the_domain() {
    let domain = HistogramDomain {
        min: -2.0,
        max: 2.0,
    };
    assert_eq!(domain.value_at_plot_x(50.0, 100.0), 0.0);
    assert_eq!(domain.value_at_plot_x(-10.0, 100.0), -2.0);
    assert_eq!(domain.value_at_plot_x(200.0, 100.0), 2.0);
}

#[test]
fn brushing_retains_the_plate_instance_for_each_draw() {
    let samples = vec![
        HistogramSample {
            instance: 0,
            draw_index: 0,
            value: 0.25,
        },
        HistogramSample {
            instance: 1,
            draw_index: 0,
            value: 1.0,
        },
        HistogramSample {
            instance: 1,
            draw_index: 1,
            value: 0.5,
        },
    ];

    assert_eq!(
        selected_instances(&samples, &[vec![0], vec![1]], 0.0, 0.5),
        HashMap::from([(vec![0], HashSet::from([0])), (vec![1], HashSet::from([1]))])
    );
}

#[test]
fn linked_plate_histograms_match_rows_before_draws() {
    let instances = vec![
        NodeInstanceSamples {
            indices: vec![0],
            samples: vec![sample(0, 10.0), sample(1, 11.0)],
        },
        NodeInstanceSamples {
            indices: vec![1],
            samples: vec![sample(0, 20.0), sample(1, 21.0)],
        },
    ];
    let selection = SampleSelection {
        source: SelectionSource::Histogram {
            node_id: 1,
            lower: 0.0,
            upper: 1.0,
        },
        context_plate_ids: vec![10],
        context_instance_paths: vec![vec![0], vec![1]],
        draws_by_instance: HashMap::from([(vec![0], HashSet::from([0, 1]))]),
    };

    let linked = linked_samples(&instances, &selection, &[10]);

    assert_eq!(linked, unweighted_samples(&instances[0].samples));

    let root = linked_samples(
        &[NodeInstanceSamples {
            indices: Vec::new(),
            samples: vec![sample(0, 30.0), sample(1, 31.0)],
        }],
        &selection,
        &[],
    );
    assert_eq!(effective_count(&root), 1.0);
    assert!(root.iter().all(|sample| sample.weight == 0.5));
}

#[test]
fn linked_histograms_keep_the_original_domain_for_the_highlight_layer() {
    let instances = vec![NodeInstanceSamples {
        indices: Vec::new(),
        samples: vec![sample(0, 100.0), sample(1, 4.0), sample(2, 7.0)],
    }];
    let full_histogram = build_histogram(&instances[0].samples, 2).unwrap();
    let selection = SampleSelection {
        source: SelectionSource::Histogram {
            node_id: 1,
            lower: 4.0,
            upper: 7.0,
        },
        context_plate_ids: Vec::new(),
        context_instance_paths: vec![Vec::new()],
        draws_by_instance: HashMap::from([(Vec::new(), HashSet::from([1, 2]))]),
    };
    let filtered = linked_samples(&instances, &selection, &[]);
    let highlighted_histogram =
        build_weighted_histogram(&filtered, 2, full_histogram.domain).unwrap();

    assert_eq!(
        filtered,
        vec![
            WeightedSample {
                value: 4.0,
                weight: 1.0
            },
            WeightedSample {
                value: 7.0,
                weight: 1.0
            }
        ]
    );
    assert_eq!(
        highlighted_histogram.domain,
        HistogramDomain {
            min: 4.0,
            max: 100.0
        }
    );
    assert_eq!(highlighted_histogram.bins[0].count, 2.0);
    assert_eq!(highlighted_histogram.bins[1].count, 0.0);
}

#[test]
fn additive_selections_union_overlapping_draws() {
    let instances = vec![NodeInstanceSamples {
        indices: Vec::new(),
        samples: vec![sample(0, 1.0), sample(1, 2.0), sample(2, 3.0)],
    }];
    let make_selection = |draws: HashSet<usize>| SampleSelection {
        source: SelectionSource::Histogram {
            node_id: 1,
            lower: 0.0,
            upper: 1.0,
        },
        context_plate_ids: Vec::new(),
        context_instance_paths: vec![Vec::new()],
        draws_by_instance: HashMap::from([(Vec::new(), draws)]),
    };
    let selections = SampleSelections {
        entries: vec![
            make_selection(HashSet::from([0, 1])),
            make_selection(HashSet::from([1, 2])),
        ],
    };

    let linked = linked_samples_for_selections(&instances, Some(&selections), None, &[]);

    assert_eq!(effective_count(&linked), 3.0);
    assert_eq!(
        linked.iter().map(|sample| sample.value).collect::<Vec<_>>(),
        vec![1.0, 2.0, 3.0]
    );
}

#[test]
fn additive_plate_selections_combine_distinct_instance_weight() {
    let target = vec![NodeInstanceSamples {
        indices: Vec::new(),
        samples: vec![sample(0, 9.0)],
    }];
    let make_selection = |path: Vec<usize>| SampleSelection {
        source: SelectionSource::Histogram {
            node_id: 1,
            lower: 0.0,
            upper: 1.0,
        },
        context_plate_ids: vec![10],
        context_instance_paths: vec![vec![0], vec![1]],
        draws_by_instance: HashMap::from([(path, HashSet::from([0]))]),
    };
    let selections = SampleSelections {
        entries: vec![make_selection(vec![0]), make_selection(vec![1])],
    };

    let linked = linked_samples_for_selections(&target, Some(&selections), None, &[]);

    assert_eq!(effective_count(&linked), 1.0);
}

#[test]
fn joint_samples_pair_matching_draws_and_plate_rows() {
    let x = vec![
        NodeInstanceSamples {
            indices: vec![0],
            samples: vec![sample(0, 1.0), sample(1, 2.0)],
        },
        NodeInstanceSamples {
            indices: vec![1],
            samples: vec![sample(0, 10.0)],
        },
    ];
    let y = vec![
        NodeInstanceSamples {
            indices: vec![0],
            samples: vec![sample(0, 3.0), sample(1, 4.0)],
        },
        NodeInstanceSamples {
            indices: vec![1],
            samples: vec![sample(0, 30.0)],
        },
    ];

    let (points, plate_ids, paths) = build_joint_samples(&x, &[7], &y, &[7]);

    assert_eq!(plate_ids, vec![7]);
    assert_eq!(paths, vec![vec![0], vec![1]]);
    assert_eq!(points.len(), 3);
    assert!(
        points
            .iter()
            .any(|point| point.x == 10.0 && point.y == 30.0)
    );
}

#[test]
fn selected_cell_boundaries_merge_adjacent_cells() {
    let mut selected = vec![false; 9];
    selected[4] = true;
    selected[5] = true;

    assert_eq!(
        selected_cell_boundary(&selected, 3, 1, 1),
        SelectedCellBoundary {
            left: true,
            right: false,
            top: true,
            bottom: true,
        }
    );
    assert_eq!(
        selected_cell_boundary(&selected, 3, 2, 1),
        SelectedCellBoundary {
            left: false,
            right: true,
            top: true,
            bottom: true,
        }
    );
}

#[test]
fn joint_polygon_fills_empty_cells_inside_selection_boundary() {
    let selection = SampleSelection {
        source: SelectionSource::Joint {
            x_node_id: 1,
            y_node_id: 2,
            polygon: vec![
                Vec2::new(0.2, 0.2),
                Vec2::new(0.8, 0.2),
                Vec2::new(0.8, 0.8),
                Vec2::new(0.2, 0.8),
            ],
        },
        context_plate_ids: Vec::new(),
        context_instance_paths: vec![Vec::new()],
        draws_by_instance: HashMap::new(),
    };
    let cells = joint_selection_cells(
        &[0; 25],
        5,
        1,
        2,
        Some(&SampleSelections {
            entries: vec![selection],
        }),
    );

    assert!(cells[2 * 5 + 2]);
    assert!(!cells[0]);
    assert!(!cells[4 * 5 + 4]);
}

#[test]
fn histogram_selection_uses_exact_partial_bin_bounds() {
    let samples = vec![
        HistogramSample {
            instance: 0,
            draw_index: 0,
            value: 0.1,
        },
        HistogramSample {
            instance: 0,
            draw_index: 1,
            value: 0.25,
        },
        HistogramSample {
            instance: 0,
            draw_index: 2,
            value: 0.75,
        },
        HistogramSample {
            instance: 0,
            draw_index: 3,
            value: 0.9,
        },
    ];
    let selected = selected_instances(&samples, &[Vec::new()], 0.24, 0.76);

    assert_eq!(selected[&Vec::new()], HashSet::from([1, 2]));
}

#[test]
fn probability_text_uses_two_sided_and_one_sided_bounds() {
    assert_eq!(
        format_interval_probability("beta", -0.25, 0.75, -1.0, 1.0, 0.625),
        "P(beta > -0.2500 & beta < 0.7500) = 0.6250"
    );
    assert_eq!(
        format_interval_probability("beta", -1.0, 0.75, -1.0, 1.0, 0.875),
        "P(beta < 0.7500) = 0.8750"
    );
    assert_eq!(
        format_interval_probability("beta", -0.25, 1.0, -1.0, 1.0, 0.75),
        "P(beta > -0.2500) = 0.7500"
    );
}

#[test]
fn polygon_selection_handles_arbitrary_non_rectangular_shapes() {
    let triangle = [
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, 1.0),
    ];
    assert!(point_in_polygon(Vec2::new(0.2, 0.2), &triangle));
    assert!(!point_in_polygon(Vec2::new(0.8, 0.8), &triangle));
}

#[test]
fn plate_index_brush_snaps_to_complete_indices() {
    assert_eq!(snapped_plate_index_range(-0.49, 2.49, 5), Some((0, 2)));
    assert_eq!(snapped_plate_index_range(3.51, 1.51, 5), Some((2, 4)));
    assert_eq!(snapped_plate_index_range(0.0, 1.0, 0), None);
}

#[test]
fn plate_index_brushes_union_and_normalize_ranges() {
    let mut selections = PlateIndexSelections::default();
    selections.union(7, 3, 5);
    selections.union(7, 8, 8);
    selections.union(7, 6, 7);
    assert_eq!(selections.ranges_by_plate[&7], vec![(3, 8)]);
    assert!(selections.contains(7, 4));
    assert!(!selections.contains(7, 2));
    assert_eq!(selections.selected_count(7, 20), 6);
}

#[test]
fn plate_index_clicks_toggle_individual_indices() {
    let mut selections = PlateIndexSelections::default();
    selections.toggle(7, 3);
    selections.toggle(7, 5);
    selections.toggle(7, 4);
    assert_eq!(selections.ranges_by_plate[&7], vec![(3, 5)]);

    selections.toggle(7, 4);
    assert_eq!(selections.ranges_by_plate[&7], vec![(3, 3), (5, 5)]);
    selections.toggle(7, 3);
    selections.toggle(7, 5);
    assert!(!selections.ranges_by_plate.contains_key(&7));
    assert!(!selections.contains(7, 3));
}

#[test]
fn long_selection_labels_truncate_without_losing_ranges() {
    let ranges = (0..8)
        .map(|index| (index * 2, index * 2))
        .collect::<Vec<_>>();
    let label = format_selection_ranges(&ranges);
    assert_eq!(label, "0, 2, 4, 6, 8, 10, ... (+2 ranges)");
    assert_eq!(ranges.len(), 8);
}

#[test]
fn plate_selection_keeps_the_full_base_and_highlights_every_matching_draw() {
    let instances = (0..2)
        .flat_map(|i| {
            (0..3).map(move |j| NodeInstanceSamples {
                indices: vec![i, j],
                samples: vec![
                    sample(0, (i * 10 + j) as f64),
                    sample(1, (i * 10 + j) as f64 + 0.5),
                ],
            })
        })
        .collect::<Vec<_>>();
    let mut selections = PlateIndexSelections::default();
    selections.union(10, 1, 1);

    let base = pool_instance_samples(&instances);
    let linked = linked_samples_for_selections(&instances, None, Some(&selections), &[10, 20]);

    assert_eq!(base.len(), 12);
    assert_eq!(linked.len(), 6);
    assert!(linked.iter().all(|sample| sample.value >= 10.0));
}

#[test]
fn plate_selection_is_a_no_op_for_root_and_unrelated_nodes() {
    let mut selections = PlateIndexSelections::default();
    selections.union(10, 1, 1);
    let root = vec![NodeInstanceSamples {
        indices: Vec::new(),
        samples: vec![sample(0, 1.0)],
    }];
    let sibling = vec![NodeInstanceSamples {
        indices: vec![1],
        samples: vec![sample(0, 2.0)],
    }];

    assert!(linked_samples_for_selections(&root, None, Some(&selections), &[]).is_empty());
    assert!(linked_samples_for_selections(&sibling, None, Some(&selections), &[20]).is_empty());
}

#[test]
fn selections_on_different_plates_use_or_semantics() {
    let instances = (0..2)
        .flat_map(|p| {
            (0..2).map(move |q| NodeInstanceSamples {
                indices: vec![p, q],
                samples: vec![sample(0, (p * 10 + q) as f64)],
            })
        })
        .collect::<Vec<_>>();
    let mut selections = PlateIndexSelections::default();
    selections.union(10, 1, 1);
    selections.union(20, 1, 1);

    let linked = linked_samples_for_selections(&instances, None, Some(&selections), &[10, 20]);
    assert_eq!(
        linked.iter().map(|sample| sample.value).collect::<Vec<_>>(),
        vec![1.0, 10.0, 11.0]
    );
}

#[test]
fn plate_value_and_joint_selections_share_union_semantics() {
    let instances = (0..2)
        .flat_map(|p| {
            (0..2).map(move |q| NodeInstanceSamples {
                indices: vec![p, q],
                samples: vec![
                    sample(0, (p * 10 + q) as f64),
                    sample(1, 100.0 + (p * 10 + q) as f64),
                ],
            })
        })
        .collect::<Vec<_>>();
    let make_selection = |source, path, draw| SampleSelection {
        source,
        context_plate_ids: vec![10, 20],
        context_instance_paths: instances
            .iter()
            .map(|instance| instance.indices.clone())
            .collect(),
        draws_by_instance: HashMap::from([(path, HashSet::from([draw]))]),
    };
    let sample_selections = SampleSelections {
        entries: vec![
            make_selection(
                SelectionSource::Histogram {
                    node_id: 1,
                    lower: 0.0,
                    upper: 1.0,
                },
                vec![0, 0],
                0,
            ),
            make_selection(
                SelectionSource::Joint {
                    x_node_id: 1,
                    y_node_id: 2,
                    polygon: Vec::new(),
                },
                vec![0, 1],
                1,
            ),
        ],
    };
    let mut plate_selections = PlateIndexSelections::default();
    plate_selections.union(10, 1, 1);

    let linked = linked_samples_for_selections(
        &instances,
        Some(&sample_selections),
        Some(&plate_selections),
        &[10, 20],
    );
    assert_eq!(linked.len(), 6);
    assert!(linked.iter().any(|sample| sample.value == 0.0));
    assert!(linked.iter().any(|sample| sample.value == 101.0));
    assert!(linked.iter().any(|sample| sample.value == 110.0));
    assert!(linked.iter().any(|sample| sample.value == 111.0));
}

#[test]
fn new_live_draws_for_selected_instances_are_linked_without_draw_storage() {
    let mut instances = vec![NodeInstanceSamples {
        indices: vec![1],
        samples: vec![sample(0, 1.0)],
    }];
    let mut selections = PlateIndexSelections::default();
    selections.union(10, 1, 1);
    assert_eq!(
        linked_samples_for_selections(&instances, None, Some(&selections), &[10]).len(),
        1
    );

    instances[0].samples.push(sample(1, 2.0));
    assert_eq!(
        linked_samples_for_selections(&instances, None, Some(&selections), &[10]).len(),
        2
    );
    assert_eq!(selections.ranges_by_plate[&10], vec![(1, 1)]);
}

#[test]
fn joint_base_is_complete_and_plate_highlights_use_union_context() {
    let x = vec![
        NodeInstanceSamples {
            indices: vec![0, 0],
            samples: vec![sample(0, 1.0)],
        },
        NodeInstanceSamples {
            indices: vec![1, 0],
            samples: vec![sample(0, 2.0)],
        },
    ];
    let y = vec![
        NodeInstanceSamples {
            indices: vec![0, 0],
            samples: vec![sample(0, 10.0)],
        },
        NodeInstanceSamples {
            indices: vec![0, 1],
            samples: vec![sample(0, 20.0)],
        },
    ];
    let (points, plate_ids, paths) = build_joint_samples(&x, &[10, 20], &y, &[20, 30]);
    let mut selections = PlateIndexSelections::default();
    selections.union(30, 1, 1);

    assert_eq!(points.len(), 4);
    assert_eq!(plate_ids, vec![10, 20, 30]);
    assert_eq!(
        points
            .iter()
            .filter(|point| sample_is_selected(
                &plate_ids,
                &paths[point.context_instance],
                point.draw_index,
                None,
                Some(&selections),
            ))
            .count(),
        2
    );
}

#[test]
fn raw_observations_stay_one_per_owner_row_and_ignore_broadcast_only_selections() {
    let raw = observed_samples(&[1.0, 2.0, 3.0]);
    let mut selections = PlateIndexSelections::default();
    selections.union(20, 0, 2);
    assert_eq!(raw.len(), 3);
    assert!(linked_observed_samples(&raw, 10, Some(&selections)).is_empty());

    selections.union(10, 1, 1);
    let linked = linked_observed_samples(&raw, 10, Some(&selections));
    assert_eq!(
        linked,
        vec![WeightedSample {
            value: 2.0,
            weight: 1.0
        }]
    );
    assert_eq!(raw.len(), 3);
}

#[test]
fn clearing_linked_selections_removes_plate_and_posterior_resources() {
    use bevy::ecs::world::CommandQueue;

    let mut world = World::new();
    world.insert_resource(SampleSelections {
        entries: vec![SampleSelection {
            source: SelectionSource::Histogram {
                node_id: 1,
                lower: 0.0,
                upper: 1.0,
            },
            context_plate_ids: Vec::new(),
            context_instance_paths: vec![Vec::new()],
            draws_by_instance: HashMap::from([(Vec::new(), HashSet::from([0]))]),
        }],
    });
    let mut plate_selections = PlateIndexSelections::default();
    plate_selections.union(10, 0, 1);
    world.insert_resource(plate_selections);
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, &world);
    clear_linked_selection_resources(&mut commands);
    queue.apply(&mut world);

    assert!(!world.contains_resource::<SampleSelections>());
    assert!(!world.contains_resource::<PlateIndexSelections>());
}

#[test]
fn selecting_all_indices_builds_a_full_linked_layer() {
    let instances = (0..3)
        .map(|index| NodeInstanceSamples {
            indices: vec![index],
            samples: vec![sample(0, index as f64), sample(1, index as f64 + 0.5)],
        })
        .collect::<Vec<_>>();
    let mut selections = PlateIndexSelections::default();
    selections.union(10, 0, 2);
    let linked = linked_samples_for_selections(&instances, None, Some(&selections), &[10]);

    assert_eq!(effective_count(&linked), 6.0);
    assert_eq!(linked.len(), pool_instance_samples(&instances).len());
    assert_eq!(selections.selected_count(10, 3), 3);
}
