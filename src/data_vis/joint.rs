//! Joint posterior plots, heatmaps, and lasso selection.
use super::*;

pub fn open_joint_distribution_view(
    event: On<OpenJointDistributionView>,
    mut commands: Commands,
    results: Option<Res<InferenceResultResource>>,
    graph: Option<Res<GraphIRResource>>,
    selections: Option<Res<SampleSelections>>,
    plate_selections: Option<Res<PlateIndexSelections>>,
    bin_count_setting: Res<HistogramBinCount>,
    old_sidebars: Query<Entity, With<LocalSidebar>>,
    old_joint_indicators: Query<Entity, With<JointSelectedIndicator>>,
    graph_nodes: Query<(
        Entity,
        &GraphNode,
        Option<&RandomNode>,
        Option<&ScalarNode>,
        Option<&ComputeNode>,
    )>,
    node_labels: Query<(&ChildOf, &Text2d), With<NodeLabel>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let (Some(results), Some(graph)) = (results, graph) else {
        return;
    };
    let Some(x_plate_ids) = graph
        .0
        .node_plate_path(event.x_node_id)
        .map(<[u32]>::to_vec)
    else {
        return;
    };
    let Some(y_plate_ids) = graph
        .0
        .node_plate_path(event.y_node_id)
        .map(<[u32]>::to_vec)
    else {
        return;
    };
    let Ok(x_instances) = results.0.samples_for_node(event.x_node_id) else {
        return;
    };
    let Ok(y_instances) = results.0.samples_for_node(event.y_node_id) else {
        return;
    };
    let (points, context_plate_ids, context_instance_paths) =
        build_joint_samples(&x_instances, &x_plate_ids, &y_instances, &y_plate_ids);
    if points.is_empty() {
        commands.trigger(ErrorToast {
            text: "These variables have no posterior draws in common.".to_string(),
            color: ERR_COLOR,
        });
        return;
    }
    let x_values = points
        .iter()
        .enumerate()
        .map(|(draw_index, point)| PosteriorSample {
            draw_index,
            value: point.x,
        })
        .collect::<Vec<_>>();
    let y_values = points
        .iter()
        .enumerate()
        .map(|(draw_index, point)| PosteriorSample {
            draw_index,
            value: point.y,
        })
        .collect::<Vec<_>>();
    let bins = bin_count_setting.0.clamp(1, MAX_HISTOGRAM_BINS);
    let x_histogram = match node_histogram(Some(&graph), event.x_node_id, &x_values, bins) {
        Ok(histogram) => histogram,
        Err(_) => return,
    };
    let y_histogram = match node_histogram(Some(&graph), event.y_node_id, &y_values, bins) {
        Ok(histogram) => histogram,
        Err(_) => return,
    };
    let x_label_text = graph_node_label(event.x_node_id, &graph_nodes, &node_labels);
    let y_label_text = graph_node_label(event.y_node_id, &graph_nodes, &node_labels);

    for sidebar in &old_sidebars {
        commands.entity(sidebar).despawn();
    }
    for indicator in &old_joint_indicators {
        commands.entity(indicator).despawn();
    }
    if let Some((entity, _, random, scalar, _)) = graph_nodes
        .iter()
        .find(|(_, node, _, _, _)| node.0 == event.y_node_id)
    {
        let mesh = if let Some(random) = random {
            random_selection_mesh(&random_node_label(random, event.y_node_id))
        } else if scalar.is_some() {
            selection_indicator(SCALAR_NODE_RAD)
        } else {
            selection_indicator(COMPUTE_NODE_RAD)
        };
        commands.entity(entity).with_child((
            JointSelectedIndicator,
            Pickable::IGNORE,
            Mesh2d(meshes.add(mesh)),
            MeshMaterial2d(materials.add(SELECTED_SAMPLE_COLOR)),
            Transform::from_xyz(0.0, 0.0, 1.1),
        ));
    }
    let sidebar = commands
        .spawn((
            LocalSidebar,
            JointDistributionView {
                x_node_id: event.x_node_id,
                y_node_id: event.y_node_id,
            },
            Node {
                position_type: PositionType::Absolute,
                right: px(0.0),
                top: px(0.0),
                width: px(SIDEBAR_WIDTH),
                height: percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: px(12.0).all(),
                row_gap: px(7.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.13, 0.16)),
            ZIndex(90),
        ))
        .observe(|mut event: On<Pointer<Press>>| event.propagate(false))
        .id();
    let heading = commands
        .spawn((
            Pickable::IGNORE,
            Text::new(format!("Joint posterior: {x_label_text} & {y_label_text}")),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..text_font()
            },
        ))
        .id();
    let hint = commands
        .spawn((
            Pickable::IGNORE,
            Text::new("Drag a closed shape over the heatmap to add a sample selection."),
            TextColor(Color::srgb(0.72, 0.74, 0.80)),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..text_font()
            },
        ))
        .id();
    let x_label = commands
        .spawn((
            Pickable::IGNORE,
            Text::new(format!("{x_label_text} (x)")),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..text_font()
            },
        ))
        .id();
    let top_hist = spawn_marginal_histogram(
        &mut commands,
        &x_histogram,
        false,
        SELECTION_INDICATOR_COLOR,
    );
    let body = commands
        .spawn(Node {
            width: percent(100.0),
            height: px(222.0),
            flex_direction: FlexDirection::Row,
            column_gap: px(5.0),
            ..default()
        })
        .id();
    let heatmap = spawn_joint_heatmap(
        &mut commands,
        event.x_node_id,
        event.y_node_id,
        &points,
        &context_plate_ids,
        &context_instance_paths,
        x_histogram.domain,
        y_histogram.domain,
        bins,
        selections.as_deref(),
        plate_selections.as_deref(),
    );
    let right = commands
        .spawn(Node {
            width: px(43.0),
            height: percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    let right_hist =
        spawn_marginal_histogram(&mut commands, &y_histogram, true, SELECTED_SAMPLE_COLOR);
    commands.entity(right).add_child(right_hist);
    commands.entity(body).add_children(&[heatmap, right]);

    let y_label_row = commands
        .spawn(Node {
            width: px(270.0),
            justify_content: JustifyContent::FlexEnd,
            ..default()
        })
        .id();
    let y_label = commands
        .spawn((
            Pickable::IGNORE,
            Node {
                width: px(43.0),
                ..default()
            },
            Text::new(format!("{y_label_text} (y)")),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..text_font()
            },
            TextLayout::justify(Justify::Center),
        ))
        .id();
    commands.entity(y_label_row).add_child(y_label);
    let mut sidebar_children = vec![heading, hint, x_label, top_hist, body, y_label_row];
    if selections
        .as_ref()
        .is_some_and(|selections| !selections.entries.is_empty())
        || plate_selections
            .as_ref()
            .is_some_and(|selections| !selections.ranges_by_plate.is_empty())
    {
        let selected = points
            .iter()
            .filter(|point| {
                sample_is_selected(
                    &context_plate_ids,
                    &context_instance_paths[point.context_instance],
                    point.draw_index,
                    selections.as_deref(),
                    plate_selections.as_deref(),
                )
            })
            .count();
        let probability = selected as f64 / points.len() as f64;
        let probability_text = commands
            .spawn((
                Pickable::IGNORE,
                Text::new(format!(
                    "P(({x_label_text}, {y_label_text}) in selection) = {probability:.4}"
                )),
                TextColor(Color::WHITE),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..text_font()
                },
            ))
            .id();
        sidebar_children.push(probability_text);
    }
    commands.entity(sidebar).add_children(&sidebar_children);
}

fn spawn_marginal_histogram(
    commands: &mut Commands,
    histogram: &Histogram,
    horizontal: bool,
    color: Color,
) -> Entity {
    let bin_extent = if histogram.bins.is_empty() {
        0.0
    } else {
        100.0 / histogram.bins.len() as f32
    };
    let root = commands
        .spawn(Node {
            width: if horizontal {
                percent(100.0)
            } else {
                px(222.0)
            },
            height: if horizontal { percent(100.0) } else { px(55.0) },
            flex_direction: if horizontal {
                FlexDirection::ColumnReverse
            } else {
                FlexDirection::Row
            },
            align_items: if horizontal {
                AlignItems::Stretch
            } else {
                AlignItems::End
            },
            ..default()
        })
        .id();
    for bin in &histogram.bins {
        let fraction = if histogram.max_count == 0.0 {
            0.0
        } else {
            (bin.count / histogram.max_count) as f32 * 100.0
        };
        let slot = commands
            .spawn(Node {
                width: if horizontal {
                    percent(100.0)
                } else {
                    percent(bin_extent)
                },
                height: if horizontal {
                    percent(bin_extent)
                } else {
                    percent(100.0)
                },
                min_width: px(0.0),
                min_height: px(0.0),
                align_items: if horizontal {
                    AlignItems::Center
                } else {
                    AlignItems::End
                },
                ..default()
            })
            .id();
        let bar = commands
            .spawn((
                Pickable::IGNORE,
                Node {
                    width: if horizontal {
                        percent(fraction)
                    } else {
                        percent(100.0)
                    },
                    height: if horizontal {
                        percent(100.0)
                    } else {
                        percent(fraction)
                    },
                    min_width: if horizontal && bin.count > 0.0 {
                        px(1.0)
                    } else {
                        px(0.0)
                    },
                    min_height: if !horizontal && bin.count > 0.0 {
                        px(1.0)
                    } else {
                        px(0.0)
                    },
                    ..default()
                },
                BackgroundColor(color),
            ))
            .id();
        commands.entity(slot).add_child(bar);
        commands.entity(root).add_child(slot);
    }
    root
}

#[allow(clippy::too_many_arguments)]
fn spawn_joint_heatmap(
    commands: &mut Commands,
    x_node_id: u32,
    y_node_id: u32,
    points: &[JointSample],
    context_plate_ids: &[u32],
    context_instance_paths: &[Vec<usize>],
    x_domain: HistogramDomain,
    y_domain: HistogramDomain,
    bins: usize,
    selections: Option<&SampleSelections>,
    plate_selections: Option<&PlateIndexSelections>,
) -> Entity {
    let mut counts = vec![0usize; bins * bins];
    let mut selected_counts = vec![0usize; bins * bins];
    for point in points {
        let x = (x_domain.fraction_for_value(point.x) * bins as f32).floor() as usize;
        let y = (y_domain.fraction_for_value(point.y) * bins as f32).floor() as usize;
        let index = y.min(bins - 1) * bins + x.min(bins - 1);
        counts[index] += 1;
        if sample_is_selected(
            context_plate_ids,
            &context_instance_paths[point.context_instance],
            point.draw_index,
            selections,
            plate_selections,
        ) {
            selected_counts[index] += 1;
        }
    }
    let selected_cells =
        joint_selection_cells(&selected_counts, bins, x_node_id, y_node_id, selections);
    let max_count = counts.iter().copied().max().unwrap_or(1).max(1) as f32;
    let plot = commands
        .spawn((
            JointPlot {
                x_node_id,
                y_node_id,
                x_domain,
                y_domain,
                points: points.to_vec(),
                context_plate_ids: context_plate_ids.to_vec(),
                context_instance_paths: context_instance_paths.to_vec(),
            },
            Node {
                width: px(222.0),
                height: px(222.0),
                display: Display::Grid,
                grid_template_columns: vec![RepeatedGridTrack::flex(bins as u16, 1.0)],
                grid_template_rows: vec![RepeatedGridTrack::flex(bins as u16, 1.0)],
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.90, 0.91, 0.93)),
        ))
        .observe(begin_joint_lasso)
        .observe(update_joint_lasso)
        .observe(finish_joint_lasso)
        .id();
    for screen_y in 0..bins {
        let data_y = bins - 1 - screen_y;
        for x in 0..bins {
            let index = data_y * bins + x;
            let density = counts[index] as f32 / max_count;
            let color = Color::srgb(
                0.90 - density * 0.82,
                0.91 - density * 0.78,
                0.93 - density * 0.60,
            );
            let selected = selected_cells[index];
            let boundary =
                selected.then(|| selected_cell_boundary(&selected_cells, bins, x, data_y));
            let cell = commands
                .spawn((
                    Pickable::IGNORE,
                    Node {
                        width: percent(100.0),
                        height: percent(100.0),
                        // Draw each internal gridline exactly once. A full
                        // sub-pixel border on every cell can round away on
                        // fractional grid tracks or become uneven where two
                        // adjacent borders meet.
                        border: UiRect::new(
                            px(if x > 0 { 1.0 } else { 0.0 }),
                            px(0.0),
                            px(if screen_y > 0 { 1.0 } else { 0.0 }),
                            px(0.0),
                        ),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.2, 0.22, 0.28, 0.35)),
                    BackgroundColor(color),
                ))
                .id();
            if let Some(boundary) = boundary {
                let selection_outline = commands
                    .spawn((
                        Pickable::IGNORE,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0.0),
                            right: px(0.0),
                            top: px(0.0),
                            bottom: px(0.0),
                            border: UiRect::new(
                                px(if boundary.left { 1.8 } else { 0.0 }),
                                px(if boundary.right { 1.8 } else { 0.0 }),
                                px(if boundary.top { 1.8 } else { 0.0 }),
                                px(if boundary.bottom { 1.8 } else { 0.0 }),
                            ),
                            ..default()
                        },
                        BorderColor::all(Color::BLACK),
                        ZIndex(1),
                    ))
                    .id();
                commands.entity(cell).add_child(selection_outline);
            }
            commands.entity(plot).add_child(cell);
        }
    }
    plot
}

pub(super) fn joint_selection_cells(
    selected_counts: &[usize],
    bins: usize,
    x_node_id: u32,
    y_node_id: u32,
    selections: Option<&SampleSelections>,
) -> Vec<bool> {
    let mut selected = selected_counts
        .iter()
        .map(|&count| count > 0)
        .collect::<Vec<_>>();
    let Some(selections) = selections else {
        return selected;
    };

    // Sample occupancy is intentionally sparse, so it cannot describe the
    // interior of a lasso. Rasterize matching joint polygons into the same
    // data-space grid to produce a closed, continuous selection boundary.
    for polygon in selections
        .entries
        .iter()
        .filter_map(|selection| match &selection.source {
            SelectionSource::Joint {
                x_node_id: x,
                y_node_id: y,
                polygon,
            } if *x == x_node_id && *y == y_node_id => Some(polygon),
            _ => None,
        })
    {
        for data_y in 0..bins {
            for x in 0..bins {
                let cell_center = Vec2::new(
                    (x as f32 + 0.5) / bins as f32,
                    1.0 - (data_y as f32 + 0.5) / bins as f32,
                );
                if point_in_polygon(cell_center, polygon) {
                    selected[data_y * bins + x] = true;
                }
            }
        }
    }
    selected
}

pub(super) fn selected_cell_boundary(
    selected_cells: &[bool],
    bins: usize,
    x: usize,
    data_y: usize,
) -> SelectedCellBoundary {
    let is_selected = |x: usize, y: usize| selected_cells[y * bins + x];
    SelectedCellBoundary {
        left: x == 0 || !is_selected(x - 1, data_y),
        right: x + 1 == bins || !is_selected(x + 1, data_y),
        top: data_y + 1 == bins || !is_selected(x, data_y + 1),
        bottom: data_y == 0 || !is_selected(x, data_y - 1),
    }
}

fn begin_joint_lasso(
    mut event: On<Pointer<Press>>,
    ui_scale: Res<UiScale>,
    mut commands: Commands,
    plots: Query<
        (
            &ComputedNode,
            &ComputedUiRenderTargetInfo,
            &UiGlobalTransform,
        ),
        With<JointPlot>,
    >,
    dashes: Query<Entity, With<ActiveJointLassoDash>>,
) {
    event.propagate(false);
    if event.button != PointerButton::Primary {
        return;
    }
    let Ok((computed, target, transform)) = plots.get(event.entity) else {
        return;
    };
    let Some(point) = plot_pointer_fractions(
        event.pointer_location.position,
        computed,
        target,
        transform,
        &ui_scale,
    ) else {
        return;
    };
    for dash in &dashes {
        commands.entity(dash).despawn();
    }
    commands.entity(event.entity).insert(JointLasso {
        points: vec![point],
    });
}

fn update_joint_lasso(
    mut event: On<Pointer<Drag>>,
    ui_scale: Res<UiScale>,
    mut commands: Commands,
    dashes: Query<Entity, With<ActiveJointLassoDash>>,
    mut plots: Query<
        (
            &mut JointLasso,
            &ComputedNode,
            &ComputedUiRenderTargetInfo,
            &UiGlobalTransform,
        ),
        With<JointPlot>,
    >,
) {
    event.propagate(false);
    let Ok((mut lasso, computed, target, transform)) = plots.get_mut(event.entity) else {
        return;
    };
    let Some(point) = plot_pointer_fractions(
        event.pointer_location.position,
        computed,
        target,
        transform,
        &ui_scale,
    ) else {
        return;
    };
    if lasso
        .points
        .last()
        .is_some_and(|last| last.distance(point) < 0.012)
    {
        return;
    }
    lasso.points.push(point);
    for dash in &dashes {
        commands.entity(dash).despawn();
    }
    let content_box = computed.content_box();
    spawn_active_lasso_dashes(
        &mut commands,
        event.entity,
        &lasso.points,
        Vec2::new(content_box.width(), content_box.height()),
    );
}

fn spawn_active_lasso_dashes(
    commands: &mut Commands,
    plot: Entity,
    points: &[Vec2],
    plot_size: Vec2,
) {
    const DASH_LENGTH: f32 = 5.0;
    const DASH_GAP: f32 = 4.0;
    const DASH_THICKNESS: f32 = 1.25;

    let pattern_length = DASH_LENGTH + DASH_GAP;
    let mut path_distance = 0.0;
    for points in points.windows(2) {
        let start = points[0];
        let end = points[1];
        let delta_pixels = (end - start) * plot_size;
        let segment_length = delta_pixels.length();
        if segment_length <= f32::EPSILON {
            continue;
        }

        let mut cursor = 0.0;
        while cursor < segment_length {
            let pattern_position = (path_distance + cursor) % pattern_length;
            let drawing_dash = pattern_position < DASH_LENGTH;
            let distance_to_transition = if drawing_dash {
                DASH_LENGTH - pattern_position
            } else {
                pattern_length - pattern_position
            };
            let next_cursor = (cursor + distance_to_transition).min(segment_length);
            if drawing_dash {
                let dash_start = start + (end - start) * (cursor / segment_length);
                let dash_end = start + (end - start) * (next_cursor / segment_length);
                let midpoint = (dash_start + dash_end) * 0.5;
                let dash_length = next_cursor - cursor;
                let angle = delta_pixels.y.atan2(delta_pixels.x);
                let dash = commands
                    .spawn((
                        ActiveJointLassoDash,
                        Pickable::IGNORE,
                        Node {
                            position_type: PositionType::Absolute,
                            left: percent(midpoint.x * 100.0),
                            top: percent(midpoint.y * 100.0),
                            width: px(dash_length),
                            height: px(DASH_THICKNESS),
                            ..default()
                        },
                        UiTransform {
                            translation: Val2::px(-dash_length * 0.5, -DASH_THICKNESS * 0.5),
                            rotation: Rot2::radians(angle),
                            ..default()
                        },
                        BackgroundColor(Color::BLACK),
                        ZIndex(5),
                    ))
                    .id();
                commands.entity(plot).add_child(dash);
            }
            cursor = next_cursor;
        }
        path_distance += segment_length;
    }
}

fn finish_joint_lasso(
    mut event: On<Pointer<DragEnd>>,
    mut commands: Commands,
    mut selections: Option<ResMut<SampleSelections>>,
    plots: Query<(&JointPlot, &JointLasso)>,
    dashes: Query<Entity, With<ActiveJointLassoDash>>,
    histogram_view: Option<Single<&HistogramView>>,
) {
    event.propagate(false);
    let Ok((plot, lasso)) = plots.get(event.entity) else {
        return;
    };
    commands.entity(event.entity).remove::<JointLasso>();
    for dash in &dashes {
        commands.entity(dash).despawn();
    }
    if lasso.points.len() < 3 {
        return;
    }
    let mut draws_by_instance = HashMap::<Vec<usize>, HashSet<usize>>::new();
    for point in &plot.points {
        let normalized = Vec2::new(
            plot.x_domain.fraction_for_value(point.x),
            1.0 - plot.y_domain.fraction_for_value(point.y),
        );
        if point_in_polygon(normalized, &lasso.points) {
            draws_by_instance
                .entry(plot.context_instance_paths[point.context_instance].clone())
                .or_default()
                .insert(point.draw_index);
        }
    }
    if draws_by_instance.is_empty() {
        return;
    }
    let selection = SampleSelection {
        source: SelectionSource::Joint {
            x_node_id: plot.x_node_id,
            y_node_id: plot.y_node_id,
            polygon: lasso.points.clone(),
        },
        context_plate_ids: plot.context_plate_ids.clone(),
        context_instance_paths: plot.context_instance_paths.clone(),
        draws_by_instance,
    };
    if let Some(selections) = selections.as_mut() {
        selections.entries.push(selection);
    } else {
        commands.insert_resource(SampleSelections {
            entries: vec![selection],
        });
    }
    if let Some(view) = histogram_view {
        reopen_histogram(&mut commands, &view);
    }
    commands.trigger(OpenJointDistributionView {
        x_node_id: plot.x_node_id,
        y_node_id: plot.y_node_id,
    });
}
