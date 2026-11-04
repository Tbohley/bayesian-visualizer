//! Opening and closing posterior, observed-data, and plate-index panels.
use super::*;

pub fn open_histogram_panel(
    event: On<OpenHistogramPanel>,
    mut commands: Commands,
    inference_results: Option<Res<InferenceResultResource>>,
    graph: Option<Res<GraphIRResource>>,
    inference_status: Option<Res<InferenceStatusResource>>,
    selections: Option<Res<SampleSelections>>,
    plate_selections: Option<Res<PlateIndexSelections>>,
    bin_count_setting: Res<HistogramBinCount>,
    old_panels: Query<Entity, With<InferenceHistogramPanel>>,
    graph_nodes: Query<(
        Entity,
        &GraphNode,
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
    )>,
    node_labels: Query<(&ChildOf, &Text2d), With<NodeLabel>>,
    plates: Query<(&GraphNode, &crate::graph::Plate)>,
) {
    let requested_bin_count = bin_count_setting.0.clamp(1, MAX_HISTOGRAM_BINS);
    if event.clear_toasts {
        commands.trigger(ClearToasts);
    }
    despawn_histogram_panels(&mut commands, &old_panels);

    if let HistogramSubject::PlateIndex(plate_id) = event.subject {
        open_plate_index_panel(
            &mut commands,
            graph.as_deref(),
            plate_selections.as_deref(),
            plate_id,
            requested_bin_count,
        );
        return;
    }
    if let HistogramSubject::ObservedData(node_id) = event.subject {
        let label = graph_node_label(node_id, &graph_nodes, &node_labels);
        let node_entity = graph_nodes
            .iter()
            .find_map(|(entity, id, ..)| (id.0 == node_id).then_some(entity));
        let live_source = node_entity.and_then(|entity| {
            plates
                .iter()
                .filter_map(|(id, plate)| {
                    let column = plate.mapping.get(&entity)?;
                    if column == "unobserved" {
                        return None;
                    }
                    Some((
                        id.0,
                        column.as_str(),
                        plate.data.data.get(column)?.as_slice(),
                    ))
                })
                .min_by_key(|(id, _, _)| *id)
        });
        open_observed_data_panel(
            &mut commands,
            graph.as_deref(),
            live_source,
            plate_selections.as_deref(),
            node_id,
            &label,
            requested_bin_count,
        );
        return;
    }
    let HistogramSubject::Node(node_id) = event.subject else {
        unreachable!()
    };

    let Some(results) = inference_results else {
        return;
    };
    let Some(plate_ids) = graph
        .as_ref()
        .and_then(|graph| graph.0.node_plate_path(node_id))
        .map(<[u32]>::to_vec)
    else {
        commands.trigger(ErrorToast {
            text: format!("Could not find plate metadata for node {node_id}."),
            color: ERR_COLOR,
        });
        return;
    };
    if !results.0.node_extents.contains_key(&node_id) {
        return;
    }
    let instances = match results.0.samples_for_node(node_id) {
        Ok(instances) if !instances.is_empty() => instances,
        Ok(_) => {
            commands.trigger(ErrorToast {
                text: format!("Node {node_id} has no posterior values."),
                color: ERR_COLOR,
            });
            return;
        }
        Err(error) => {
            commands.trigger(ErrorToast {
                text: format!("Could not display node {node_id}: {error}"),
                color: ERR_COLOR,
            });
            return;
        }
    };
    let node_label = graph_node_label(node_id, &graph_nodes, &node_labels);

    let (full_samples, population) = displayed_instance_samples(&instances);
    let (plot_samples, instance_paths) = histogram_samples(&instances);
    let full_weighted_samples = unweighted_samples(&full_samples.samples);
    let highlighted_samples = linked_samples_for_selections(
        &instances,
        selections.as_deref(),
        plate_selections.as_deref(),
        &plate_ids,
    );
    let highlighted_samples = (!highlighted_samples.is_empty()).then_some(highlighted_samples);
    let displayed_sample_count = highlighted_samples
        .as_deref()
        .map_or(full_samples.samples.len() as f64, effective_count);
    let bin_count = requested_bin_count;
    let histogram =
        match node_histogram(graph.as_deref(), node_id, &full_samples.samples, bin_count) {
            Ok(histogram) => histogram,
            Err(error) => {
                commands.trigger(ErrorToast {
                    text: format!("Could not build histogram: {error}"),
                    color: ERR_COLOR,
                });
                return;
            }
        };
    let bin_count = histogram.bins.len();
    let highlighted_histogram = match highlighted_samples.as_deref() {
        Some(samples) => match build_weighted_histogram(samples, bin_count, histogram.domain) {
            Ok(histogram) => Some(histogram),
            Err(error) => {
                commands.trigger(ErrorToast {
                    text: format!("Could not build selected histogram layer: {error}"),
                    color: ERR_COLOR,
                });
                return;
            }
        },
        None => None,
    };
    let heading = match inference_status.as_deref() {
        Some(status) if status.state == InferenceResultState::Running => format!(
            "Live posterior samples - {node_label} - {} / {} draws",
            results.0.n_samples, status.requested_samples,
        ),
        Some(status) if status.state == InferenceResultState::Cancelled => format!(
            "Partial posterior - {node_label} - cancelled at {} / {} draws",
            results.0.n_samples, status.requested_samples,
        ),
        Some(status) if status.state == InferenceResultState::Failed => format!(
            "Partial posterior - {node_label} - failed at {} / {} draws",
            results.0.n_samples, status.requested_samples,
        ),
        _ => format!("Posterior samples - {node_label}"),
    };
    let heading = if plate_ids.is_empty() {
        heading
    } else {
        format!(
            "{heading}\n{} instances x {} draws",
            instances.len(),
            results.0.n_samples
        )
    };

    let panel = spawn_histogram_panel(
        &mut commands,
        event.subject,
        bin_count,
        displayed_sample_count,
    );

    let probability_line = histogram_selection_probability_line(
        &node_label,
        node_id,
        &full_samples.samples,
        highlighted_samples.as_deref(),
        selections.as_deref(),
        plate_selections.as_deref(),
        &plate_ids,
        graph.as_deref(),
    );
    let stats = if histogram.kind != HistogramKind::Continuous {
        discrete_stats(
            &mut commands,
            &node_label,
            &histogram,
            &full_weighted_samples,
        )
    } else {
        spawn_stats(
            &mut commands,
            &node_label,
            &full_weighted_samples,
            &population,
            probability_line,
            bin_count,
        )
    };
    let chart = spawn_chart(
        &mut commands,
        &histogram,
        highlighted_histogram.as_ref(),
        &plot_samples,
        &instance_paths,
        &plate_ids,
        event.subject,
        selections.as_deref(),
        &heading,
    );
    commands.entity(panel).add_children(&[stats, chart]);
}

fn open_plate_index_panel(
    commands: &mut Commands,
    graph: Option<&GraphIRResource>,
    selections: Option<&PlateIndexSelections>,
    plate_id: u32,
    requested_bins: usize,
) {
    let Some(plate) = graph.and_then(|graph| graph.0.graph().plates.get(&plate_id)) else {
        commands.trigger(ErrorToast {
            text: format!("Compile the graph before opening plate {plate_id}'s index selection."),
            color: ERR_COLOR,
        });
        return;
    };
    let extent = plate.n;
    let bin_count = requested_bins.min(extent).max(1);
    let domain = HistogramDomain {
        min: -0.5,
        max: extent as f64 - 0.5,
    };
    let weighted = (0..extent)
        .map(|index| WeightedSample {
            value: index as f64,
            weight: 1.0,
        })
        .collect::<Vec<_>>();
    let histogram = build_weighted_histogram(&weighted, bin_count, domain)
        .expect("validated positive plate extent makes a valid index histogram");
    let highlighted_histogram = selections
        .and_then(|selections| {
            selections
                .ranges_by_plate
                .get(&plate_id)
                .map(|_| selections)
        })
        .map(|selections| {
            let selected = weighted
                .iter()
                .enumerate()
                .filter(|(index, _)| selections.contains(plate_id, *index))
                .map(|(_, sample)| sample.clone())
                .collect::<Vec<_>>();
            build_weighted_histogram(&selected, bin_count, domain)
                .expect("selected plate indices share the validated index domain")
        });
    let samples = (0..extent)
        .map(|index| HistogramSample {
            instance: index,
            draw_index: 0,
            value: index as f64,
        })
        .collect::<Vec<_>>();
    let paths = (0..extent).map(|index| vec![index]).collect::<Vec<_>>();
    let selected = selections.map_or(0, |selections| selections.selected_count(plate_id, extent));
    let ranges = selections
        .and_then(|selections| selections.ranges_by_plate.get(&plate_id))
        .map_or_else(
            || "none".to_string(),
            |ranges| format_selection_ranges(ranges),
        );
    let percent_selected = selected as f64 * 100.0 / extent as f64;

    let panel = spawn_histogram_panel(
        commands,
        HistogramSubject::PlateIndex(plate_id),
        bin_count,
        selected as f64,
    );
    let summary = commands
        .spawn((
            Pickable::IGNORE,
            Node {
                width: px(210.0),
                flex_direction: FlexDirection::Column,
                row_gap: px(8.0),
                ..default()
            },
            children![
                (Text::new("Linked index selection"), TextColor(Color::WHITE), text_font()),
                (Text::new(format!("{selected} of {extent} indices selected ({percent_selected:.1}%)")), TextColor(Color::WHITE), text_font()),
                (Text::new(format!("Selected: {ranges}")), TextColor(Color::srgb(0.72, 0.74, 0.80)), text_font()),
                (Text::new("Drag to select whole indices and highlight matching instances across views."), TextColor(Color::srgb(0.72, 0.74, 0.80)), TextFont { font_size: FontSize::Px(10.0), ..text_font() }),
            ],
        ))
        .id();
    let chart = spawn_chart(
        commands,
        &histogram,
        highlighted_histogram.as_ref(),
        &samples,
        &paths,
        &[plate_id],
        HistogramSubject::PlateIndex(plate_id),
        None,
        &format!("Plate {plate_id} index (uniform discrete selector)"),
    );
    commands.entity(panel).add_children(&[summary, chart]);
}

fn open_observed_data_panel(
    commands: &mut Commands,
    graph: Option<&GraphIRResource>,
    live_source: Option<(u32, &str, &[f64])>,
    plate_selections: Option<&PlateIndexSelections>,
    node_id: u32,
    node_label: &str,
    requested_bins: usize,
) {
    // Read live plate data first, so viewing a column never requires compiling
    // the model or running inference (and reflects edits to its mapping).
    let source = live_source.or_else(|| {
        graph?
            .0
            .graph()
            .plates
            .iter()
            .find_map(|(&plate_id, plate)| {
                let column = plate.mapping.get(&node_id)?;
                Some((
                    plate_id,
                    column.as_str(),
                    plate.data.get(column)?.as_slice(),
                ))
            })
    });
    let Some((owner_id, column, values)) = source else {
        commands.trigger(ErrorToast {
            text: format!("Node {node_id} has no mapped observation column."),
            color: ERR_COLOR,
        });
        return;
    };
    let raw = observed_samples(values);
    let bin_count = requested_bins.clamp(1, MAX_HISTOGRAM_BINS);
    let histogram = match node_histogram(graph, node_id, &raw, bin_count) {
        Ok(value) => value,
        Err(error) => {
            commands.trigger(ErrorToast {
                text: format!("Could not build observed-data histogram: {error}"),
                color: ERR_COLOR,
            });
            return;
        }
    };
    let bin_count = histogram.bins.len();
    let weighted = unweighted_samples(&raw);
    let highlighted = linked_observed_samples(&raw, owner_id, plate_selections);
    let highlighted = (!highlighted.is_empty()).then_some(highlighted);
    let highlighted_histogram = highlighted.as_deref().map(|samples| {
        build_weighted_histogram(samples, bin_count, histogram.domain)
            .expect("selected observed rows share the full observed-data domain")
    });
    let samples = raw
        .iter()
        .enumerate()
        .map(|(instance, sample)| HistogramSample {
            instance,
            draw_index: sample.draw_index,
            value: sample.value,
        })
        .collect::<Vec<_>>();
    let paths = raw
        .iter()
        .map(|sample| vec![sample.draw_index])
        .collect::<Vec<_>>();
    let dimensions = graph
        .and_then(|graph| graph.0.node_plate_path(node_id))
        .unwrap_or_default();
    let broadcasts = dimensions
        .iter()
        .filter(|&&id| id != owner_id)
        .map(u32::to_string)
        .collect::<Vec<_>>();
    let broadcast_text = if broadcasts.is_empty() {
        String::new()
    } else {
        format!(
            "; broadcast across plate dimensions {} without duplicating rows",
            broadcasts.join(", ")
        )
    };
    let panel = spawn_histogram_panel(
        commands,
        HistogramSubject::ObservedData(node_id),
        bin_count,
        raw.len() as f64,
    );
    let stats = if histogram.kind != HistogramKind::Continuous {
        discrete_stats(commands, node_label, &histogram, &weighted)
    } else {
        spawn_stats(
            commands,
            node_label,
            &weighted,
            &HistogramPopulation::Pooled {
                instance_count: raw.len(),
            },
            Some(match highlighted.as_deref() {
                Some(selected) => format!(
                    "{} of {} source rows highlighted",
                    selected.len(),
                    raw.len()
                ),
                None => format!("{} source rows", raw.len()),
            }),
            bin_count,
        )
    };
    let chart = spawn_chart(
        commands,
        &histogram,
        highlighted_histogram.as_ref(),
        &samples,
        &paths,
        &[owner_id],
        HistogramSubject::ObservedData(node_id),
        None,
        &format!(
            "Column distribution - {node_label} | plate {owner_id} column {column}{broadcast_text}"
        ),
    );
    commands.entity(panel).add_children(&[stats, chart]);
}

pub fn close_histogram_panel(
    _event: On<CloseHistogramPanel>,
    mut commands: Commands,
    panels: Query<Entity, With<InferenceHistogramPanel>>,
    joint_views: Query<Entity, With<JointDistributionView>>,
    joint_indicators: Query<Entity, With<JointSelectedIndicator>>,
) {
    despawn_histogram_panels(&mut commands, &panels);
    for view in &joint_views {
        commands.entity(view).despawn();
    }
    for indicator in &joint_indicators {
        commands.entity(indicator).despawn();
    }
}

fn spawn_histogram_panel(
    commands: &mut Commands,
    subject: HistogramSubject,
    bin_count: usize,
    displayed_sample_count: f64,
) -> Entity {
    commands
        .spawn((
            InferenceHistogramPanel,
            HistogramView {
                subject,
                bin_count,
                displayed_sample_count,
            },
            Node {
                position_type: PositionType::Absolute,
                left: px(SIDEBAR_WIDTH),
                right: px(SIDEBAR_WIDTH),
                bottom: px(0.0),
                height: px(HISTOGRAM_PANEL_HEIGHT),
                padding: px(16.0).all(),
                column_gap: px(20.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            BackgroundColor(Color::srgb(0.10, 0.11, 0.14)),
            BorderColor::all(Color::srgb(0.28, 0.30, 0.36)),
            ZIndex(100),
        ))
        .observe(|mut event: On<Pointer<Press>>| event.propagate(false))
        .id()
}

pub(super) fn graph_node_label(
    node_id: u32,
    graph_nodes: &Query<(
        Entity,
        &GraphNode,
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
    )>,
    labels: &Query<(&ChildOf, &Text2d), With<NodeLabel>>,
) -> String {
    let Some(entity) = graph_nodes
        .iter()
        .find_map(|(entity, node, _, _, _)| (node.0 == node_id).then_some(entity))
    else {
        return format!("node#{node_id}");
    };
    labels
        .iter()
        .find_map(|(child_of, label)| (child_of.parent() == entity).then(|| label.0.clone()))
        .unwrap_or_else(|| format!("node#{node_id}"))
}

fn despawn_histogram_panels(
    commands: &mut Commands,
    panels: &Query<Entity, With<InferenceHistogramPanel>>,
) {
    for panel in panels {
        commands.entity(panel).despawn();
    }
}

fn spawn_stats(
    commands: &mut Commands,
    node_label: &str,
    samples: &[WeightedSample],
    population: &HistogramPopulation,
    probability_line: Option<String>,
    bin_count: usize,
) -> Entity {
    let stats = commands
        .spawn(Node {
            width: px(250.0),
            height: percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: px(6.0),
            ..default()
        })
        .id();

    let instance_note = match population {
        HistogramPopulation::Instance(indices) => format_instance_label(node_label, indices),
        HistogramPopulation::Pooled { instance_count } => {
            format!("{node_label} ({instance_count} pooled instances)")
        }
    };
    let mut lines = vec![(instance_note, 18.0)];
    if let Some(probability_line) = probability_line {
        lines.push((probability_line, 12.0));
    }
    let mut sorted_samples = samples.to_vec();
    sorted_samples.sort_by(|a, b| a.value.total_cmp(&b.value));
    lines.extend([
        (
            format!("Samples: {}", format_count(effective_count(samples))),
            14.0,
        ),
        (format!("Mean: {:.4}", weighted_mean(samples)), 14.0),
        (
            format!("SD: {:.4}", weighted_standard_deviation(samples)),
            14.0,
        ),
        (
            format!(
                "Median: {:.4}",
                weighted_quantile_sorted(&sorted_samples, 0.5)
            ),
            14.0,
        ),
        (
            format!(
                "95% CrI: [{:.4}, {:.4}]",
                weighted_quantile_sorted(&sorted_samples, 0.025),
                weighted_quantile_sorted(&sorted_samples, 0.975)
            ),
            14.0,
        ),
    ]);

    for (line, font_size) in lines {
        let text = commands
            .spawn((
                Pickable::IGNORE,
                Text::new(line),
                TextColor(Color::WHITE),
                TextFont {
                    font_size: FontSize::Px(font_size),
                    ..text_font()
                },
            ))
            .id();
        commands.entity(stats).add_child(text);
    }
    let stepper = spawn_bin_stepper(commands, bin_count);
    commands.entity(stats).add_child(stepper);
    stats
}
