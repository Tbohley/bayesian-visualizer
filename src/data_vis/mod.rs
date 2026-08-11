

use bevy::{
    input_focus::{InputFocus, tab_navigation::TabIndex},
    prelude::*,
    text::{EditableText, TextCursorStyle},
};
use crate::bayesian_core::{NodeInstanceSamples, PosteriorSample};
use crate::bevy_to_fugue::InferenceResultResource;
use crate::constants::{ERR_COLOR, SAMPLE_COLOR, SIDEBAR_WIDTH};
use crate::ui::ErrorToast;

pub const DEFAULT_HISTOGRAM_BINS: usize = 20;
pub const MAX_HISTOGRAM_BINS: usize = 200;
pub const HISTOGRAM_PANEL_HEIGHT: f32 = 260.0;

#[derive(Event)]
pub struct OpenHistogramPanel {
    pub node_id: u32,
    pub bin_count: usize,
}

#[derive(Event)]
pub struct CloseHistogramPanel;

#[derive(Component)]
pub struct InferenceHistogramPanel;

/// The currently displayed posterior slice and its rendering parameters.
#[derive(Component)]
pub struct HistogramView {
    pub node_id: u32,
    pub scope: HistogramScope,
    pub bin_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistogramScope {
    Instance(Vec<usize>),
    Pooled { instance_count: usize },
}

/// Screen-space interaction surface for future posterior brushing.
#[derive(Component)]
pub struct HistogramPlot {
    pub domain: HistogramDomain,
}

#[derive(Component)]
pub struct HistogramBar {
    pub bin_index: usize,
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistogramBin {
    pub lower: f64,
    pub upper: f64,
    pub count: usize,
    /// Draws contributing to this bin, for future linked brushing.
    pub draw_indices: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    pub domain: HistogramDomain,
    pub bins: Vec<HistogramBin>,
    pub max_count: usize,
}

/// Bins one concrete node instance's posterior samples.
///
/// Bins are left-inclusive and right-exclusive, except that the final bin also
/// includes the domain maximum.
pub fn build_histogram(
    samples: &[PosteriorSample],
    bin_count: usize,
) -> Result<Histogram, String> {
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

    let bin_width = (domain.max - domain.min) / bin_count as f64;
    let mut bins = (0..bin_count)
        .map(|index| HistogramBin {
            lower: domain.min + index as f64 * bin_width,
            upper: if index + 1 == bin_count {
                domain.max
            } else {
                domain.min + (index + 1) as f64 * bin_width
            },
            count: 0,
            draw_indices: Vec::new(),
        })
        .collect::<Vec<_>>();

    for sample in samples {
        let index = (((sample.value - domain.min) / bin_width).floor() as usize)
            .min(bin_count - 1);
        bins[index].count += 1;
        bins[index].draw_indices.push(sample.draw_index);
    }

    let max_count = bins.iter().map(|bin| bin.count).max().unwrap_or(0);
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

pub fn open_histogram_panel(
    event: On<OpenHistogramPanel>,
    mut commands: Commands,
    inference_results: Option<Res<InferenceResultResource>>,
    old_panels: Query<Entity, With<InferenceHistogramPanel>>,
) {
    despawn_histogram_panels(&mut commands, &old_panels);

    let Some(results) = inference_results else {
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

    let (displayed_samples, scope) = if instances.len() == 1 {
        (
            instances[0].clone(),
            HistogramScope::Instance(instances[0].indices.clone()),
        )
    } else {
        // A posterior draw contributes one point for every concrete plate
        // instance. Repeated draw indices are therefore intentional here.
        let pooled = pool_instance_samples(&instances);
        (
            NodeInstanceSamples {
                indices: Vec::new(),
                samples: pooled,
            },
            HistogramScope::Pooled {
                instance_count: instances.len(),
            },
        )
    };
    let bin_count = event.bin_count.clamp(1, MAX_HISTOGRAM_BINS);
    let histogram = match build_histogram(&displayed_samples.samples, bin_count) {
        Ok(histogram) => histogram,
        Err(error) => {
            commands.trigger(ErrorToast {
                text: format!("Could not build histogram: {error}"),
                color: ERR_COLOR,
            });
            return;
        }
    };

    let panel = commands
        .spawn((
            InferenceHistogramPanel,
            HistogramView {
                node_id: event.node_id,
                scope: scope.clone(),
                bin_count,
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
        &displayed_samples,
        &scope,
        bin_count,
    );
    let chart = spawn_chart(&mut commands, &histogram);
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
    instance: &NodeInstanceSamples,
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
        (format!("Samples: {}", instance.count()), 14.0),
        (format!("Mean: {:.4}", instance.mean()), 14.0),
        (format!("SD: {:.4}", instance.standard_deviation()), 14.0),
        (format!("Median: {:.4}", instance.median()), 14.0),
        (
            format!("95% CrI: [{:.4}, {:.4}]", instance.lower_95(), instance.upper_95()),
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
                    ..default()
                },
            ))
            .id();
        commands.entity(stats).add_child(text);
    }
    let stepper = spawn_bin_stepper(commands, bin_count);
    commands.entity(stats).add_child(stepper);
    stats
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
                ..default()
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
                    ..default()
                },
            )],
        ))
        .id()
}

fn decrement_histogram_bins(
    mut event: On<Pointer<Click>>,
    mut commands: Commands,
    view: Single<&HistogramView>,
) {
    event.propagate(false);
    reopen_histogram(&mut commands, &view, view.bin_count.saturating_sub(1).max(1));
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

fn spawn_chart(commands: &mut Commands, histogram: &Histogram) -> Entity {
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
            Text::new("Posterior samples"),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
        ))
        .id();

    let plot = commands
        .spawn((
            HistogramPlot {
                domain: histogram.domain,
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
        .id();

    let max_count_label = commands
        .spawn((
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                left: px(3.0),
                top: px(2.0),
                ..default()
            },
            Text::new(histogram.max_count.to_string()),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..default()
            },
            ZIndex(1),
        ))
        .id();
    commands.entity(plot).add_child(max_count_label);

    for (bin_index, bin) in histogram.bins.iter().enumerate() {
        let slot = commands
            .spawn(Node {
                flex_grow: 1.0,
                flex_basis: px(0.0),
                min_width: px(0.0),
                height: percent(100.0),
                align_items: AlignItems::End,
                ..default()
            })
            .id();
        let height = if histogram.max_count == 0 {
            0.0
        } else {
            bin.count as f32 / histogram.max_count as f32 * 100.0
        };
        let bar = commands
            .spawn((
                HistogramBar { bin_index },
                Pickable::IGNORE,
                Node {
                    width: percent(100.0),
                    min_width: px(0.0),
                    height: percent(height),
                    min_height: if bin.count == 0 { px(0.0) } else { px(1.0) },
                    ..default()
                },
                BackgroundColor(SAMPLE_COLOR),
            ))
            .id();
        commands.entity(slot).add_child(bar);
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
                    ..default()
                },
            ))
            .id();
        commands.entity(axis).add_child(text);
    }

    commands.entity(chart).add_children(&[heading, plot, axis]);
    chart
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
    fn bins_samples_and_retains_draw_indices() {
        let samples = vec![
            sample(0, 0.0),
            sample(1, 0.25),
            sample(2, 0.75),
            sample(3, 1.0),
        ];

        let histogram = build_histogram(&samples, 2).unwrap();

        assert_eq!(histogram.domain, HistogramDomain { min: 0.0, max: 1.0 });
        assert_eq!(histogram.bins[0].count, 2);
        assert_eq!(histogram.bins[0].draw_indices, vec![0, 1]);
        assert_eq!(histogram.bins[1].count, 2);
        assert_eq!(histogram.bins[1].draw_indices, vec![2, 3]);
        assert_eq!(histogram.max_count, 2);
    }

    #[test]
    fn constant_samples_get_a_renderable_domain() {
        let histogram = build_histogram(&[sample(0, 4.0), sample(1, 4.0)], 5).unwrap();

        assert!(histogram.domain.min < 4.0);
        assert!(histogram.domain.max > 4.0);
        assert_eq!(histogram.bins.iter().map(|bin| bin.count).sum::<usize>(), 2);
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
        let domain = HistogramDomain { min: -2.0, max: 2.0 };
        assert_eq!(domain.value_at_plot_x(50.0, 100.0), 0.0);
        assert_eq!(domain.value_at_plot_x(-10.0, 100.0), -2.0);
        assert_eq!(domain.value_at_plot_x(200.0, 100.0), 2.0);
    }
}
