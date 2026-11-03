//! Conversion from editor entities and plate geometry into core graph IR.
use crate::{
    bayesian_core::{GraphIR, NodeIR, ParamIR, PlateIR},
    graph::{Plate, PlateBounds},
    nodes::{
        ComputeNode, GraphNode, NodeDisplay, ParamValue, RandomNode, ScalarNode, VectorNode,
        random_node_label,
    },
};
use bevy::prelude::*;
use std::collections::HashMap;

pub fn compile_ir(
    rand_nodes: &Query<(Entity, &RandomNode), (Without<ComputeNode>, Without<ScalarNode>)>,
    compute_nodes: &Query<(Entity, &ComputeNode), (Without<RandomNode>, Without<ScalarNode>)>,
    scalar_nodes: &Query<(Entity, &ScalarNode), (Without<RandomNode>, Without<ComputeNode>)>,
    vector_nodes: &Query<(Entity, &VectorNode)>,
    node_ids: &Query<(Entity, &GraphNode)>,
    node_positions: &Query<(&GraphNode, &Transform), Without<Plate>>,
    plates: &Query<(&GraphNode, &Plate)>,
) -> Result<GraphIR, String> {
    let mut graph = GraphIR::new();

    let param_to_ir = |param: &ParamValue| -> Result<ParamIR, ()> {
        // The sidebar presents both an empty parameter and a stale reference to
        // a deleted entity as "Pick a node", so compilation should report both
        // as the same user-facing validation error.
        let entity = param.1.ok_or(())?;
        let node_id = node_ids.get(entity).map_err(|_| ())?.1.0;

        Ok(ParamIR { from_node: node_id })
    };

    for (entity, rand) in rand_nodes.into_iter() {
        let node = node_ids
            .get(entity)
            .map_err(|_| "Node is missing its GraphNode ID")?
            .1;
        let params = rand
            .params
            .iter()
            .map(|param| {
                param_to_ir(param).map_err(|()| {
                    format!(
                        "node {} has an unassigned parameter!",
                        random_node_label(rand, node.0)
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        graph.nodes.insert(
            node.0,
            NodeIR::Random {
                dist_type: rand.dist_type.clone(),
                params: params,
            },
        );
    }

    for (entity, compute) in compute_nodes.into_iter() {
        let node = node_ids
            .get(entity)
            .map_err(|_| "Node is missing its GraphNode ID")?
            .1;
        let params = compute
            .params
            .iter()
            .map(|param| {
                param_to_ir(param)
                    .map_err(|()| format!("node {} has an unassigned parameter!", compute.label()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        graph.nodes.insert(
            node.0,
            NodeIR::Compute {
                operation: compute.operation,
                params: params,
            },
        );
    }

    for (entity, scalar) in scalar_nodes.into_iter() {
        let node = node_ids
            .get(entity)
            .map_err(|_| "Node is missing its GraphNode ID")?
            .1;
        graph
            .nodes
            .insert(node.0, NodeIR::Scalar { value: scalar.val });
    }

    for (entity, vector) in vector_nodes {
        let id = node_ids
            .get(entity)
            .map_err(|_| "Node is missing its GraphNode ID")?
            .1
            .0;
        graph.nodes.insert(
            id,
            NodeIR::Constant {
                value: vector.elements.value(),
            },
        );
    }

    let plate_bounds = plates
        .iter()
        .filter(|(_, plate)| plate.bounds.is_substantial())
        .map(|(node, plate)| (node.0, plate.bounds, plate.data.n))
        .collect::<Vec<_>>();
    let positions = node_positions
        .iter()
        .map(|(node, transform)| (node.0, transform.translation.truncate()))
        .collect::<Vec<_>>();
    graph.plates = compile_plate_irs(&plate_bounds, &positions)?;

    for (node, plate) in plates
        .iter()
        .filter(|(_, plate)| plate.bounds.is_substantial())
    {
        let plate_ir = graph
            .plates
            .get_mut(&node.0)
            .expect("substantial plates should have compiled IR");
        plate_ir.data = plate.data.data.clone();

        for (&entity, column) in &plate.mapping {
            if column == "unobserved" {
                continue;
            }

            let node_id = node_ids
                .get(entity)
                .map_err(|_| format!("plate {} maps a missing node", node.0))?
                .1
                .0;
            plate_ir.mapping.insert(node_id, column.clone());
        }
    }

    Ok(graph)
}

pub(super) fn compile_plate_irs(
    plates: &[(u32, PlateBounds, usize)],
    nodes: &[(u32, Vec2)],
) -> Result<HashMap<u32, PlateIR>, String> {
    for (index, &(left_id, left_bounds, _)) in plates.iter().enumerate() {
        for &(right_id, right_bounds, _) in &plates[index + 1..] {
            let left_contains_right = left_bounds.contains_bounds(right_bounds);
            let right_contains_left = right_bounds.contains_bounds(left_bounds);

            if left_contains_right && right_contains_left {
                return Err(format!(
                    "plates {left_id} and {right_id} have identical bounds"
                ));
            }
        }
    }

    let mut result = HashMap::new();

    for &(plate_id, bounds, n) in plates {
        let contained_plates = plates
            .iter()
            .filter(|(candidate_id, candidate_bounds, _)| {
                *candidate_id != plate_id && bounds.contains_bounds(*candidate_bounds)
            })
            .copied()
            .collect::<Vec<_>>();

        let mut direct_plates = contained_plates
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>();
        direct_plates.sort_unstable();

        let mut member_nodes = nodes
            .iter()
            .filter(|(_, position)| bounds.contains_point(*position))
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        member_nodes.sort_unstable();

        result.insert(
            plate_id,
            PlateIR {
                id: plate_id,
                n,
                nodes: member_nodes,
                plates: direct_plates,
                data: HashMap::new(),
                mapping: HashMap::new(),
            },
        );
    }

    Ok(result)
}
