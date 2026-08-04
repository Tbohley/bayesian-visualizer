use std::collections::HashMap;
use bevy::prelude::*;
use super::*;
use crate::bayesian_core::graph_checks::ModelResult;
use crate::nodes::*;
use crate::bayesian_core::*;
use crate::sidebar::link_params::format_number;
use crate::ui::ErrorToast;
use crate::constants::*;
use crate::graph::*;


pub fn compile(
    _event: On<TriggerCompilation>,
    mut commands: Commands,
    finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    rand_nodes: Query<(Entity, &RandomNode), (Without<ComputeNode>, Without<ScalarNode>)>,
    compute_nodes: Query<(Entity, &ComputeNode), (Without<RandomNode>, Without<ScalarNode>)>,
    scalar_nodes: Query<(Entity, &ScalarNode), (Without<RandomNode>, Without<ComputeNode>)>,
    node_ids: Query<(Entity, &GraphNode)>,
    node_positions: Query<(&GraphNode, &Transform), Without<Plate>>,
    plates: Query<(&GraphNode, &Plate)>,
) {
    let graph = compile_ir(
        &finished_links,
        &rand_nodes,
        &compute_nodes,
        &scalar_nodes,
        &node_ids,
        &node_positions,
        &plates,
    );

    match graph {
        Ok(g) => {
            if let Err(error) = g.validate_plate_semantics() {
                commands.trigger(ErrorToast {
                    color: ERR_COLOR,
                    text: error.clone(),
                });
                println!("{error}");
                commands.remove_resource::<GraphIRResource>();
                return;
            }

            match g.check_cycles() {
                Ok(()) => {
                    commands.trigger(ErrorToast {
                        color: SAMPLE_COLOR,
                        text: String::from(
                            "Graph successfully compiled. No errors detected... yet.",
                        ),
                    });
                    println!("Compiled plates: {:#?}", g.plates);
                    //save graph for other functions
                    commands.insert_resource(GraphIRResource(g));
                }
                Err(node_ids) => {
                    commands.trigger(ErrorToast {
                        color: ERR_COLOR,
                        text: format!(
                            "Graph contains a cycle including node IDs: {:?}",
                            node_ids
                        ),
                    });
                    commands.remove_resource::<GraphIRResource>();
                }
            }
        }
        Err(error) => {
            commands.trigger(ErrorToast {
                color: ERR_COLOR,
                text: error.clone(),
            });
            println!("{error}");
            commands.remove_resource::<GraphIRResource>();
        }
    };
}


pub fn global_sample(
    _event: On<Pointer<Click>>,
    mut commands: Commands,
    finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    rand_nodes: Query<(Entity, &RandomNode), (Without<ComputeNode>, Without<ScalarNode>)>,
    compute_nodes: Query<(Entity, &ComputeNode), (Without<RandomNode>, Without<ScalarNode>)>,
    scalar_nodes: Query<(Entity, &ScalarNode), (Without<RandomNode>, Without<ComputeNode>)>,
    node_ids: Query<(Entity, &GraphNode, &Transform)>,
    graph_resource: Option<ResMut<GraphIRResource>>,
    old_samples: Query<(Entity, &SamplePopup)>
){
    for samp in old_samples.iter(){
        commands.entity(samp.0).despawn();
    }
    let g: GraphIR;
    if let Some(graph) = graph_resource {
        g = graph.into_inner().0.clone();
    }else{
        commands.trigger(ErrorToast{
            text: "Graph not compiled.".to_string(),
            color: ERR_COLOR
        });
        return;
    }
    let sample_res = g.ancestral_sample();
    let vals: HashMap<u32, ModelResult>;
    let order = g.topological_sort().expect("Topological ordering should be validated by compilation.");

    if let Err(e) = sample_res {
        commands.trigger(ErrorToast{
            text: format!("Sampling error: {}", e),
            color: ERR_COLOR
        });
        return;
    }else{vals = sample_res.unwrap();}

    match g.format_model_values(&vals) {
        Ok(output) => println!("Ancestral sample:\n{output}"),
        Err(error) => println!("Could not format ancestral sample: {error}"),
    }
    
    for node_id in order{
        let (_, _, transform) = node_ids.iter()
        .find(|(_, node, _)| node.0 == node_id)
        .expect("node not found");
        let value = vals
            .get(&node_id)
            .expect("sampled node val doesn't exist");
        let console_output = match value {
            ModelResult::Scalar(_) => None,
            ModelResult::Plate(_) => Some(
                g.format_node_value(node_id, value)
                    .unwrap_or_else(|error| format!("Could not format node {node_id}: {error}")),
            ),
        };

        commands.trigger(SampleDisplay{
            pos: Vec2{x: transform.translation.x, y: transform.translation.y},
            val: first_scalar(value)
                .map(format_number)
                .unwrap_or_else(|| "empty".to_string()),
            console_output,
        })
    }



}

pub fn sample_popup(
    event: On<SampleDisplay>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
){
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(100., 30.))),
        MeshMaterial2d(materials.add(ColorMaterial::from_color(SAMPLE_COLOR))),
        SamplePopup {
            timer: Timer::from_seconds(15.0, TimerMode::Once),
            console_output: event.console_output.clone(),
        },
        Pickable {
            should_block_lower: true,
            is_hoverable: true,
        },
        Transform::from_xyz(event.pos.x, event.pos.y + 50., 99.),
        children![(
            Pickable::IGNORE,
            Text2d::new(event.val.clone()),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(14.),
                ..default()
            },
        )],
    ))
    .observe(print_plate_sample);
}

fn first_scalar(value: &ModelResult) -> Option<f64> {
    match value {
        ModelResult::Scalar(value) => Some(*value),
        ModelResult::Plate(values) => values.iter().find_map(first_scalar),
    }
}

fn print_plate_sample(
    mut event: On<Pointer<Click>>,
    popups: Query<&SamplePopup>,
) {
    event.propagate(false);
    let Ok(popup) = popups.get(event.event_target()) else {
        return;
    };
    if let Some(output) = &popup.console_output {
        println!("Plate sample:\n{output}");
    }
}

pub fn tick_sample_popups(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut SamplePopup)>,
) {
    for (entity, mut toast) in &mut q {
        toast.timer.tick(time.delta());

        if toast.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

pub fn compile_ir(
    //commands: Commands,
    finished_links: &Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    rand_nodes: &Query<(Entity, &RandomNode), (Without<ComputeNode>, Without<ScalarNode>)>,
    compute_nodes: &Query<(Entity, &ComputeNode), (Without<RandomNode>, Without<ScalarNode>)>,
    scalar_nodes: &Query<(Entity, &ScalarNode), (Without<RandomNode>, Without<ComputeNode>)>,
    node_ids: &Query<(Entity, &GraphNode)>,
    node_positions: &Query<(&GraphNode, &Transform), Without<Plate>>,
    plates: &Query<(&GraphNode, &Plate)>,
) -> Result<GraphIR, String>
{
    let mut graph = GraphIR::new();

    let param_to_ir = |param: &ParamValue| -> Result<ParamIR, String> {
        let entity = param.1
            .ok_or_else(|| "A node has unspecified parameters!".to_string())?;

        let node_id = node_ids
            .get(entity)
            .map_err(|_| "Parameter references a missing node!".to_string())?
            .1
            .0;
    
        Ok(ParamIR { from_node: node_id, param_name: Some(param.0.to_string()) })
    };

    for (entity, rand) in rand_nodes.into_iter(){
        let node = node_ids.get(entity)
        .map_err(|_| "Node is missing its GraphNode ID")?
        .1;
        let params = rand.params.iter().map(param_to_ir).collect::<Result<Vec<_>, _>>()?;
        graph.nodes.insert(node.0, NodeIR::Random{
            id: node.0,
            label: rand.name.clone(),
            dist_type: rand.dist_type.clone(),
            params: params,
        });
    }

    for (entity, compute) in compute_nodes.into_iter(){
        let node = node_ids.get(entity)
        .map_err(|_| "Node is missing its GraphNode ID")?
        .1;
        let params = compute.params.iter().map(param_to_ir).collect::<Result<Vec<_>, _>>()?;
        graph.nodes.insert(node.0, NodeIR::Compute{
            id: node.0,
            operation: compute.operation,
            params: params,
        });
    }

    for (entity, scalar) in scalar_nodes.into_iter(){
        let node = node_ids.get(entity)
        .map_err(|_| "Node is missing its GraphNode ID")?
        .1;
        graph.nodes.insert(node.0, NodeIR::Scalar{
            id: node.0,
            value: scalar.val,
        });
    }

    for (_entity, link) in finished_links.into_iter(){
        graph.edges.push(EdgeIR{
            from: node_ids.get(link.from).unwrap().1.0,
            to: node_ids.get(link.to.unwrap()).unwrap().1.0
        })
    };

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

    for (node, plate) in plates.iter().filter(|(_, plate)| plate.bounds.is_substantial()) {
        if plate.data.data.is_empty() {
            return Err(format!("plate {} has no dataset", node.0));
        }

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

fn compile_plate_irs(
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

            let interiors_overlap = left_bounds.min.x < right_bounds.max.x
                && left_bounds.max.x > right_bounds.min.x
                && left_bounds.min.y < right_bounds.max.y
                && left_bounds.max.y > right_bounds.min.y;

            if interiors_overlap && !left_contains_right && !right_contains_left {
                return Err(format!(
                    "plates {left_id} and {right_id} partially overlap; plates must be disjoint or fully nested"
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
            .filter(|(candidate_id, candidate_bounds, _)| {
                !contained_plates.iter().any(|(middle_id, middle_bounds, _)| {
                    middle_id != candidate_id
                        && middle_bounds.contains_bounds(*candidate_bounds)
                })
            })
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>();
        direct_plates.sort_unstable();

        let mut direct_nodes = nodes
            .iter()
            .filter(|(_, position)| {
                bounds.contains_point(*position)
                    && !contained_plates.iter().any(|(_, child_bounds, _)| {
                        child_bounds.contains_point(*position)
                    })
            })
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        direct_nodes.sort_unstable();

        result.insert(
            plate_id,
            PlateIR {
                id: plate_id,
                n,
                nodes: direct_nodes,
                plates: direct_plates,
                data: HashMap::new(),
                mapping: HashMap::new(),
            },
        );
    }

    let mut node_owners = HashMap::<u32, u32>::new();
    for (&plate_id, plate) in &result {
        for &node_id in &plate.nodes {
            if let Some(previous_owner) = node_owners.insert(node_id, plate_id) {
                return Err(format!(
                    "node {node_id} belongs to multiple sibling plates: {previous_owner} and {plate_id}"
                ));
            }
        }
    }

    Ok(result)
}

//you can tell ai wrote it when there starts to actually be tests...
#[cfg(test)]
mod plate_tests {
    use super::*;

    #[test]
    fn plate_ir_records_direct_nested_contents() {
        let plates = vec![
            (
                1,
                PlateBounds::from_points(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0)),
                10,
            ),
            (
                2,
                PlateBounds::from_points(Vec2::new(20.0, 20.0), Vec2::new(80.0, 80.0)),
                15,
            ),
        ];
        let nodes = vec![
            (1, Vec2::new(10.0, 10.0)),
            (2, Vec2::new(50.0, 50.0)),
            (3, Vec2::new(120.0, 120.0)),
        ];

        let result = compile_plate_irs(&plates, &nodes).unwrap();

        assert_eq!(result[&1].nodes, vec![1]);
        assert_eq!(result[&1].plates, vec![2]);
        assert_eq!(result[&2].nodes, vec![2]);
        assert!(result[&2].plates.is_empty());
    }

    #[test]
    fn plate_ir_rejects_partial_overlap() {
        let plates = vec![
            (
                10,
                PlateBounds::from_points(Vec2::ZERO, Vec2::new(100.0, 100.0)),
                3,
            ),
            (
                11,
                PlateBounds::from_points(Vec2::new(50.0, 50.0), Vec2::new(150.0, 150.0)),
                4,
            ),
        ];

        let error = compile_plate_irs(&plates, &[]).unwrap_err();
        assert!(error.contains("partially overlap"));
    }

    #[test]
    fn plate_ir_allows_touching_sibling_borders() {
        let plates = vec![
            (
                10,
                PlateBounds::from_points(Vec2::ZERO, Vec2::new(50.0, 50.0)),
                3,
            ),
            (
                11,
                PlateBounds::from_points(Vec2::new(50.0, 0.0), Vec2::new(100.0, 50.0)),
                4,
            ),
        ];

        assert!(compile_plate_irs(&plates, &[]).is_ok());
    }

    #[test]
    fn plate_ir_rejects_node_on_shared_sibling_border() {
        let plates = vec![
            (
                10,
                PlateBounds::from_points(Vec2::ZERO, Vec2::new(50.0, 50.0)),
                3,
            ),
            (
                11,
                PlateBounds::from_points(Vec2::new(50.0, 0.0), Vec2::new(100.0, 50.0)),
                4,
            ),
        ];
        let nodes = vec![(1, Vec2::new(50.0, 25.0))];

        let error = compile_plate_irs(&plates, &nodes).unwrap_err();
        assert!(error.contains("multiple sibling plates"));
    }
}
