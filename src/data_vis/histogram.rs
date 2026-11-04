//! Histogram binning and weighted summary statistics.
use super::*;

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

pub(super) fn build_weighted_histogram(
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
        kind: HistogramKind::Continuous,
        domain,
        bins,
        max_count,
    })
}

pub(super) fn effective_count(samples: &[WeightedSample]) -> f64 {
    samples.iter().map(|sample| sample.weight).sum()
}

pub(super) fn weighted_mean(samples: &[WeightedSample]) -> f64 {
    let count = effective_count(samples);
    samples
        .iter()
        .map(|sample| sample.value * sample.weight)
        .sum::<f64>()
        / count
}

pub(super) fn weighted_standard_deviation(samples: &[WeightedSample]) -> f64 {
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

/// `samples` must already be sorted by value.
pub(super) fn weighted_quantile_sorted(samples: &[WeightedSample], probability: f64) -> f64 {
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
    for sample in samples {
        cumulative += sample.weight;
        if cumulative >= threshold {
            return sample.value;
        }
    }
    samples.last().map_or(f64::NAN, |sample| sample.value)
}

pub(super) fn format_count(count: f64) -> String {
    if (count - count.round()).abs() < 0.05 {
        format!("{count:.0}")
    } else {
        format!("{count:.1}")
    }
}
