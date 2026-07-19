use bevy::input::keyboard::KeyboardInput;
use bevy::input_focus::InputFocus;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::text::EditableText;
use fugue::*;
use fugue::error::FugueError::InvalidParameters;
use rand::thread_rng;
use crate::constants::*;
use crate::sidebar::*;
use crate::ui::*;
use crate::graph::*;
use super::*;


pub fn new_random(
    commands: &mut Commands,
    loc: Vec3,
    node_num: u32,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
){
    commands.spawn((
        GraphNode(node_num),
        Pickable{should_block_lower: true, is_hoverable: true},
        Mesh2d(meshes.add(Circle::new(RANDOM_NODE_RAD))),
        MeshMaterial2d(materials.add(RANDOM_NODE_COLOR)),
        Transform::from_xyz(
            loc.x,
            loc.y,
            1.0),
        RandomNode{      //TODO: move to global sidebar
            name: None,
            dist_type: String::from("Normal"),
            dist: Box::new(Normal::new(0.0, 1.0).unwrap().clone()),
            params: vec![ParamValue("mean", 0.),ParamValue("std_dev", 1.)]
        }
    )).with_child((
        NodeLabel(node_num.to_string()),
        Text2d::new(node_num.to_string()),
        TextColor(NODE_NAME_COLOR),
        Pickable::IGNORE,
        Transform::from_xyz(0.0,0.0,2.0)
    ))
    .observe(on_node_drag)
    .observe(on_node_click);
}

//rename selected node to single-letter name from keyboard
pub fn on_keypress(
    mut kbd: MessageReader<KeyboardInput>,
    mut commands: Commands,
    selected: Option<Single<(Entity, &mut RandomNode), With<Selected>>>,
    labels: Query<(Entity, &NodeLabel, &ChildOf)>
){
    let Some(single) = selected else {
        return;
    };
    let (entity, mut random_node) = single.into_inner();

    //for all keyboard inputs while node is selected
    for event in kbd.read() {
        if !event.state.is_pressed() {
            continue;
        }
        let Some(text) = &event.text else {
            continue;
        };
        //only alphabetic, numbers reserved for unnamed nodes
        if text.chars().count() != 1 || !text.chars().all(|c| c.is_alphabetic()) {
            continue;
        }
        random_node.name = Some(text.to_string());
        
        for (label_entity,_, child_of) in labels.iter() {
            if child_of.parent() == entity {
                commands.entity(label_entity).despawn();
            }
        }

        commands.entity(entity).with_child((
            NodeLabel(text.to_string()),
            Text2d::new(text.to_string()),
            TextColor(NODE_NAME_COLOR),
            Pickable::IGNORE,
            Transform::from_xyz(0.0, 0.0, 2.0),
        ));
        //reload sidebar
        commands.trigger(ReloadSidebar);
    }

}

// Submit the new param when Enter is pressed
pub fn on_enter_clicked(
    input_focus: Res<InputFocus>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut param_textboxes: Query<(&mut EditableText, &ParamTextbox, &Name), Without<ScalarValueTextbox>>,
    mut scalar_textboxes: Query<(&mut EditableText, &Name), With<ScalarValueTextbox>>,
    selected_random: Option<Single<(&mut RandomNode, &Selected)>>,
    selected_scalar: Option<Single<(Entity, &mut ScalarNode, &Selected)>>,
    labels: Query<(Entity, &NodeLabel, &ChildOf)>,
    mut commands: Commands,
) {
    if !keyboard_input.just_pressed(KeyCode::Enter) {
        return;
    }
    let Some(focused_entity) = input_focus.get() else {
        return;
    };

    // Existing random-node param behavior
    if let Ok((mut text_input, param_num, _name)) = param_textboxes.get_mut(focused_entity) {
        let Some(single) = selected_random else {
            return;
        };
        let (mut random_var, _selected) = single.into_inner();
        let num = text_input.value().to_string().parse::<f64>();

        match num {
            Ok(f) => {
                println!(
                    "Node w/{} distribution: {} set to {}",
                    random_var.dist_type,
                    distribution_params()
                        .get(&random_var.dist_type)
                        .unwrap()
                        .get(param_num.0)
                        .unwrap()
                        .0,
                    f
                );
                random_var
                    .params
                    .get_mut(param_num.0)
                    .expect("invalid param_num")
                    .1 = f;
            }
            Err(_e) => {
                println!("Not a valid parameter number!");
                text_input.clear();
            }
        }
        println!("Dist params: {:?}", random_var.params);
        refresh_var_dist(&mut random_var, &mut commands);
        commands.trigger(ReloadSidebar);
        return;
    }

    // Scalar-node value behavior
    if let Ok((mut text_input, _name)) = scalar_textboxes.get_mut(focused_entity) {
        let Some(single) = selected_scalar else {
            return;
        };
        let (scalar_entity, mut scalar_node, _selected) = single.into_inner();
        let num = text_input.value().to_string().parse::<f64>();
        match num {
            Ok(f) => {
                scalar_node.val = f;

                replace_node_label(
                    &mut commands,
                    scalar_entity,
                    f.to_string(),
                    &labels,
                );

                commands.trigger(ReloadSidebar);
            }
            Err(_e) => {
                println!("Not a valid scalar number!");
                text_input.clear();
            }
        }
    }
}


pub fn refresh_var_dist(
    node: &mut RandomNode,
    commands: &mut Commands
) {
    let mut new_param_vals = Vec::<ParamValue>::new();

    //for all default parameters in the truth set for this distribution:
    for new_param_truth in distribution_params().get(&node.dist_type).unwrap() {
        let value = node
            .params
            .iter()
            .find(|old_param_val| old_param_val.0 == new_param_truth.0)
            .map(|old_param_val| old_param_val.1)
            .unwrap_or(new_param_truth.1);
    
        new_param_vals.push(ParamValue(new_param_truth.0, value));
    }
    
    node.params = new_param_vals;
    println!("New params: {:?}", &node.params);


    let p = |i: usize| {
        node.params
            .get(i).unwrap().1
    };

    let e = |err| {
        match err {
            InvalidParameters { distribution, reason, code, context: _ } => {
                commands.trigger(ErrorToast{ color: ERR_COLOR, text: format!("{} failed: {} (Code: {:?}). Please set new parameters and hit enter.", distribution, reason, code)});
            }
            other => {commands.trigger(ErrorToast{ color: ERR_COLOR, text: format!("distribution construction failed: {:?}. Please set new parameters and hit enter.", other)});}
        }
        None
    };

    let new_dist: Option<Box<dyn DistributionDebug<f64>>> = match node.dist_type.as_str() {
        "Normal" => Normal::new(p(0), p(1))
            .map(|d| Some(Box::new(d.clone()) as Box<dyn DistributionDebug<f64>>))
            .unwrap_or_else(e),
        "LogNormal" => LogNormal::new(p(0), p(1))
            .map(|d| Some(Box::new(d.clone()) as Box<dyn DistributionDebug<f64>>))
            .unwrap_or_else(e),
        "Exponential" => Exponential::new(p(0))
            .map(|d| Some(Box::new(d.clone()) as Box<dyn DistributionDebug<f64>>))
            .unwrap_or_else(e),
        "Gamma" => Gamma::new(p(0), p(1))
            .map(|d| Some(Box::new(d.clone()) as Box<dyn DistributionDebug<f64>>))
            .unwrap_or_else(e),
        "Beta" => Beta::new(p(0), p(1))
            .map(|d| Some(Box::new(d.clone()) as Box<dyn DistributionDebug<f64>>))
            .unwrap_or_else(e),
        "Uniform" => Uniform::new(p(0), p(1))
            .map(|d| Some(Box::new(d.clone()) as Box<dyn DistributionDebug<f64>>))
            .unwrap_or_else(e),
        other => {
            commands.trigger(ErrorToast {
                text: format!("unsupported distribution type: {}", other),
                color: ERR_COLOR
            });
            None
        }
    };
    
    if let Some(new_dist) = new_dist {
        node.dist = new_dist;
    }

    let mut rng = thread_rng();
    println!("Node distribution set to: {:?}", node.dist);
    println!("Sample from node: {}", node.dist.sample(&mut rng))
}

pub fn sample_node_toast(
    _event: On<Pointer<Click>>,
    mut node: Single<&mut RandomNode, With<Selected>>,
    mut commands: Commands
) {
    let mut rng = thread_rng();
    refresh_var_dist(&mut node, &mut commands);
    println!("Sample from node: {}", node.dist.sample(&mut rng));
    commands.trigger(ErrorToast{text: format!("Sample from node: {}", node.dist.sample(&mut rng)), color: SAMPLE_COLOR})
}

//store parameters for distributions plus a valid default value
pub fn distribution_params() -> HashMap<String, Vec<ParamValue>> {
    HashMap::from([
        (String::from("Normal"), vec![ParamValue("mean", 0.), ParamValue("std_dev", 1.)]),
        (String::from("LogNormal"), vec![ParamValue("mean", 0.), ParamValue("std_dev", 1.)]),
        (String::from("Gamma"), vec![ParamValue("shape", 1.), ParamValue("scale", 1.)]),
        (String::from("Beta"), vec![ParamValue("alpha", 2.), ParamValue("beta", 2.)]),
        (String::from("Exponential"), vec![ParamValue("rate", 2.)]),
        (String::from("Uniform"), vec![ParamValue("min", 0.), ParamValue("max", 10.)])
    ])
}