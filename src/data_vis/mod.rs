//! Shared plot state and public entry points for posterior visualization.
mod discrete;
use discrete::*;
mod controls;
mod histogram;
mod histogram_view;
mod joint;
mod panels;
mod samples;

pub use controls::{
    apply_typed_histogram_bin_count, deselect_histogram_selection,
    update_histogram_selection_controls,
};
pub use histogram::build_histogram;
pub use joint::open_joint_distribution_view;
pub use panels::{close_histogram_panel, open_histogram_panel};

use controls::*;
use histogram::*;
use histogram_view::*;
use panels::*;
use samples::*;

use crate::bayesian_core::shared_dimension_positions;
use crate::bayesian_core::{NodeInstanceSamples, PosteriorSample};
use crate::bevy_to_fugue::{
    GraphIRResource, InferenceResultResource, InferenceResultState, InferenceStatusResource,
};
use crate::constants::{
    ERR_COLOR, SAMPLE_COLOR, SELECTION_INDICATOR_COLOR, SIDEBAR_WIDTH, text_font,
};
use crate::nodes::{
    ComputeNode, GraphNode, NodeLabel, RandomNode, ScalarNode, random_node_label,
    random_selection_mesh,
};
use crate::sidebar::LocalSidebar;
use crate::ui::{ClearToasts, ErrorToast, selection_indicator};
use crate::{COMPUTE_NODE_RAD, SCALAR_NODE_RAD};
use bevy::{
    input_focus::{InputFocus, tab_navigation::TabIndex},
    prelude::*,
    text::{EditableText, TextCursorStyle},
};
use std::collections::{HashMap, HashSet};

pub const DEFAULT_HISTOGRAM_BINS: usize = 20;
pub const MAX_HISTOGRAM_BINS: usize = 200;
pub const HISTOGRAM_PANEL_HEIGHT: f32 = 260.0;
const SELECTED_SAMPLE_COLOR: Color = Color::srgb(0.35, 0.55, 0.95);
pub const LINKED_SELECTION_COLOR: Color = Color::srgb(0.2, 0.7, 0.35);

#[derive(Event)]
pub struct OpenHistogramPanel {
    pub subject: HistogramSubject,
    pub clear_toasts: bool,
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct HistogramBinCount(pub usize);

impl Default for HistogramBinCount {
    fn default() -> Self {
        Self(DEFAULT_HISTOGRAM_BINS)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistogramSubject {
    Node(u32),
    PlateIndex(u32),
    ObservedData(u32),
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct PlateIndexSelections {
    /// Inclusive, sorted, disjoint ranges for each plate's linked selection.
    pub ranges_by_plate: HashMap<u32, Vec<(usize, usize)>>,
}

impl PlateIndexSelections {
    pub fn contains(&self, plate_id: u32, index: usize) -> bool {
        self.ranges_by_plate.get(&plate_id).is_some_and(|ranges| {
            ranges
                .iter()
                .any(|&(start, end)| index >= start && index <= end)
        })
    }

    pub fn union(&mut self, plate_id: u32, start: usize, end: usize) {
        let mut ranges = self.ranges_by_plate.remove(&plate_id).unwrap_or_default();
        ranges.push((start.min(end), start.max(end)));
        ranges.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
        for (start, end) in ranges {
            if let Some(last) = merged.last_mut()
                && start <= last.1.saturating_add(1)
            {
                last.1 = last.1.max(end);
            } else {
                merged.push((start, end));
            }
        }
        self.ranges_by_plate.insert(plate_id, merged);
    }

    pub fn selected_count(&self, plate_id: u32, extent: usize) -> usize {
        self.ranges_by_plate.get(&plate_id).map_or(0, |ranges| {
            ranges
                .iter()
                .map(|&(start, end)| {
                    let start = start.min(extent);
                    let end = end.min(extent.saturating_sub(1));
                    end.saturating_sub(start) + usize::from(start < extent)
                })
                .sum()
        })
    }

    pub fn toggle(&mut self, plate_id: u32, index: usize) {
        let Some(ranges) = self.ranges_by_plate.get(&plate_id) else {
            self.union(plate_id, index, index);
            return;
        };
        if !ranges
            .iter()
            .any(|&(start, end)| index >= start && index <= end)
        {
            self.union(plate_id, index, index);
            return;
        }

        let mut updated = Vec::with_capacity(ranges.len() + 1);
        for &(start, end) in ranges {
            if index < start || index > end {
                updated.push((start, end));
            } else {
                if start < index {
                    updated.push((start, index - 1));
                }
                if index < end {
                    updated.push((index + 1, end));
                }
            }
        }
        if updated.is_empty() {
            self.ranges_by_plate.remove(&plate_id);
        } else {
            self.ranges_by_plate.insert(plate_id, updated);
        }
    }
}

#[derive(Event)]
pub struct CloseHistogramPanel;

#[derive(Event)]
pub struct OpenJointDistributionView {
    pub x_node_id: u32,
    pub y_node_id: u32,
}

#[derive(Component)]
pub struct InferenceHistogramPanel;

/// Viewport-independent posterior sample selections. Brushes and lassos append
/// entries; categorical bin clicks toggle their individual entries.
#[derive(Resource, Clone, Debug, Default)]
pub struct SampleSelections {
    pub entries: Vec<SampleSelection>,
}

#[derive(Clone, Debug)]
pub struct SampleSelection {
    pub source: SelectionSource,
    pub context_plate_ids: Vec<u32>,
    pub context_instance_paths: Vec<Vec<usize>>,
    pub draws_by_instance: HashMap<Vec<usize>, HashSet<usize>>,
}

#[derive(Clone, Debug)]
pub enum SelectionSource {
    Histogram {
        node_id: u32,
        lower: f64,
        upper: f64,
    },
    Joint {
        x_node_id: u32,
        y_node_id: u32,
        polygon: Vec<Vec2>,
    },
}

impl SampleSelections {
    fn point_count(&self) -> usize {
        self.entries.iter().map(SampleSelection::point_count).sum()
    }
}

impl SampleSelection {
    fn point_count(&self) -> usize {
        self.draws_by_instance.values().map(HashSet::len).sum()
    }
}

impl SelectionSource {
    fn histogram_range_for(&self, node_id: u32) -> Option<(f64, f64)> {
        match self {
            Self::Histogram {
                node_id: source,
                lower,
                upper,
            } if *source == node_id => Some((*lower, *upper)),
            _ => None,
        }
    }
}

#[derive(Component, Clone, Copy)]
pub struct JointDistributionView {
    pub x_node_id: u32,
    pub y_node_id: u32,
}

#[derive(Component)]
pub struct JointSelectedIndicator;

#[derive(Component)]
struct JointPlot {
    x_node_id: u32,
    y_node_id: u32,
    x_domain: HistogramDomain,
    y_domain: HistogramDomain,
    points: Vec<JointSample>,
    context_plate_ids: Vec<u32>,
    context_instance_paths: Vec<Vec<usize>>,
}

#[derive(Clone, Debug)]
struct JointSample {
    context_instance: usize,
    draw_index: usize,
    x: f64,
    y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectedCellBoundary {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
}

#[derive(Component)]
struct JointLasso {
    points: Vec<Vec2>,
}

#[derive(Component)]
struct ActiveJointLassoDash;

/// The currently open posterior histogram and its rendering parameters.
#[derive(Component)]
pub struct HistogramView {
    pub subject: HistogramSubject,
    pub bin_count: usize,
    pub displayed_sample_count: f64,
}

#[derive(Component)]
pub struct HistogramSelectionControls;

#[derive(Component)]
pub struct HistogramSelectionStatus;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistogramPopulation {
    Instance(Vec<usize>),
    Pooled { instance_count: usize },
}

/// Screen-space interaction surface for future posterior brushing.
#[derive(Component)]
pub struct HistogramPlot {
    pub kind: HistogramKind,
    pub domain: HistogramDomain,
    pub bins: Vec<HistogramBin>,
    pub highlighted_bins: Option<Vec<HistogramBin>>,
    pub samples: Vec<HistogramSample>,
    pub instance_paths: Vec<Vec<usize>>,
    pub plate_ids: Vec<u32>,
    pub subject: HistogramSubject,
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
struct ActiveHistogramBrushOverlay;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistogramKind {
    Continuous,
    Integer,
    Categorical,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    pub kind: HistogramKind,
    pub domain: HistogramDomain,
    pub bins: Vec<HistogramBin>,
    pub max_count: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WeightedSample {
    value: f64,
    weight: f64,
}

#[cfg(test)]
#[path = "tests/data_vis.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/discrete.rs"]
mod discrete_tests;
