use crate::bayesian_core::{NodeInstanceSamples, PosteriorSample};
use crate::bevy_to_fugue::{
    GraphIRResource, InferenceResultResource, InferenceResultState, InferenceStatusResource,
};
use crate::constants::{text_font, ERR_COLOR, SAMPLE_COLOR, SIDEBAR_WIDTH};
use crate::ui::{ClearToasts, ErrorToast};
use bevy::{
    input_focus::{tab_navigation::TabIndex, InputFocus},
    prelude::*,
    text::{EditableText, TextCursorStyle},
};
use std::collections::{HashMap, HashSet};

pub const DEFAULT_HISTOGRAM_BINS: usize = 20;
pub const MAX_HISTOGRAM_BINS: usize = 200;
pub const HISTOGRAM_PANEL_HEIGHT: f32 = 260.0;
const SELECTED_SAMPLE_COLOR: Color = Color::srgb(0.35, 0.55, 0.95);

#[derive(Event)]
pub struct OpenHistogramPanel {
    pub node_id: u32,
    pub bin_count: usize,
}

#[derive(Event)]
pub struct CloseHistogramPanel;

#[derive(Component)]
pub struct InferenceHistogramPanel;

/// The one active cross-histogram brush, retaining draws by source instance.
#[derive(Resource, Clone, Debug)]
pub struct HistogramSelection {
    pub source_node_id: u32,
    pub source_plate_ids: Vec<u32>,
    pub source_instance_paths: Vec<Vec<usize>>,
    pub lower: f64,
    pub upper: f64,
    pub draws_by_instance: HashMap<Vec<usize>, HashSet<usize>>,
}

impl HistogramSelection {
    fn point_count(&self) -> usize {
        self.draws_by_instance.values().map(HashSet::len).sum()
    }
}

/// The currently open posterior histogram and its rendering parameters.
#[derive(Component)]
pub struct HistogramView {
    pub node_id: u32,
    pub bin_count: usize,
    pub displayed_sample_count: f64,
}

#[derive(Component)]
pub struct HistogramSelectionControls;

#[derive(Component)]
pub struct HistogramSelectionStatus;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistogramScope {
    Instance(Vec<usize>),
    Pooled { instance_count: usize },
}

/// Screen-space interaction surface for future posterior brushing.
#[derive(Component)]
pub struct HistogramPlot {
    pub domain: HistogramDomain,
    pub bins: Vec<HistogramBin>,
    pub highlighted_bins: Option<Vec<HistogramBin>>,
    pub samples: Vec<HistogramSample>,
    pub instance_paths: Vec<Vec<usize>>,
    pub plate_ids: Vec<u32>,
    pub source_node_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HistogramSample {
    pub instance: usize,
    pub draw_index: usize,
    pub value: f64,
}

#[derive(Component)]
pub struct HistogramTooltip;

#[derive(Component)]
pub struct HistogramBrushOverlay;

#[derive(Component)]
struct HistogramBrushStart {
    fraction: f32,
    dragged: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HistogramDomain {
    pub min: f64,
    pub max: f64,
}

impl HistogramDomain {
    /// Maps a horizontal position in the plot back into posterior value space.
    pub fn value_at_plot_x(&self, local_x: f32, plot_width: f32) -> f64 {
        let proportion = if plot_width <= 0.0 {
            0.0
        } else {
            (local_x / plot_width).clamp(0.0, 1.0) as f64
        };
        self.min + proportion * (self.max - self.min)
    }

    fn fraction_for_value(&self, value: f64) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistogramBin {
    pub lower: f64,
    pub upper: f64,
    pub count: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    pub domain: HistogramDomain,
    pub bins: Vec<HistogramBin>,
    pub max_count: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WeightedSample {
    value: f64,
    weight: f64,
}

/// Bins one concrete node instance's posterior samples.
///
/// Bins are left-inclusive and right-exclusive, except that the final bin also
/// includes the domain maximum.
pub fn build_histogram(samples: &[PosteriorSample], bin_count: usize) -> Result<Histogram, String> {
    if bin_count == 0 {
        return Err("histogram bin count must be greater than zero".to_string());
    }
    if samples.is_empty() {
        return Err("cannot build a histogram from an empty posterior".to_string());
    }
    if samples.iter().any(|sample| !sample.value.is_finite()) {
        return Err("cannot build a histogram containing non-finite values".to_string());
    }

    let data_min = samples
        .iter()
        .map(|sample| sample.value)
        .min_by(f64::total_cmp)
        .expect("samples were checked as non-empty");
    let data_max = samples
        .iter()
        .map(|sample| sample.value)
        .max_by(f64::total_cmp)
        .expect("samples were checked as non-empty");

    // A non-zero domain keeps constant posteriors renderable and invertible.
    let domain = if data_min == data_max {
        let padding = if data_min == 0.0 {
            0.5
        } else {
            (data_min.abs() * 0.05).max(f64::EPSILON)
        };
        HistogramDomain {
            min: data_min - padding,
            max: data_max + padding,
        }
    } else {
        HistogramDomain {
            min: data_min,
            max: data_max,
        }
    };

    build_histogram_with_domain(samples, bin_count, domain)
}

fn build_histogram_with_domain(
    samples: &[PosteriorSample],
    bin_count: usize,
    domain: HistogramDomain,
) -> Result<Histogram, String> {
    let samples = samples
        .iter()
        .map(|sample| WeightedSample {
            value: sample.value,
            weight: 1.0,
        })
        .collect::<Vec<_>>();
    build_weighted_histogram(&samples, bin_count, domain)
}

fn build_weighted_histogram(
    samples: &[WeightedSample],
    bin_count: usize,
    domain: HistogramDomain,
) -> Result<Histogram, String> {
    if bin_count == 0 {
        return Err("histogram bin count must be greater than zero".to_string());
    }
    if samples.iter().any(|sample| {
        !sample.value.is_finite() || !sample.weight.is_finite() || sample.weight < 0.0
    }) {
        return Err("cannot build a histogram containing invalid values or weights".to_string());
    }
    if !domain.min.is_finite() || !domain.max.is_finite() || domain.max <= domain.min {
        return Err("histogram domain must be finite and non-empty".to_string());
    }

    let bin_width = (domain.max - domain.min) / bin_count as f64;
    let mut bins = (0..bin_count)
        .map(|index| HistogramBin {
            lower: domain.min + index as f64 * bin_width,
            upper: if index + 1 == bin_count {
                domain.max
            } else {
                domain.min + (index + 1) as f64 * bin_width
            },
            count: 0.0,
        })
        .collect::<Vec<_>>();

    for sample in samples {
        let index = (((sample.value - domain.min) / bin_width).floor() as usize).min(bin_count - 1);
        bins[index].count += sample.weight;
    }

    let max_count = bins.iter().map(|bin| bin.count).fold(0.0, f64::max);
    Ok(Histogram {
        domain,
        bins,
        max_count,
    })
}

fn pool_instance_samples(instances: &[NodeInstanceSamples]) -> Vec<PosteriorSample> {
    instances
        .iter()
        .flat_map(|instance| instance.samples.iter().copied())
        .collect()
}

fn histogram_samples(instances: &[NodeInstanceSamples]) -> (Vec<HistogramSample>, Vec<Vec<usize>>) {
    let paths = instances
        .iter()
        .map(|instance| instance.indices.clone())
        .collect();
    let samples = instances
        .iter()
        .enumerate()
        .flat_map(|(instance, values)| {
            values.samples.iter().map(move |sample| HistogramSample {
                instance,
                draw_index: sample.draw_index,
                value: sample.value,
            })
        })
        .collect();
    (samples, paths)
}

fn displayed_instance_samples(
    instances: &[NodeInstanceSamples],
) -> (NodeInstanceSamples, HistogramScope) {
    if instances.len() == 1 {
        (
            instances[0].clone(),
            HistogramScope::Instance(instances[0].indices.clone()),
        )
    } else {
        // A posterior draw contributes one point for every concrete plate
        // instance. Repeated draw indices are therefore intentional here.
        (
            NodeInstanceSamples {
                indices: Vec::new(),
                samples: pool_instance_samples(instances),
            },
            HistogramScope::Pooled {
                instance_count: instances.len(),
            },
        )
    }
}

fn linked_samples(
    instances: &[NodeInstanceSamples],
    selection: &HistogramSelection,
    target_plate_ids: &[u32],
) -> Vec<WeightedSample> {
    let shared_depth = selection
        .source_plate_ids
        .iter()
        .zip(target_plate_ids)
        .take_while(|(source, target)| source == target)
        .count();
    let mut instances_by_prefix = HashMap::<Vec<usize>, usize>::new();
    for indices in &selection.source_instance_paths {
        *instances_by_prefix
            .entry(indices[..shared_depth].to_vec())
            .or_default() += 1;
    }

    let mut weights_by_prefix = HashMap::<Vec<usize>, HashMap<usize, f64>>::new();
    for (indices, draws) in &selection.draws_by_instance {
        let prefix = indices[..shared_depth].to_vec();
        let weight = 1.0 / instances_by_prefix[&prefix] as f64;
        let weights = weights_by_prefix.entry(prefix).or_default();
        for draw in draws {
            *weights.entry(*draw).or_default() += weight;
        }
    }

    instances
        .iter()
        .flat_map(|instance| {
            let weights = weights_by_prefix.get(&instance.indices[..shared_depth]);
            instance.samples.iter().filter_map(move |sample| {
                let weight = weights?.get(&sample.draw_index)?;
                Some(WeightedSample {
                    value: sample.value,
                    weight: *weight,
                })
            })
        })
        .collect()
}

fn unweighted_samples(samples: &[PosteriorSample]) -> Vec<WeightedSample> {
    samples
        .iter()
        .map(|sample| WeightedSample {
            value: sample.value,
            weight: 1.0,
        })
        .collect()
}

fn effective_count(samples: &[WeightedSample]) -> f64 {
    samples.iter().map(|sample| sample.weight).sum()
}

fn selected_instances(
    samples: &[HistogramSample],
    instance_paths: &[Vec<usize>],
    lower: f64,
    upper: f64,
) -> HashMap<Vec<usize>, HashSet<usize>> {
    let mut selected = HashMap::<Vec<usize>, HashSet<usize>>::new();
    for sample in samples
        .iter()
        .filter(|sample| sample.value >= lower && sample.value <= upper)
    {
        selected
            .entry(instance_paths[sample.instance].clone())
            .or_default()
            .insert(sample.draw_index);
    }
    selected
}

pub fn open_histogram_panel(
    event: On<OpenHistogramPanel>,
    mut commands: Commands,
    inference_results: Option<Res<InferenceResultResource>>,
    graph: Option<Res<GraphIRResource>>,
    inference_status: Option<Res<InferenceStatusResource>>,
    selection: Option<Res<HistogramSelection>>,
    old_panels: Query<Entity, With<InferenceHistogramPanel>>,
) {
    commands.trigger(ClearToasts);
    despawn_histogram_panels(&mut commands, &old_panels);

    let Some(results) = inference_results else {
        return;
    };
    let Some(plate_ids) = graph
        .as_ref()
        .and_then(|graph| graph.0.node_plate_path(event.node_id))
        .map(<[u32]>::to_vec)
    else {
        commands.trigger(ErrorToast {
            text: format!("Could not find plate metadata for node {}.", event.node_id),
            color: ERR_COLOR,
        });
        return;
    };
    let instances = match results.0.samples_for_node(event.node_id) {
        Ok(instances) if !instances.is_empty() => instances,
        Ok(_) => {
            commands.trigger(ErrorToast {
                text: format!("Node {} has no posterior values.", event.node_id),
                color: ERR_COLOR,
            });
            return;
        }
        Err(error) => {
            commands.trigger(ErrorToast {
                text: format!("Could not display node {}: {error}", event.node_id),
                color: ERR_COLOR,
            });
            return;
        }
    };

    let is_selection_source = selection
        .as_ref()
        .is_some_and(|selection| selection.source_node_id == event.node_id);
    let (full_samples, scope) = displayed_instance_samples(&instances);
    let (plot_samples, instance_paths) = histogram_samples(&instances);
    let full_weighted_samples = unweighted_samples(&full_samples.samples);
    let highlighted_samples = selection
        .as_ref()
        .filter(|_| !is_selection_source)
        .map(|selection| linked_samples(&instances, selection, &plate_ids));
    let stats_samples = highlighted_samples
        .as_deref()
        .unwrap_or(&full_weighted_samples);
    let bin_count = event.bin_count.clamp(1, MAX_HISTOGRAM_BINS);
    let histogram = match build_histogram(&full_samples.samples, bin_count) {
        Ok(histogram) => histogram,
        Err(error) => {
            commands.trigger(ErrorToast {
                text: format!("Could not build histogram: {error}"),
                color: ERR_COLOR,
            });
            return;
        }
    };
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
            "Live posterior samples — {} / {} draws",
            results.0.n_samples, status.requested_samples
        ),
        Some(status) if status.state == InferenceResultState::Cancelled => format!(
            "Partial posterior — cancelled at {} / {} draws",
            results.0.n_samples, status.requested_samples
        ),
        Some(status) if status.state == InferenceResultState::Failed => format!(
            "Partial posterior — failed at {} / {} draws",
            results.0.n_samples, status.requested_samples
        ),
        _ => "Posterior samples".to_string(),
    };

    let panel = commands
        .spawn((
            InferenceHistogramPanel,
            HistogramView {
                node_id: event.node_id,
                bin_count,
                displayed_sample_count: effective_count(stats_samples),
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
        .observe(|mut event: On<Pointer<Press>>| {
            event.propagate(false);
        })
        .id();

    let stats = spawn_stats(
        &mut commands,
        event.node_id,
        stats_samples,
        &scope,
        bin_count,
    );
    let chart = spawn_chart(
        &mut commands,
        &histogram,
        highlighted_histogram.as_ref(),
        &plot_samples,
        &instance_paths,
        &plate_ids,
        event.node_id,
        selection.as_deref(),
        &heading,
    );
    commands.entity(panel).add_children(&[stats, chart]);
}

pub fn close_histogram_panel(
    _event: On<CloseHistogramPanel>,
    mut commands: Commands,
    panels: Query<Entity, With<InferenceHistogramPanel>>,
) {
    despawn_histogram_panels(&mut commands, &panels);
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
    node_id: u32,
    samples: &[WeightedSample],
    scope: &HistogramScope,
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

    let instance_note = match scope {
        HistogramScope::Instance(indices) => format_instance_label(node_id, indices),
        HistogramScope::Pooled { instance_count } => {
            format!("node#{node_id} ({instance_count} pooled instances)")
        }
    };
    let lines = [
        (instance_note, 18.0),
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
            format!("Median: {:.4}", weighted_quantile(samples, 0.5)),
            14.0,
        ),
        (
            format!(
                "95% CrI: [{:.4}, {:.4}]",
                weighted_quantile(samples, 0.025),
                weighted_quantile(samples, 0.975)
            ),
            14.0,
        ),
    ];

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

fn weighted_mean(samples: &[WeightedSample]) -> f64 {
    let count = effective_count(samples);
    samples
        .iter()
        .map(|sample| sample.value * sample.weight)
        .sum::<f64>()
        / count
}

fn weighted_standard_deviation(samples: &[WeightedSample]) -> f64 {
    let count = effective_count(samples);
    if count <= 1.0 {
        return 0.0;
    }
    let mean = weighted_mean(samples);
    (samples
        .iter()
        .map(|sample| sample.weight * (sample.value - mean).powi(2))
        .sum::<f64>()
        / (count - 1.0))
        .sqrt()
}

fn weighted_quantile(samples: &[WeightedSample], probability: f64) -> f64 {
    let mut samples = samples.to_vec();
    samples.sort_by(|a, b| a.value.total_cmp(&b.value));
    if samples.iter().all(|sample| sample.weight == 1.0) {
        if samples.is_empty() {
            return f64::NAN;
        }
        let position = probability * (samples.len() - 1) as f64;
        let lower = position.floor() as usize;
        let upper = position.ceil() as usize;
        let fraction = position - lower as f64;
        return samples[lower].value + (samples[upper].value - samples[lower].value) * fraction;
    }
    let threshold = probability * effective_count(&samples);
    let mut cumulative = 0.0;
    for sample in &samples {
        cumulative += sample.weight;
        if cumulative >= threshold {
            return sample.value;
        }
    }
    samples.last().map_or(f64::NAN, |sample| sample.value)
}

fn format_count(count: f64) -> String {
    if (count - count.round()).abs() < 0.05 {
        format!("{count:.0}")
    } else {
        format!("{count:.1}")
    }
}

fn spawn_bin_stepper(commands: &mut Commands, bin_count: usize) -> Entity {
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

#[derive(Component)]
pub struct HistogramBinCountInput;

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
) {
    event.propagate(false);
    commands.remove_resource::<HistogramSelection>();
    if let Some(view) = view {
        reopen_histogram(&mut commands, &view, view.bin_count);
    }
}

pub fn update_histogram_selection_controls(
    selection: Option<Res<HistogramSelection>>,
    view: Option<Single<&HistogramView>>,
    mut controls: Query<&mut Node, With<HistogramSelectionControls>>,
    mut statuses: Query<&mut Text, With<HistogramSelectionStatus>>,
) {
    let Ok(mut controls) = controls.single_mut() else {
        return;
    };
    let Some(selection) = selection else {
        controls.display = Display::None;
        return;
    };
    controls.display = Display::Flex;

    let (count, suffix) = match view {
        Some(view) if view.node_id != selection.source_node_id => {
            (view.displayed_sample_count, "displayed")
        }
        _ => (selection.point_count() as f64, "selected"),
    };
    if let Ok(mut status) = statuses.single_mut() {
        status.0 = format!("{} samples {suffix}", format_count(count));
    }
}

fn decrement_histogram_bins(
    mut event: On<Pointer<Click>>,
    mut commands: Commands,
    view: Single<&HistogramView>,
) {
    event.propagate(false);
    reopen_histogram(
        &mut commands,
        &view,
        view.bin_count.saturating_sub(1).max(1),
    );
}

fn increment_histogram_bins(
    mut event: On<Pointer<Click>>,
    mut commands: Commands,
    view: Single<&HistogramView>,
) {
    event.propagate(false);
    reopen_histogram(
        &mut commands,
        &view,
        view.bin_count.saturating_add(1).min(MAX_HISTOGRAM_BINS),
    );
}

fn reopen_histogram(commands: &mut Commands, view: &HistogramView, bin_count: usize) {
    commands.trigger(OpenHistogramPanel {
        node_id: view.node_id,
        bin_count,
    });
}

pub fn apply_typed_histogram_bin_count(
    input_focus: Res<InputFocus>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    inputs: Query<&EditableText, With<HistogramBinCountInput>>,
    view: Option<Single<&HistogramView>>,
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
            reopen_histogram(&mut commands, &view, bin_count);
        }
        _ => {
            commands.trigger(ErrorToast {
                text: format!("Histogram bins must be from 1 to {MAX_HISTOGRAM_BINS}."),
                color: ERR_COLOR,
            });
            reopen_histogram(&mut commands, &view, view.bin_count);
        }
    }
}

fn spawn_chart(
    commands: &mut Commands,
    histogram: &Histogram,
    highlighted_histogram: Option<&Histogram>,
    samples: &[HistogramSample],
    instance_paths: &[Vec<usize>],
    plate_ids: &[u32],
    node_id: u32,
    selection: Option<&HistogramSelection>,
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

    let overlay = commands
        .spawn((
            HistogramBrushOverlay,
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

    if let Some(selection) = selection.filter(|selection| selection.source_node_id == node_id) {
        set_brush_overlay(
            commands,
            overlay,
            histogram.domain.fraction_for_value(selection.lower),
            histogram.domain.fraction_for_value(selection.upper),
        );
    }

    let plot = commands
        .spawn((
            HistogramPlot {
                domain: histogram.domain,
                bins: histogram.bins.clone(),
                highlighted_bins: highlighted_histogram.map(|histogram| histogram.bins.clone()),
                samples: samples.to_vec(),
                instance_paths: instance_paths.to_vec(),
                plate_ids: plate_ids.to_vec(),
                source_node_id: node_id,
            },
            Node {
                width: percent(100.0),
                min_width: px(0.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::End,
                column_gap: px(0.0),
                padding: UiRect::new(px(8.0), px(8.0), px(8.0), px(0.0)),
                overflow: Overflow::clip_x(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.16, 0.17, 0.21)),
        ))
        .observe(update_histogram_tooltip)
        .observe(hide_histogram_tooltip)
        .observe(begin_histogram_brush)
        .observe(deselect_outside_histogram_brush)
        .observe(update_histogram_brush)
        .observe(finish_histogram_brush)
        .id();

    commands.entity(plot).add_children(&[overlay, tooltip]);

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
            TextColor(Color::WHITE),
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
                    BackgroundColor(SELECTED_SAMPLE_COLOR),
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
    for label in [
        format!("{:.4}", histogram.domain.min),
        format!("{:.4}", histogram.domain.max),
    ] {
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

    commands.entity(chart).add_children(&[heading, plot, axis]);
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

fn plot_pointer_fractions(
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
        &ComputedNode,
        &ComputedUiRenderTargetInfo,
        &UiGlobalTransform,
    )>,
    mut tooltips: Query<(&mut Node, &mut Text), With<HistogramTooltip>>,
) {
    let Ok((plot, computed_node, target, transform)) = plots.get(event.entity) else {
        return;
    };
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

fn deselect_outside_histogram_brush(
    mut event: On<Pointer<Click>>,
    ui_scale: Res<UiScale>,
    mut commands: Commands,
    selection: Option<Res<HistogramSelection>>,
    plots: Query<(
        &HistogramPlot,
        &HistogramBrushStart,
        &ComputedNode,
        &ComputedUiRenderTargetInfo,
        &UiGlobalTransform,
    )>,
    view: Single<&HistogramView>,
) {
    event.propagate(false);
    if event.button != PointerButton::Primary {
        return;
    }
    let Ok((plot, brush, computed_node, target, transform)) = plots.get(event.entity) else {
        return;
    };
    if brush.dragged {
        return;
    }
    let Some(selection) =
        selection.filter(|selection| selection.source_node_id == plot.source_node_id)
    else {
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
    let width = computed_node.content_box().width();
    let value = plot.domain.value_at_plot_x(fraction * width, width);
    if value < selection.lower || value > selection.upper {
        commands.remove_resource::<HistogramSelection>();
        reopen_histogram(&mut commands, &view, view.bin_count);
    }
}

fn update_histogram_brush(
    mut event: On<Pointer<Drag>>,
    ui_scale: Res<UiScale>,
    mut plots: Query<
        (
            &mut HistogramBrushStart,
            &ComputedNode,
            &ComputedUiRenderTargetInfo,
            &UiGlobalTransform,
        ),
        With<HistogramPlot>,
    >,
    mut overlays: Query<&mut Node, With<HistogramBrushOverlay>>,
) {
    event.propagate(false);
    let Ok((mut brush, computed_node, target, transform)) = plots.get_mut(event.entity) else {
        return;
    };
    brush.dragged = true;
    let Some(fraction) = plot_pointer_fraction(
        event.pointer_location.position,
        computed_node,
        target,
        transform,
        &ui_scale,
    ) else {
        return;
    };
    let Ok(mut overlay) = overlays.single_mut() else {
        return;
    };
    let left = brush.fraction.min(fraction);
    let right = brush.fraction.max(fraction);
    overlay.display = Display::Flex;
    overlay.left = percent(left * 100.0);
    overlay.width = percent((right - left) * 100.0);
}

fn finish_histogram_brush(
    mut event: On<Pointer<DragEnd>>,
    ui_scale: Res<UiScale>,
    mut commands: Commands,
    selection: Option<Res<HistogramSelection>>,
    plots: Query<(
        &HistogramPlot,
        &HistogramBrushStart,
        &ComputedNode,
        &ComputedUiRenderTargetInfo,
        &UiGlobalTransform,
    )>,
    view: Single<&HistogramView>,
    mut overlays: Query<&mut Node, With<HistogramBrushOverlay>>,
) {
    event.propagate(false);
    let Ok((plot, brush, computed_node, target, transform)) = plots.get(event.entity) else {
        return;
    };
    commands
        .entity(event.entity)
        .remove::<HistogramBrushStart>();
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
        if selection.is_some() {
            reopen_histogram(&mut commands, &view, view.bin_count);
        } else if let Ok(mut overlay) = overlays.single_mut() {
            overlay.display = Display::None;
        }
        return;
    }

    let lower_fraction = brush.fraction.min(fraction);
    let upper_fraction = brush.fraction.max(fraction);
    let width = computed_node.content_box().width();
    let lower = plot.domain.value_at_plot_x(lower_fraction * width, width);
    let upper = plot.domain.value_at_plot_x(upper_fraction * width, width);
    let draws_by_instance = selected_instances(&plot.samples, &plot.instance_paths, lower, upper);
    if draws_by_instance.is_empty() {
        if let Ok(mut overlay) = overlays.single_mut() {
            overlay.display = Display::None;
        }
        return;
    }

    commands.insert_resource(HistogramSelection {
        source_node_id: plot.source_node_id,
        source_plate_ids: plot.plate_ids.clone(),
        source_instance_paths: plot.instance_paths.clone(),
        lower,
        upper,
        draws_by_instance,
    });
    reopen_histogram(&mut commands, &view, view.bin_count);
}

fn format_instance_label(node_id: u32, indices: &[usize]) -> String {
    let mut label = format!("node#{node_id}");
    for index in indices {
        label.push_str(&format!("[{index}]"));
    }
    label
}

#[cfg(test)]
mod tests {
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
        let selection = HistogramSelection {
            source_node_id: 1,
            source_plate_ids: vec![10],
            source_instance_paths: vec![vec![0], vec![1]],
            lower: 0.0,
            upper: 1.0,
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
        let selection = HistogramSelection {
            source_node_id: 1,
            source_plate_ids: Vec::new(),
            source_instance_paths: vec![Vec::new()],
            lower: 4.0,
            upper: 7.0,
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
}
