//! Sample populations and projection of selections across plate contexts.
use super::*;

pub(super) fn pool_instance_samples(instances: &[NodeInstanceSamples]) -> Vec<PosteriorSample> {
    instances
        .iter()
        .flat_map(|instance| instance.samples.iter().copied())
        .collect()
}

pub(super) fn histogram_samples(
    instances: &[NodeInstanceSamples],
) -> (Vec<HistogramSample>, Vec<Vec<usize>>) {
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

pub(super) fn displayed_instance_samples(
    instances: &[NodeInstanceSamples],
) -> (NodeInstanceSamples, HistogramPopulation) {
    if instances.len() == 1 {
        (
            instances[0].clone(),
            HistogramPopulation::Instance(instances[0].indices.clone()),
        )
    } else {
        // A posterior draw contributes one point for every concrete plate
        // instance. Repeated draw indices are therefore intentional here.
        (
            NodeInstanceSamples {
                indices: Vec::new(),
                samples: pool_instance_samples(instances),
            },
            HistogramPopulation::Pooled {
                instance_count: instances.len(),
            },
        )
    }
}

pub(super) fn format_selection_ranges(ranges: &[(usize, usize)]) -> String {
    const DISPLAY_LIMIT: usize = 6;
    let mut display = ranges
        .iter()
        .take(DISPLAY_LIMIT)
        .map(|&(start, end)| {
            if start == end {
                start.to_string()
            } else {
                format!("{start}-{end}")
            }
        })
        .collect::<Vec<_>>();
    if ranges.len() > DISPLAY_LIMIT {
        display.push(format!("... (+{} ranges)", ranges.len() - DISPLAY_LIMIT));
    }
    if display.is_empty() {
        "none".to_string()
    } else {
        display.join(", ")
    }
}

#[cfg(test)]
pub(super) fn linked_samples(
    instances: &[NodeInstanceSamples],
    selection: &SampleSelection,
    target_plate_ids: &[u32],
) -> Vec<WeightedSample> {
    let weights = selection_weights(
        instances,
        &selection.context_plate_ids,
        &selection.context_instance_paths,
        &selection.draws_by_instance,
        target_plate_ids,
    );
    instances
        .iter()
        .flat_map(|instance| {
            instance.samples.iter().filter_map(|sample| {
                Some(WeightedSample {
                    value: sample.value,
                    weight: *weights.get(&(instance.indices.clone(), sample.draw_index))?,
                })
            })
        })
        .collect()
}

fn selection_weights(
    instances: &[NodeInstanceSamples],
    context_plate_ids: &[u32],
    context_instance_paths: &[Vec<usize>],
    draws_by_instance: &HashMap<Vec<usize>, HashSet<usize>>,
    target_plate_ids: &[u32],
) -> HashMap<(Vec<usize>, usize), f64> {
    let shared = shared_plate_positions(context_plate_ids, target_plate_ids);
    let mut instances_by_prefix = HashMap::<Vec<usize>, usize>::new();
    for indices in context_instance_paths {
        *instances_by_prefix
            .entry(project_context_path(indices, &shared, true))
            .or_default() += 1;
    }

    let mut weights_by_prefix = HashMap::<Vec<usize>, HashMap<usize, f64>>::new();
    for (indices, draws) in draws_by_instance {
        let prefix = project_context_path(indices, &shared, true);
        let weight = 1.0 / instances_by_prefix[&prefix] as f64;
        let weights = weights_by_prefix.entry(prefix).or_default();
        for draw in draws {
            *weights.entry(*draw).or_default() += weight;
        }
    }

    let mut result = HashMap::new();
    for instance in instances {
        let key = project_context_path(&instance.indices, &shared, false);
        let Some(weights) = weights_by_prefix.get(&key) else {
            continue;
        };
        for sample in &instance.samples {
            if let Some(weight) = weights.get(&sample.draw_index) {
                result.insert((instance.indices.clone(), sample.draw_index), *weight);
            }
        }
    }
    result
}

fn plate_instance_is_selected(
    target_plate_ids: &[u32],
    target_path: &[usize],
    plate_selections: Option<&PlateIndexSelections>,
) -> bool {
    plate_selections.is_some_and(|selections| {
        target_plate_ids
            .iter()
            .enumerate()
            .any(|(position, &plate_id)| {
                target_path
                    .get(position)
                    .is_some_and(|&index| selections.contains(plate_id, index))
            })
    })
}

pub(super) fn linked_samples_for_selections(
    instances: &[NodeInstanceSamples],
    selections: Option<&SampleSelections>,
    plate_selections: Option<&PlateIndexSelections>,
    target_plate_ids: &[u32],
) -> Vec<WeightedSample> {
    type SelectionGroup = (Vec<u32>, Vec<Vec<usize>>);
    let mut groups = HashMap::<SelectionGroup, HashMap<Vec<usize>, HashSet<usize>>>::new();
    if let Some(selections) = selections {
        for selection in &selections.entries {
            let draws = groups
                .entry((
                    selection.context_plate_ids.clone(),
                    selection.context_instance_paths.clone(),
                ))
                .or_default();
            for (path, selected_draws) in &selection.draws_by_instance {
                draws
                    .entry(path.clone())
                    .or_default()
                    .extend(selected_draws);
            }
        }
    }

    let mut weights = HashMap::<(Vec<usize>, usize), f64>::new();
    for ((plate_ids, instance_paths), draws) in groups {
        for (key, weight) in selection_weights(
            instances,
            &plate_ids,
            &instance_paths,
            &draws,
            target_plate_ids,
        ) {
            weights
                .entry(key)
                .and_modify(|current| *current = current.max(weight))
                .or_insert(weight);
        }
    }
    for instance in instances {
        if plate_instance_is_selected(target_plate_ids, &instance.indices, plate_selections) {
            for sample in &instance.samples {
                weights.insert((instance.indices.clone(), sample.draw_index), 1.0);
            }
        }
    }
    instances
        .iter()
        .flat_map(|instance| {
            instance.samples.iter().filter_map(|sample| {
                Some(WeightedSample {
                    value: sample.value,
                    weight: *weights.get(&(instance.indices.clone(), sample.draw_index))?,
                })
            })
        })
        .collect()
}

fn shared_plate_positions(source: &[u32], target: &[u32]) -> Vec<(usize, usize)> {
    shared_dimension_positions(source, target)
}

fn project_context_path(
    path: &[usize],
    shared: &[(usize, usize)],
    use_source_positions: bool,
) -> Vec<usize> {
    shared
        .iter()
        .map(|(source, target)| {
            path[if use_source_positions {
                *source
            } else {
                *target
            }]
        })
        .collect()
}

pub(super) fn unweighted_samples(samples: &[PosteriorSample]) -> Vec<WeightedSample> {
    samples
        .iter()
        .map(|sample| WeightedSample {
            value: sample.value,
            weight: 1.0,
        })
        .collect()
}

pub(super) fn observed_samples(values: &[f64]) -> Vec<PosteriorSample> {
    values
        .iter()
        .enumerate()
        .map(|(row, &value)| PosteriorSample {
            draw_index: row,
            value,
        })
        .collect()
}

pub(super) fn linked_observed_samples(
    samples: &[PosteriorSample],
    owner_plate_id: u32,
    plate_selections: Option<&PlateIndexSelections>,
) -> Vec<WeightedSample> {
    samples
        .iter()
        .filter(|sample| {
            plate_instance_is_selected(&[owner_plate_id], &[sample.draw_index], plate_selections)
        })
        .map(|sample| WeightedSample {
            value: sample.value,
            weight: 1.0,
        })
        .collect()
}

pub(super) fn selected_instances(
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

pub(super) fn build_joint_samples(
    x_instances: &[NodeInstanceSamples],
    x_plate_ids: &[u32],
    y_instances: &[NodeInstanceSamples],
    y_plate_ids: &[u32],
) -> (Vec<JointSample>, Vec<u32>, Vec<Vec<usize>>) {
    let shared = shared_plate_positions(x_plate_ids, y_plate_ids);
    let mut context_plate_ids = x_plate_ids.to_vec();
    context_plate_ids.extend(
        y_plate_ids
            .iter()
            .filter(|plate| !x_plate_ids.contains(plate))
            .copied(),
    );
    let mut context_paths = Vec::<Vec<usize>>::new();
    let mut context_lookup = HashMap::<Vec<usize>, usize>::new();
    let mut points = Vec::new();

    for x_instance in x_instances {
        for y_instance in y_instances {
            if shared
                .iter()
                .any(|(x, y)| x_instance.indices[*x] != y_instance.indices[*y])
            {
                continue;
            }
            let mut context = x_instance.indices.clone();
            context.extend(
                y_plate_ids
                    .iter()
                    .enumerate()
                    .filter(|(_, plate)| !x_plate_ids.contains(plate))
                    .map(|(index, _)| y_instance.indices[index]),
            );
            let context_instance = *context_lookup.entry(context.clone()).or_insert_with(|| {
                context_paths.push(context);
                context_paths.len() - 1
            });
            let y_by_draw = y_instance
                .samples
                .iter()
                .map(|sample| (sample.draw_index, sample.value))
                .collect::<HashMap<_, _>>();
            points.extend(x_instance.samples.iter().filter_map(|x| {
                Some(JointSample {
                    context_instance,
                    draw_index: x.draw_index,
                    x: x.value,
                    y: *y_by_draw.get(&x.draw_index)?,
                })
            }));
        }
    }
    (points, context_plate_ids, context_paths)
}

pub(super) fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = polygon.len() - 1;
    for current in 0..polygon.len() {
        let a = polygon[current];
        let b = polygon[previous];
        if ((a.y > point.y) != (b.y > point.y))
            && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

pub(super) fn sample_is_selected(
    context_plate_ids: &[u32],
    context_path: &[usize],
    draw_index: usize,
    selections: Option<&SampleSelections>,
    plate_selections: Option<&PlateIndexSelections>,
) -> bool {
    plate_instance_is_selected(context_plate_ids, context_path, plate_selections)
        || selections.is_some_and(|selections| {
            selections.entries.iter().any(|selection| {
                let shared =
                    shared_plate_positions(&selection.context_plate_ids, context_plate_ids);
                let target_key = project_context_path(context_path, &shared, false);
                selection.draws_by_instance.iter().any(|(path, draws)| {
                    draws.contains(&draw_index)
                        && project_context_path(path, &shared, true) == target_key
                })
            })
        })
}
