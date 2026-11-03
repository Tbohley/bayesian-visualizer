//! Histogram bin controls and selection status.
use super::*;

pub(super) fn histogram_selection_probability_line(
    node_label: &str,
    node_id: u32,
    full_samples: &[PosteriorSample],
    selected_samples: Option<&[WeightedSample]>,
    selections: Option<&SampleSelections>,
    plate_selections: Option<&PlateIndexSelections>,
    plate_ids: &[u32],
    graph: Option<&GraphIRResource>,
) -> Option<String> {
    let selected_samples = selected_samples?;
    let total = full_samples.len() as f64;
    if total == 0.0 {
        return None;
    }
    let selected = effective_count(selected_samples);
    let probability = (selected / total).clamp(0.0, 1.0);
    let detailed_bounds = if plate_selections.is_none_or(|plates| plates.ranges_by_plate.is_empty())
    {
        match selections.map(|selections| selections.entries.as_slice()) {
            Some([selection]) => selection.source.histogram_range_for(node_id),
            _ => None,
        }
    } else {
        None
    };
    let Some((lower, upper)) = detailed_bounds else {
        let mut line = format!(
            "Linked selection: {} of {} instance-draws ({:.1}%)",
            format_count(selected),
            format_count(total),
            probability * 100.0,
        );
        if let (Some(plate_selections), Some(graph)) = (plate_selections, graph) {
            let spans = plate_ids
                .iter()
                .filter_map(|plate_id| {
                    plate_selections.ranges_by_plate.get(plate_id)?;
                    let extent = graph.0.graph().plates.get(plate_id)?.n;
                    Some(format!(
                        "{} of {extent} plate {plate_id} indices",
                        plate_selections.selected_count(*plate_id, extent)
                    ))
                })
                .collect::<Vec<_>>();
            if !spans.is_empty() {
                line.push_str(&format!("; spanning {}", spans.join(", ")));
            }
        }
        return Some(line);
    };
    let data_min = full_samples
        .iter()
        .map(|sample| sample.value)
        .min_by(f64::total_cmp)?;
    let data_max = full_samples
        .iter()
        .map(|sample| sample.value)
        .max_by(f64::total_cmp)?;
    Some(format_interval_probability(
        node_label,
        lower,
        upper,
        data_min,
        data_max,
        probability,
    ))
}

pub(super) fn format_interval_probability(
    label: &str,
    lower: f64,
    upper: f64,
    data_min: f64,
    data_max: f64,
    probability: f64,
) -> String {
    let tolerance = ((data_max - data_min).abs() * 1e-9).max(f64::EPSILON * 16.0);
    let reaches_lower = lower <= data_min + tolerance;
    let reaches_upper = upper >= data_max - tolerance;
    match (reaches_lower, reaches_upper) {
        (false, false) => {
            format!("P({label} > {lower:.4} & {label} < {upper:.4}) = {probability:.4}")
        }
        (true, false) => format!("P({label} < {upper:.4}) = {probability:.4}"),
        (false, true) => format!("P({label} > {lower:.4}) = {probability:.4}"),
        (true, true) => format!("P({label} in selection) = {probability:.4}"),
    }
}

pub(super) fn spawn_bin_stepper(commands: &mut Commands, bin_count: usize) -> Entity {
    let row = commands
        .spawn(Node {
            align_items: AlignItems::Center,
            column_gap: px(5.0),
            ..default()
        })
        .id();
    let label = commands
        .spawn((
            Pickable::IGNORE,
            Text::new("Bins:"),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(14.0),
                ..text_font()
            },
        ))
        .id();
    let decrement = spawn_bin_button(commands, "-");
    commands.entity(decrement).observe(decrement_histogram_bins);
    let input = commands
        .spawn((
            HistogramBinCountInput,
            Node {
                width: px(48.0),
                min_height: px(24.0),
                border: px(1.0).all(),
                padding: UiRect::axes(px(5.0), px(2.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.55, 0.57, 0.64)),
            BackgroundColor(Color::srgb(0.16, 0.17, 0.21)),
            EditableText::new(bin_count.to_string()),
            text_font(),
            TextColor(Color::WHITE),
            TextLayout::no_wrap(),
            TextCursorStyle::default(),
            TabIndex(0),
            Name::new("histogram_bin_count_input"),
        ))
        .id();
    let increment = spawn_bin_button(commands, "+");
    commands.entity(increment).observe(increment_histogram_bins);
    commands
        .entity(row)
        .add_children(&[label, decrement, input, increment]);
    row
}

fn spawn_bin_button(commands: &mut Commands, label: &'static str) -> Entity {
    commands
        .spawn((
            Button,
            Node {
                width: px(24.0),
                height: px(24.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.28, 0.30, 0.36)),
            children![(
                Pickable::IGNORE,
                Text::new(label),
                TextColor(Color::WHITE),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..text_font()
                },
            )],
        ))
        .id()
}

pub fn deselect_histogram_selection(
    mut event: On<Pointer<Click>>,
    mut commands: Commands,
    view: Option<Single<&HistogramView>>,
    joint_view: Option<Single<&JointDistributionView>>,
) {
    event.propagate(false);
    clear_linked_selection_resources(&mut commands);
    refresh_selection_views(&mut commands, view, joint_view);
}

pub(super) fn clear_linked_selection_resources(commands: &mut Commands) {
    commands.remove_resource::<SampleSelections>();
    commands.remove_resource::<PlateIndexSelections>();
}

fn refresh_selection_views(
    commands: &mut Commands,
    view: Option<Single<&HistogramView>>,
    joint_view: Option<Single<&JointDistributionView>>,
) {
    if let Some(view) = view {
        reopen_histogram(commands, &view);
    }
    if let Some(joint) = joint_view {
        commands.trigger(OpenJointDistributionView {
            x_node_id: joint.x_node_id,
            y_node_id: joint.y_node_id,
        });
    }
}

pub fn update_histogram_selection_controls(
    selections: Option<Res<SampleSelections>>,
    plate_selections: Option<Res<PlateIndexSelections>>,
    view: Option<Single<&HistogramView>>,
    mut controls: Query<&mut Node, With<HistogramSelectionControls>>,
    mut statuses: Query<&mut Text, With<HistogramSelectionStatus>>,
) {
    let Ok(mut controls) = controls.single_mut() else {
        return;
    };
    let selection_count = selections
        .as_ref()
        .map_or(0, |selections| selections.entries.len());
    let plate_selection_count = plate_selections
        .as_ref()
        .map_or(0, |selections| selections.ranges_by_plate.len());
    if selection_count == 0 && plate_selection_count == 0 {
        controls.display = Display::None;
        return;
    }
    controls.display = Display::Flex;

    let (count, suffix) = match view {
        Some(view) => (view.displayed_sample_count, "displayed"),
        None => (
            selections.as_ref().map_or(0, |s| s.point_count()) as f64,
            "selected",
        ),
    };
    if let Ok(mut status) = statuses.single_mut() {
        status.0 = format!(
            "{} samples {suffix}; {selection_count} value/joint selection{}; {plate_selection_count} plate-index selection{}",
            format_count(count),
            if selection_count == 1 { "" } else { "s" },
            if plate_selection_count == 1 { "" } else { "s" },
        );
    }
}

fn decrement_histogram_bins(
    mut event: On<Pointer<Click>>,
    mut commands: Commands,
    view: Single<&HistogramView>,
    joint: Option<Single<&JointDistributionView>>,
    mut setting: ResMut<HistogramBinCount>,
) {
    event.propagate(false);
    setting.0 = view.bin_count.saturating_sub(1).max(1);
    reopen_histogram(&mut commands, &view);
    reopen_joint_if_open(&mut commands, joint.map(|joint| *joint));
}

fn increment_histogram_bins(
    mut event: On<Pointer<Click>>,
    mut commands: Commands,
    view: Single<&HistogramView>,
    joint: Option<Single<&JointDistributionView>>,
    mut setting: ResMut<HistogramBinCount>,
) {
    event.propagate(false);
    setting.0 = view.bin_count.saturating_add(1).min(MAX_HISTOGRAM_BINS);
    reopen_histogram(&mut commands, &view);
    reopen_joint_if_open(&mut commands, joint.map(|joint| *joint));
}

pub(super) fn reopen_joint_if_open(commands: &mut Commands, joint: Option<&JointDistributionView>) {
    if let Some(joint) = joint {
        commands.trigger(OpenJointDistributionView {
            x_node_id: joint.x_node_id,
            y_node_id: joint.y_node_id,
        });
    }
}

pub(super) fn reopen_histogram(commands: &mut Commands, view: &HistogramView) {
    commands.trigger(OpenHistogramPanel {
        subject: view.subject,
        clear_toasts: true,
    });
}

pub fn apply_typed_histogram_bin_count(
    input_focus: Res<InputFocus>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    inputs: Query<&EditableText, With<HistogramBinCountInput>>,
    view: Option<Single<&HistogramView>>,
    joint: Option<Single<&JointDistributionView>>,
    mut setting: ResMut<HistogramBinCount>,
    mut commands: Commands,
) {
    if !keyboard_input.just_pressed(KeyCode::Enter) {
        return;
    }
    let Some(focused_entity) = input_focus.get() else {
        return;
    };
    let Ok(input) = inputs.get(focused_entity) else {
        return;
    };
    let Some(view) = view else {
        return;
    };
    let parsed = input.value().to_string().trim().parse::<usize>();
    match parsed {
        Ok(bin_count @ 1..=MAX_HISTOGRAM_BINS) => {
            setting.0 = bin_count;
            reopen_histogram(&mut commands, &view);
            reopen_joint_if_open(&mut commands, joint.map(|joint| *joint));
        }
        _ => {
            commands.trigger(ErrorToast {
                text: format!("Histogram bins must be from 1 to {MAX_HISTOGRAM_BINS}."),
                color: ERR_COLOR,
            });
            reopen_histogram(&mut commands, &view);
        }
    }
}

#[derive(Component)]
pub struct HistogramBinCountInput;
