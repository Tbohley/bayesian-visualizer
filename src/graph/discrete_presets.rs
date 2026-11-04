//! Synthetic, reproducible examples of count inference and sensor fault classification.
use super::*;
use crate::nodes::{Operation, VectorElements};
fn scalar(id: u32, x: f32, y: f32, value: f64) -> PresetNode {
    PresetNode::Scalar {
        id,
        position: [x, y],
        value,
        name: None,
    }
}
fn integer(id: u32, x: f32, y: f32, value: i64) -> PresetNode {
    PresetNode::Integer {
        id,
        position: [x, y],
        value,
        name: None,
    }
}
fn compute(id: u32, x: f32, y: f32, operation: Operation, sources: &[u32]) -> PresetNode {
    PresetNode::Compute {
        id,
        position: [x, y],
        operation,
        parameters: sources
            .iter()
            .enumerate()
            .map(|(index, &id)| PresetParameter {
                name: crate::nodes::compute_params(&operation)
                    .get(index)
                    .map_or("element", |p| p.0),
                source: Some(id),
            })
            .collect(),
    }
}
fn random(
    id: u32,
    x: f32,
    y: f32,
    name: &'static str,
    distribution: &'static str,
    sources: &[u32],
) -> PresetNode {
    PresetNode::Random {
        id,
        position: [x, y],
        name: Some(name),
        distribution,
        parameters: sources
            .iter()
            .enumerate()
            .map(|(index, &id)| PresetParameter {
                name: crate::nodes::distribution_params()[distribution][index].0,
                source: Some(id),
            })
            .collect(),
    }
}
pub(crate) fn discrete_presets() -> Vec<GraphPreset> {
    vec![
        GraphPreset {
            id: "coin_binomial",
            title: "Coin flips: learn the bias",
            study_condition: None,
            description: "Observe 14 heads in 20 synthetic flips. Beta(1,1) prior; the exact posterior is Beta(15,7). Click future for the predictive number of heads in another 20 flips.",
            nodes: vec![
                scalar(1, -230., 160., 1.),
                integer(2, 20., 160., 20),
                random(3, -130., 40., "bias", "Beta", &[1, 1]),
                random(5, 60., -130., "heads", "Binomial", &[2, 3]),
                random(6, 290., -20., "future", "Binomial", &[2, 3]),
            ],
            plates: vec![PresetPlate {
                id: 50,
                bounds: PresetBounds {
                    min: [-30., -200.],
                    max: [160., -70.],
                },
                dataset_id: "coin_counts.csv",
                mapping: vec![PresetMapping {
                    node: 5,
                    column: "heads",
                }],
            }],
        },
        GraphPreset {
            id: "sensor_mixture",
            title: "Sensor faults: a Gaussian mixture",
            study_condition: None,
            description: "Classify 12 synthetic equipment-temperature readings as healthy (category 0, baseline 20 C) or faulty (category 1). Learn the fault fraction and positive excess temperature. Known calibration SDs are 1 C and 2 C. Click state and select a plate row to inspect an individual reading; category labels are identified by the positive shift.",
            nodes: vec![
                scalar(1, -470., 300., 2.),
                scalar(2, -320., 300., 1.),
                random(3, -470., 180., "fault_p", "Beta", &[1, 1]),
                compute(4, -290., 120., Operation::Subtract, &[2, 3]),
                compute(5, -220., 10., Operation::Collect, &[4, 3]),
                scalar(6, -30., 300., 20.),
                scalar(7, 120., 300., 4.),
                scalar(8, 270., 300., 0.5),
                random(9, 170., 180., "shift", "Gamma", &[7, 8]),
                compute(10, 40., 110., Operation::Add, &[6, 9]),
                compute(11, 0., 0., Operation::Collect, &[6, 10]),
                PresetNode::Vector {
                    id: 12,
                    position: [450., 120.],
                    elements: VectorElements::Real(vec![1., 2.]),
                    name: Some("noise"),
                },
                random(13, 200., -120., "state", "Categorical", &[5]),
                compute(14, 350., -160., Operation::Index, &[11, 13]),
                compute(15, 520., -160., Operation::Index, &[12, 13]),
                random(16, 430., -270., "temp", "Normal", &[14, 15]),
            ],
            plates: vec![PresetPlate {
                id: 100,
                bounds: PresetBounds {
                    min: [100., -340.],
                    max: [630., -60.],
                },
                dataset_id: "sensor_faults.csv",
                mapping: vec![PresetMapping {
                    node: 16,
                    column: "temperature",
                }],
            }],
        },
    ]
}
pub(crate) fn discrete_datasets() -> Vec<Dataset> {
    vec![
        Dataset {
            name: "coin_counts.csv".into(),
            n: 1,
            data: std::collections::HashMap::from([("heads".into(), vec![14.])]),
        },
        Dataset {
            name: "sensor_faults.csv".into(),
            n: 12,
            data: std::collections::HashMap::from([(
                "temperature".into(),
                vec![
                    19.2, 20.4, 21.1, 19.8, 27.2, 20.2, 28.8, 19.6, 26.5, 20.9, 19.9, 29.1,
                ],
            )]),
        },
    ]
}
