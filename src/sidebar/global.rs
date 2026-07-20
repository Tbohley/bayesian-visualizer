use bevy::prelude::*;
use super::*;
use crate::nodes::*;
use crate::bayesian_core::*;
use crate::ui::ErrorToast;
use crate::constants::*;

pub fn compile(
    _event: On<Pointer<Click>>,
    mut commands: Commands,
    finished_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    rand_nodes: Query<(Entity, &RandomNode), (Without<ComputeNode>, Without<ScalarNode>)>,
    compute_nodes: Query<(Entity, &ComputeNode), (Without<RandomNode>, Without<ScalarNode>)>,
    scalar_nodes: Query<(Entity, &ScalarNode), (Without<RandomNode>, Without<ComputeNode>)>,
    node_ids: Query<(Entity, &GraphNode)>
){
    let mut graph = compile_ir(&finished_links, &rand_nodes, &compute_nodes, &scalar_nodes, &node_ids);

    match graph{
        Ok(g) => match g.check_cycles(){
            Ok(()) => {commands.trigger(ErrorToast{
                color: SAMPLE_COLOR,
                text: String::from("Graph successfully compiled. No errors detected... yet.")
            });
            println!("{}", format!("{:?}", g.ancestral_sample()));

        },
            Err(node_ids) => {commands.trigger(ErrorToast{
                color: ERR_COLOR,
                text: String::from(format!("Graph contains a cycle including node IDs: {:?}", node_ids))
            });
            return;
        }
        }
        Err(e) => {commands.trigger(ErrorToast{
            color: ERR_COLOR,
            text: String::from(format!("{}", e))
        });
        println!("{}", e);
        return;
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

pub fn load_global_sidebar(
    mut commands: Commands,
    global: Query<(Entity, &GlobalSidebar)>,
){
    for (sidebar_entity, _comp) in global.iter(){
        commands.entity(sidebar_entity).despawn();
    }
    let global_sidebar_entity = commands.spawn((
        GlobalSidebar,
        Node {
            position_type: PositionType::Absolute,
            left: px(0.),
            top: px(0.),
            width: px(SIDEBAR_WIDTH),
            height: percent(100.),
            flex_direction: FlexDirection::Column,
            padding: px(16).all(),
            ..default()
        },
        BackgroundColor(DARK_GREY.into())
    )).observe(
        //sidebar observes clicks to close distribution context menu
        |_: On<Pointer<Press>>, mut commands: Commands| {
            commands.trigger(CloseContextMenus);
    }).id();
    commands.entity(global_sidebar_entity).with_child(
        (
            Text::new("Bayesian Visualizer"),
            Node {
                margin: px(16).bottom(),
                ..default()
            },
            TextColor(NODE_NAME_COLOR),
        ));

    let nodemode_menu = commands.spawn((
        Name::new("node_mode_context_menu"),
        Button,
        Node {
            width: px(SIDEBAR_WIDTH * 0.75),
            height: px(30),
            border: UiRect::all(px(5)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::BLACK),
        BackgroundColor(Color::BLACK),
        children![(
            Pickable::IGNORE,
            Text::new("Node type"),
            TextColor(Color::WHITE),
            TextShadow::default(),
        )],
    )).observe(|mut event: On<Pointer<Press>>, mut commands: Commands| {
        event.propagate(false);
        println!("Clicked context menu");
        debug!("click: {}", event.pointer_location.position);
        commands.trigger(OpenNodeTypeMenu {
            pos: event.pointer_location.position,
        });
    }).id();

    let compile_button = commands.spawn((
        Name::new("compile_button"),
        Button,
        Node {
            width: px(SIDEBAR_WIDTH * 0.75),
            height: px(30),
            border: UiRect::all(px(5)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::MAX,
            margin: px(4).bottom(),
            ..default()
        },
        BorderColor::all(BUTTON_COLOR),
        BackgroundColor(BUTTON_COLOR),
        children![(
            Pickable::IGNORE,
            Text::new("Compile"),
            TextColor(Color::WHITE),
            TextShadow::default(),
        )],
    )).observe(compile).id();

    commands.entity(global_sidebar_entity).add_child(compile_button);
    commands.entity(global_sidebar_entity).add_child(nodemode_menu);
    //TODO: context menu for selecting which type of node to create
}


//
pub fn on_set_node_mode(
    event: On<Pointer<Press>>,
    menu_items: Query<&ContextMenuItem>,
    mut commands: Commands,
    node_mode: Single<&mut NodeMode>
){
    let target = event.original_event_target();

    if let Ok(item) = menu_items.get(target) {
        //set distribution of node to new dist... or maybe on apply?
        println!("Selected node creation type: {}", item.0);
        node_mode.into_inner().0 = match item.0.as_str() {
            "Random" => NodeType::Random,
            "Compute" => NodeType::Compute,
            "Scalar" => NodeType::Scalar,
            _ => NodeType::Random
        };
        commands.trigger(CloseContextMenus);
        commands.trigger(ReloadSidebar);
        
    }
    
}

pub fn on_open_node_type_menu(
    event: On<OpenNodeTypeMenu>, 
    mut commands: Commands,
) {
    commands.trigger(CloseContextMenus);
    let pos = event.pos;
    println!("open context menu at: {pos}");

    commands
        .spawn((
            Name::new("node type selector"),
            ContextMenu,
            ZIndex(999),
            Node {
                position_type: PositionType::Absolute,
                left: px(pos.x),
                top: px(pos.y),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BorderColor::all(Color::BLACK),
            BackgroundColor(Color::linear_rgb(0.1, 0.1, 0.1)),
            children![
                context_item("Random"),
                context_item("Compute"),
                context_item("Scalar"),
            ],
        ))
        .observe(on_set_node_mode);
}