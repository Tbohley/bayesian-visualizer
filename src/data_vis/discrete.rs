//! Discrete plots use integer-centered bins, including unseen categories.
use super::*;
use crate::bayesian_core::{NodeIR, Value, ValueType};

pub(super) fn node_histogram(
    graph: Option<&GraphIRResource>,
    id: u32,
    samples: &[PosteriorSample],
    requested: usize,
) -> Result<Histogram, String> {
    let Some(graph) = graph else {
        return build_histogram(samples, requested);
    };
    if graph.0.node_shapes().get(&id).map(|shape| shape.value_type) != Some(ValueType::INTEGER) {
        return build_histogram(samples, requested);
    }
    if samples.is_empty() {
        return Err("cannot display an empty discrete sample".into());
    }
    let mut min = samples
        .iter()
        .map(|s| s.value)
        .fold(f64::INFINITY, f64::min)
        .max(0.)
        .floor();
    let mut max = samples
        .iter()
        .map(|s| s.value)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil();
    let categorical = matches!(graph.0.graph().nodes.get(&id),Some(NodeIR::Random{dist_type,..}) if dist_type=="Categorical");
    if let Some(NodeIR::Random { dist_type, params }) = graph.0.graph().nodes.get(&id) {
        if dist_type == "Categorical" {
            if let ValueType::Vector { length, .. } =
                graph.0.node_shapes()[&params[0].from_node].value_type
            {
                min = 0.;
                max = length as f64 - 1.;
            }
        } else if dist_type == "Binomial" {
            if let Some(NodeIR::Constant {
                value: Value::Integer(n),
            }) = graph.0.graph().nodes.get(&params[0].from_node)
            {
                if *n < MAX_HISTOGRAM_BINS as i64 {
                    min = 0.;
                    max = *n as f64;
                }
            }
        }
    }
    let width = ((max - min + 1.) / MAX_HISTOGRAM_BINS as f64)
        .ceil()
        .max(1.);
    let bins = ((max - min + 1.) / width).ceil() as usize;
    let domain = HistogramDomain {
        min: min - 0.5,
        max: min - 0.5 + width * bins as f64,
    };
    let mut histogram = build_weighted_histogram(&unweighted_samples(samples), bins, domain)?;
    histogram.kind = if categorical {
        HistogramKind::Categorical
    } else {
        HistogramKind::Integer
    };
    Ok(histogram)
}
pub(super) fn discrete_stats(
    commands: &mut Commands,
    label: &str,
    histogram: &Histogram,
    samples: &[WeightedSample],
) -> Entity {
    let total = effective_count(samples);
    let mode = histogram
        .bins
        .iter()
        .max_by(|a, b| a.count.total_cmp(&b.count))
        .unwrap();
    let mut lines = vec![
        label.to_string(),
        format!("Samples: {}", format_count(total)),
        format!("Mode: {}", bin_label(mode)),
    ];
    if histogram.kind == HistogramKind::Categorical {
        lines.push(format!("Categories: {} (zero-based)", histogram.bins.len()));
        lines.push(format!("P(mode): {:.3}", mode.count / total));
    } else {
        lines.push(format!("Mean: {:.3}", weighted_mean(samples)));
        lines.push(format!("SD: {:.3}", weighted_standard_deviation(samples)));
        lines.push(if histogram.bins[0].upper - histogram.bins[0].lower > 1. {
            "Grouped integer counts".into()
        } else {
            "One bin per integer count".into()
        });
    }
    let root = commands
        .spawn(Node {
            width: px(250.),
            height: percent(100.),
            flex_direction: FlexDirection::Column,
            row_gap: px(6.),
            ..default()
        })
        .id();
    for line in lines {
        commands.entity(root).with_child((
            Pickable::IGNORE,
            Text::new(line),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(14.),
                ..text_font()
            },
        ));
    }
    root
}
pub(super) fn bin_label(bin: &HistogramBin) -> String {
    if bin.upper - bin.lower <= 1.000001 {
        format!("{:.0}", (bin.lower + bin.upper) / 2.)
    } else {
        format!("{:.0}..{:.0}", bin.lower + 0.5, bin.upper - 0.5)
    }
}
