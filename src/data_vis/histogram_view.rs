//! Histogram rendering, tooltips, and brush interaction.
use super::*;

pub(super) fn spawn_chart(
    commands: &mut Commands,
    histogram: &Histogram,
    highlighted_histogram: Option<&Histogram>,
    samples: &[HistogramSample],
    instance_paths: &[Vec<usize>],
    plate_ids: &[u32],
    subject: HistogramSubject,
    selections: Option<&SampleSelections>,
    heading_text: &str,
) -> Entity {
    let chart = commands
        .spawn(Node {
            flex_grow: 1.0,
            min_width: px(0.0),
            height: percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: px(5.0),
            ..default()
        })
        .id();

    let heading = commands
        .spawn((
            Pickable::IGNORE,
            Text::new(heading_text),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..text_font()
            },
        ))
        .id();
    let legend_text = match subject {
        HistogramSubject::Node(_) if highlighted_histogram.is_some() => {
            "Blue: All samples | Green: Matched by linked selection"
        }
        HistogramSubject::Node(_) => "Blue: All samples",
        HistogramSubject::PlateIndex(_) if highlighted_histogram.is_some() => {
            "Blue: all indices | Green: linked index selection"
        }
        HistogramSubject::PlateIndex(_) => {
            "Uniform plate indices | Drag to add a linked index selection"
        }
        HistogramSubject::ObservedData(_) if highlighted_histogram.is_some() => {
            "Blue: all source rows | Green: matched owner-plate rows"
        }
        HistogramSubject::ObservedData(_) => "All mapped source rows",
    };
    let legend = commands
        .spawn((
            Pickable::IGNORE,
            Text::new(legend_text),
            TextColor(Color::srgb(0.72, 0.74, 0.80)),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..text_font()
            },
        ))
        .id();

    let tooltip = commands
        .spawn((
            HistogramTooltip,
            Pickable::IGNORE,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                top: px(8.0),
                padding: UiRect::axes(px(7.0), px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.07, 0.92)),
            Text::new(""),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..text_font()
            },
            GlobalZIndex(i32::MAX),
        ))
        .id();

    let active_overlay = commands
        .spawn((
            HistogramBrushOverlay,
            ActiveHistogramBrushOverlay,
            Pickable::IGNORE,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                top: px(0.0),
                bottom: px(0.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.55, 0.75, 1.0, 0.28)),
            ZIndex(3),
        ))
        .id();

    let plot = commands
        .spawn((
            HistogramPlot {
                domain: histogram.domain,
                bins: histogram.bins.clone(),
                highlighted_bins: highlighted_histogram.map(|histogram| histogram.bins.clone()),
                samples: samples.to_vec(),
                instance_paths: instance_paths.to_vec(),
                plate_ids: plate_ids.to_vec(),
                subject,
            },
            Node {
                width: percent(100.0),
                min_width: px(0.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::End,
                // Keep the separator visible when equal-width bins land on
                // fractional pixels (notably the two center bins).
                column_gap: px(2.0),
                padding: UiRect::new(px(8.0), px(8.0), px(8.0), px(0.0)),
                ..default()
            },
            BackgroundColor(Color::WHITE),
        ))
        .observe(update_histogram_tooltip)
        .observe(hide_histogram_tooltip)
        .observe(begin_histogram_brush)
        .observe(update_histogram_brush)
        .observe(finish_histogram_brush)
        .observe(toggle_plate_index)
        .id();

    commands
        .entity(plot)
        .add_children(&[active_overlay, tooltip]);
    if let (Some(selections), HistogramSubject::Node(node_id)) = (selections, subject) {
        for (lower, upper) in selections
            .entries
            .iter()
            .filter_map(|selection| selection.source.histogram_range_for(node_id))
        {
            let overlay = commands
                .spawn((
                    HistogramBrushOverlay,
                    Pickable::IGNORE,
                    BackgroundColor(Color::srgba(0.55, 0.75, 1.0, 0.22)),
                    ZIndex(3),
                ))
                .id();
            set_brush_overlay(
                commands,
                overlay,
                histogram.domain.fraction_for_value(lower),
                histogram.domain.fraction_for_value(upper),
            );
            commands.entity(plot).add_child(overlay);
        }
    }

    let max_count_label = commands
        .spawn((
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                left: px(3.0),
                top: px(2.0),
                ..default()
            },
            Text::new(format_count(histogram.max_count)),
            TextColor(Color::BLACK),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..text_font()
            },
            ZIndex(1),
        ))
        .id();
    commands.entity(plot).add_child(max_count_label);

    for (bin_index, bin) in histogram.bins.iter().enumerate() {
        let slot = commands
            .spawn((
                Pickable::IGNORE,
                Node {
                    flex_grow: 1.0,
                    flex_basis: px(0.0),
                    min_width: px(0.0),
                    height: percent(100.0),
                    align_items: AlignItems::End,
                    ..default()
                },
            ))
            .id();
        let height = if histogram.max_count == 0.0 {
            0.0
        } else {
            (bin.count / histogram.max_count) as f32 * 100.0
        };
        let bar = commands
            .spawn((
                Pickable::IGNORE,
                Node {
                    width: percent(100.0),
                    min_width: px(0.0),
                    height: percent(height),
                    min_height: if bin.count == 0.0 { px(0.0) } else { px(1.0) },
                    ..default()
                },
                BackgroundColor(SAMPLE_COLOR),
            ))
            .id();
        commands.entity(slot).add_child(bar);
        if let Some(highlighted_bin) =
            highlighted_histogram.and_then(|histogram| histogram.bins.get(bin_index))
        {
            let highlighted_height = if histogram.max_count == 0.0 {
                0.0
            } else {
                (highlighted_bin.count / histogram.max_count) as f32 * 100.0
            };
            let highlighted_bar = commands
                .spawn((
                    Pickable::IGNORE,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0.0),
                        right: px(0.0),
                        bottom: px(0.0),
                        height: percent(highlighted_height),
                        min_height: if highlighted_bin.count == 0.0 {
                            px(0.0)
                        } else {
                            px(1.0)
                        },
                        ..default()
                    },
                    BackgroundColor(LINKED_SELECTION_COLOR),
                    ZIndex(2),
                ))
                .id();
            commands.entity(slot).add_child(highlighted_bar);
        }
        commands.entity(plot).add_child(slot);
    }

    let axis = commands
        .spawn(Node {
            width: percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .id();
    let axis_labels = match subject {
        HistogramSubject::PlateIndex(_) => {
            ["0".to_string(), samples.len().saturating_sub(1).to_string()]
        }
        _ => [
            format!("{:.4}", histogram.domain.min),
            format!("{:.4}", histogram.domain.max),
        ],
    };
    for label in axis_labels {
        let text = commands
            .spawn((
                Pickable::IGNORE,
                Text::new(label),
                TextColor(Color::WHITE),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..text_font()
                },
            ))
            .id();
        commands.entity(axis).add_child(text);
    }

    commands
        .entity(chart)
        .add_children(&[heading, legend, plot, axis]);
    chart
}

fn plot_pointer_fraction(
    pointer_position: Vec2,
    computed_node: &ComputedNode,
    target: &ComputedUiRenderTargetInfo,
    transform: &UiGlobalTransform,
    ui_scale: &UiScale,
) -> Option<f32> {
    plot_pointer_fractions(pointer_position, computed_node, target, transform, ui_scale)
        .map(|fractions| fractions.x)
}

pub(super) fn plot_pointer_fractions(
    pointer_position: Vec2,
    computed_node: &ComputedNode,
    target: &ComputedUiRenderTargetInfo,
    transform: &UiGlobalTransform,
    ui_scale: &UiScale,
) -> Option<Vec2> {
    let content_box = computed_node.content_box();
    let local_position = transform
        .try_inverse()?
        .transform_point2(pointer_position * target.scale_factor() / ui_scale.0);
    let width = content_box.width();
    let height = content_box.height();
    (width > 0.0 && height > 0.0).then(|| {
        Vec2::new(
            ((local_position.x - content_box.min.x) / width).clamp(0.0, 1.0),
            ((local_position.y - content_box.min.y) / height).clamp(0.0, 1.0),
        )
    })
}

fn set_brush_overlay(commands: &mut Commands, overlay: Entity, a: f32, b: f32) {
    let left = a.min(b).clamp(0.0, 1.0);
    let right = a.max(b).clamp(0.0, 1.0);
    commands.entity(overlay).insert(Node {
        display: Display::Flex,
        position_type: PositionType::Absolute,
        left: percent(left * 100.0),
        width: percent((right - left) * 100.0),
        top: px(0.0),
        bottom: px(0.0),
        ..default()
    });
}

fn update_histogram_tooltip(
    event: On<Pointer<Move>>,
    ui_scale: Res<UiScale>,
    plots: Query<(
        &HistogramPlot,
        Option<&HistogramBrushStart>,
        &ComputedNode,
        &ComputedUiRenderTargetInfo,
        &UiGlobalTransform,
    )>,
    mut tooltips: Query<(&mut Node, &mut Text), With<HistogramTooltip>>,
) {
    let Ok((plot, brush, computed_node, target, transform)) = plots.get(event.entity) else {
        return;
    };
    if brush.is_some() {
        return;
    }
    let Some(fractions) = plot_pointer_fractions(
        event.pointer_location.position,
        computed_node,
        target,
        transform,
        &ui_scale,
    ) else {
        return;
    };
    let Some(bin_index) = (!plot.bins.is_empty()).then(|| {
        ((fractions.x * plot.bins.len() as f32).floor() as usize).min(plot.bins.len() - 1)
    }) else {
        return;
    };
    let bin = &plot.bins[bin_index];
    let Ok((mut node, mut text)) = tooltips.single_mut() else {
        return;
    };
    node.display = Display::Flex;
    node.left = percent((fractions.x * 100.0 + 1.5).clamp(0.0, 76.0));
    node.top = percent((fractions.y * 100.0 + 3.0).clamp(0.0, 82.0));
    let closing_bracket = if bin_index + 1 == plot.bins.len() {
        "]"
    } else {
        ")"
    };
    text.0 = if let Some(highlighted_bin) = plot
        .highlighted_bins
        .as_ref()
        .and_then(|bins| bins.get(bin_index))
    {
        format!(
            "[{:.4}, {:.4}{closing_bracket}: {} displayed / {} total",
            bin.lower,
            bin.upper,
            format_count(highlighted_bin.count),
            format_count(bin.count)
        )
    } else {
        format!(
            "[{:.4}, {:.4}{closing_bracket}: {}",
            bin.lower,
            bin.upper,
            format_count(bin.count)
        )
    };
}

fn hide_histogram_tooltip(
    _event: On<Pointer<Out>>,
    mut tooltips: Query<&mut Node, With<HistogramTooltip>>,
) {
    if let Ok(mut node) = tooltips.single_mut() {
        node.display = Display::None;
    }
}

fn toggle_plate_index(
    mut event: On<Pointer<Click>>,
    ui_scale: Res<UiScale>,
    mut commands: Commands,
    mut selections: Option<ResMut<PlateIndexSelections>>,
    plots: Query<(
        &HistogramPlot,
        Option<&HistogramBrushStart>,
        &ComputedNode,
        &ComputedUiRenderTargetInfo,
        &UiGlobalTransform,
    )>,
    view: Single<&HistogramView>,
    joint: Option<Single<&JointDistributionView>>,
) {
    let Ok((plot, brush, computed, target, transform)) = plots.get(event.entity) else {
        return;
    };
    let HistogramSubject::PlateIndex(plate_id) = plot.subject else {
        return;
    };
    if event.button != PointerButton::Primary || brush.is_some_and(|brush| brush.dragged) {
        return;
    }
    event.propagate(false);
    let Some(fraction) = plot_pointer_fraction(
        event.pointer_location.position,
        computed,
        target,
        transform,
        &ui_scale,
    ) else {
        return;
    };
    let width = computed.content_box().width();
    let value = plot.domain.value_at_plot_x(fraction * width, width);
    let index = (value + 0.5)
        .floor()
        .clamp(0.0, plot.samples.len().saturating_sub(1) as f64) as usize;
    if let Some(selections) = selections.as_mut() {
        selections.toggle(plate_id, index);
    } else {
        let mut new_selections = PlateIndexSelections::default();
        new_selections.toggle(plate_id, index);
        commands.insert_resource(new_selections);
    }
    commands
        .entity(event.entity)
        .remove::<HistogramBrushStart>();
    reopen_histogram(&mut commands, &view);
    reopen_joint_if_open(&mut commands, joint.map(|joint| *joint));
}

fn begin_histogram_brush(
    mut event: On<Pointer<Press>>,
    ui_scale: Res<UiScale>,
    mut commands: Commands,
    plots: Query<
        (
            &ComputedNode,
            &ComputedUiRenderTargetInfo,
            &UiGlobalTransform,
        ),
        With<HistogramPlot>,
    >,
) {
    event.propagate(false);
    if event.button != PointerButton::Primary {
        return;
    }
    let Ok((computed_node, target, transform)) = plots.get(event.entity) else {
        return;
    };
    let Some(fraction) = plot_pointer_fraction(
        event.pointer_location.position,
        computed_node,
        target,
        transform,
        &ui_scale,
    ) else {
        return;
    };
    commands.entity(event.entity).insert(HistogramBrushStart {
        fraction,
        dragged: false,
    });
}

fn update_histogram_brush(
    mut event: On<Pointer<Drag>>,
    ui_scale: Res<UiScale>,
    mut plots: Query<
        (
            &HistogramPlot,
            &mut HistogramBrushStart,
            &ComputedNode,
            &ComputedUiRenderTargetInfo,
            &UiGlobalTransform,
        ),
        With<HistogramPlot>,
    >,
    mut overlays: Query<&mut Node, (With<ActiveHistogramBrushOverlay>, Without<HistogramTooltip>)>,
    mut tooltips: Query<
        (&mut Node, &mut Text),
        (With<HistogramTooltip>, Without<ActiveHistogramBrushOverlay>),
    >,
) {
    event.propagate(false);
    let Ok((plot, mut brush, computed_node, target, transform)) = plots.get_mut(event.entity)
    else {
        return;
    };
    brush.dragged = true;
    let Some(fractions) = plot_pointer_fractions(
        event.pointer_location.position,
        computed_node,
        target,
        transform,
        &ui_scale,
    ) else {
        return;
    };
    let fraction = fractions.x;
    let Ok(mut overlay) = overlays.single_mut() else {
        return;
    };
    let left = brush.fraction.min(fraction);
    let right = brush.fraction.max(fraction);
    overlay.display = Display::Flex;
    overlay.left = percent(left * 100.0);
    overlay.width = percent((right - left) * 100.0);

    let width = computed_node.content_box().width();
    let lower = plot.domain.value_at_plot_x(left * width, width);
    let upper = plot.domain.value_at_plot_x(right * width, width);
    let sample_count = plot
        .samples
        .iter()
        .filter(|sample| sample.value >= lower && sample.value <= upper)
        .count();
    let selected_fraction = if plot.samples.is_empty() {
        0.0
    } else {
        sample_count as f64 / plot.samples.len() as f64
    };
    if let Ok((mut tooltip, mut text)) = tooltips.single_mut() {
        tooltip.display = Display::Flex;
        tooltip.left = percent((fractions.x * 100.0 + 1.5).clamp(0.0, 76.0));
        tooltip.top = percent((fractions.y * 100.0 + 3.0).clamp(0.0, 82.0));
        text.0 = format!(
            "{sample_count} {} in selection, frac = {selected_fraction:.4}",
            if sample_count == 1 {
                "sample"
            } else {
                "samples"
            }
        );
    }
}

fn finish_histogram_brush(
    mut event: On<Pointer<DragEnd>>,
    ui_scale: Res<UiScale>,
    mut commands: Commands,
    mut selections: Option<ResMut<SampleSelections>>,
    mut plate_selections: Option<ResMut<PlateIndexSelections>>,
    plots: Query<(
        &HistogramPlot,
        &HistogramBrushStart,
        &ComputedNode,
        &ComputedUiRenderTargetInfo,
        &UiGlobalTransform,
    )>,
    view: Single<&HistogramView>,
    joint_view: Option<Single<&JointDistributionView>>,
    mut overlays: Query<&mut Node, (With<ActiveHistogramBrushOverlay>, Without<HistogramTooltip>)>,
    mut tooltips: Query<&mut Node, (With<HistogramTooltip>, Without<ActiveHistogramBrushOverlay>)>,
) {
    event.propagate(false);
    let Ok((plot, brush, computed_node, target, transform)) = plots.get(event.entity) else {
        return;
    };
    commands
        .entity(event.entity)
        .remove::<HistogramBrushStart>();
    if let Ok(mut tooltip) = tooltips.single_mut() {
        tooltip.display = Display::None;
    }
    let Some(fraction) = plot_pointer_fraction(
        event.pointer_location.position,
        computed_node,
        target,
        transform,
        &ui_scale,
    ) else {
        return;
    };
    if (fraction - brush.fraction).abs() * computed_node.content_box().width() < 3.0 {
        if let Ok(mut overlay) = overlays.single_mut() {
            overlay.display = Display::None;
        }
        return;
    }

    let lower_fraction = brush.fraction.min(fraction);
    let upper_fraction = brush.fraction.max(fraction);
    let width = computed_node.content_box().width();
    let lower = plot.domain.value_at_plot_x(lower_fraction * width, width);
    let upper = plot.domain.value_at_plot_x(upper_fraction * width, width);
    if let HistogramSubject::PlateIndex(plate_id) = plot.subject {
        let extent = plot.samples.len();
        let Some((start, end)) = snapped_plate_index_range(lower, upper, extent) else {
            return;
        };
        if let Some(selections) = plate_selections.as_mut() {
            selections.union(plate_id, start, end);
        } else {
            let mut new_selections = PlateIndexSelections::default();
            new_selections.union(plate_id, start, end);
            commands.insert_resource(new_selections);
        }
        reopen_histogram(&mut commands, &view);
        if let Some(joint) = joint_view {
            commands.trigger(OpenJointDistributionView {
                x_node_id: joint.x_node_id,
                y_node_id: joint.y_node_id,
            });
        }
        return;
    }
    let draws_by_instance = selected_instances(&plot.samples, &plot.instance_paths, lower, upper);
    if draws_by_instance.is_empty() {
        if let Ok(mut overlay) = overlays.single_mut() {
            overlay.display = Display::None;
        }
        return;
    }

    let HistogramSubject::Node(source_node_id) = plot.subject else {
        return;
    };
    let selection = SampleSelection {
        source: SelectionSource::Histogram {
            node_id: source_node_id,
            lower,
            upper,
        },
        context_plate_ids: plot.plate_ids.clone(),
        context_instance_paths: plot.instance_paths.clone(),
        draws_by_instance,
    };
    if let Some(selections) = selections.as_mut() {
        selections.entries.push(selection);
    } else {
        commands.insert_resource(SampleSelections {
            entries: vec![selection],
        });
    }
    reopen_histogram(&mut commands, &view);
    if let Some(joint) = joint_view {
        commands.trigger(OpenJointDistributionView {
            x_node_id: joint.x_node_id,
            y_node_id: joint.y_node_id,
        });
    }
}

pub(super) fn snapped_plate_index_range(
    lower: f64,
    upper: f64,
    extent: usize,
) -> Option<(usize, usize)> {
    if extent == 0 {
        return None;
    }
    let snap = |value: f64| {
        (value + 0.5)
            .floor()
            .clamp(0.0, extent.saturating_sub(1) as f64) as usize
    };
    Some((snap(lower.min(upper)), snap(lower.max(upper))))
}

pub(super) fn format_instance_label(node_label: &str, indices: &[usize]) -> String {
    let mut label = node_label.to_string();
    for index in indices {
        label.push_str(&format!("[{index}]"));
    }
    label
}
