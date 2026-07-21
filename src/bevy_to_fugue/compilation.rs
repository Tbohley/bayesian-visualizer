use std::collections::HashMap;
use bevy::prelude::*;
use super::*;
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
){
    let mut graph = compile_ir(&finished_links, &rand_nodes, &compute_nodes, &scalar_nodes, &node_ids);

    match graph{
        Ok(g) => match g.check_cycles(){
            Ok(()) => {commands.trigger(ErrorToast{
                color: SAMPLE_COLOR,
                text: String::from("Graph successfully compiled. No errors detected... yet.")
            });
            println!("{}", format!("{:?}", g.ancestral_sample()));
            //save graph for other functions
            commands.insert_resource(GraphIRResource(g));
        },
            Err(node_ids) => {commands.trigger(ErrorToast{
                color: ERR_COLOR,
                text: String::from(format!("Graph contains a cycle including node IDs: {:?}", node_ids))
            });
            commands.remove_resource::<GraphIRResource>();
            return;
        }
        }
        Err(e) => {commands.trigger(ErrorToast{
            color: ERR_COLOR,
            text: String::from(format!("{}", e))
        });
        println!("{}", e);
        commands.remove_resource::<GraphIRResource>();
        return;
    }
    }
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
    let vals: HashMap<u32, f64>;
    let order = g.topological_sort().expect("Topological ordering should be validated by compilation.");

    if let Err(e) = sample_res {
        commands.trigger(ErrorToast{
            text: format!("Sampling error: {}", e),
            color: ERR_COLOR
        });
        return;
    }else{vals = sample_res.unwrap();}
    
    for node_id in order{
        let (_, _, transform) = node_ids.iter()
        .find(|(_, node, _)| node.0 == node_id)
        .expect("node not found");

        commands.trigger(SampleDisplay{
            pos: Vec2{x: transform.translation.x, y: transform.translation.y},
            val: *vals.get(&node_id).expect("sampled node val doesn't exist")
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
            timer: Timer::from_seconds(5.0, TimerMode::Once),
        },
        Transform::from_xyz(event.pos.x, event.pos.y + 50., 99.),
        children![(
            Text2d::new(format_number(event.val)),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(14.),
                ..default()
            },
        )],
    ));
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
    node_ids: &Query<(Entity, &GraphNode)>
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

    Ok(graph)
}