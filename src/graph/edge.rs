use crate::constants::*;
use crate::graph::*;
use crate::nodes::{ComputeNode, RandomNode, ScalarNode};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;

//update arrow transforms connecting to dragged node
pub fn on_node_drag(
    event: On<Pointer<Drag>>,
    reduced_view: Res<ReducedView>,
    mut transforms: Query<&mut Transform>,
    mut mesh_query: Query<&mut Mesh2d>,
    mut graph_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    random_nodes: Query<&RandomNode>,
    compute_nodes: Query<&ComputeNode>,
    scalar_nodes: Query<&ScalarNode>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if reduced_view.active {
        return;
    }
    println!("Dragged a node");
    {
        //update node position
        if let Ok(mut ent) = transforms.get_mut(event.event_target()) {
            ent.translation.x += event.delta.x;
            ent.translation.y -= event.delta.y;
        }
    }
    //update all connected arrow positions/meshes
    for (link_entity, link_component) in graph_links.iter_mut() {
        if event.event_target() == link_component.from
            || event.event_target() == link_component.to.unwrap()
        {
            let from_radius = endpoint_radius(
                link_component.from,
                &random_nodes,
                &compute_nodes,
                &scalar_nodes,
            );
            let to_radius = endpoint_radius(
                link_component.to.unwrap(),
                &random_nodes,
                &compute_nodes,
                &scalar_nodes,
            );
            let (new_transform, new_mesh) = link_transform_helper(
                &link_component,
                &transforms,
                &mut meshes,
                from_radius,
                to_radius,
            ).unwrap();
            if let Ok(mut link_transform) = transforms.get_mut(link_entity) {
                if let Ok(mut link_mesh) = mesh_query.get_mut(link_entity) {
                    *link_transform = new_transform;
                    *link_mesh = new_mesh;
                }
            }
        }
    }
}

//custom arrow mesh constructor function
pub fn arrow_mesh(length: f32) -> Mesh {
    let hw = length / 2.0;
    let hs = ARROW_THICKNESS / 2.0;
    let hh = hs * ARROW_TIP_WIDTH_RATIO;
    let tx = hw - ARROW_TIP_LENGTH;

    let vertices: Vec<[f32; 3]> = vec![
        [-hw, hs, 0.0],  // 0: shaft top-left
        [-hw, -hs, 0.0], // 1: shaft bottom-left
        [tx, hs, 0.0],   // 2: shaft top-right
        [tx, -hs, 0.0],  // 3: shaft bottom-right
        [tx, hh, 0.0],   // 4: head top
        [tx, -hh, 0.0],  // 5: head bottom
        [hw, 0.0, 0.0],  // 6: tip
    ];

    let indices = vec![0u32, 1, 2, 2, 1, 3, 4, 5, 6];

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
    .with_inserted_indices(Indices::U32(indices))
}

pub fn spawn_finished_link(
    commands: &mut Commands,
    from: Entity,
    to: Entity,
    from_pos: Vec3,
    to_pos: Vec3,
    from_radius: f32,
    to_radius: f32,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) -> Entity {
    spawn_link_visual(
        commands,
        GraphLink { from, to: Some(to) },
        from_pos,
        to_pos,
        from_radius,
        to_radius,
        meshes,
        materials,
    )
}

pub fn spawn_link_visual<B: Bundle>(
    commands: &mut Commands,
    marker: B,
    from_pos: Vec3,
    to_pos: Vec3,
    from_radius: f32,
    to_radius: f32,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) -> Entity {
    let (translation, rotation, length) = link_geometry(
        from_pos,
        to_pos,
        from_radius,
        to_radius,
    );
    commands
        .spawn((
            marker,
            Mesh2d(meshes.add(arrow_mesh(length))),
            MeshMaterial2d(materials.add(ARROW_COLOR)),
            Transform {
                translation,
                rotation,
                ..default()
            },
        ))
        .id()
}

//helper function to compute arrow transform
pub fn link_transform_helper(
    link: &GraphLink,
    transforms: &Query<&mut Transform>,
    meshes: &mut ResMut<Assets<Mesh>>,
    from_radius: f32,
    to_radius: f32,
) -> Option<(Transform, Mesh2d)> {
    let to = link.to?;

    let from_pos = transforms.get(link.from).ok()?.translation;
    let to_pos = transforms.get(to).ok()?.translation;

    let (translation, rotation, length) = link_geometry(
        from_pos,
        to_pos,
        from_radius,
        to_radius,
    );

    Some((
        (Transform {
            translation,
            rotation,
            scale: Vec3::new(1.0, 1.0, 1.0),
        }),
        (Mesh2d(meshes.add(arrow_mesh(length)))),
    ))
}

pub fn endpoint_radius(
    entity: Entity,
    random_nodes: &Query<&RandomNode>,
    compute_nodes: &Query<&ComputeNode>,
    scalar_nodes: &Query<&ScalarNode>,
) -> f32 {
    if scalar_nodes.contains(entity) {
        SCALAR_NODE_RAD
    } else if compute_nodes.contains(entity) {
        COMPUTE_NODE_RAD
    } else if random_nodes.contains(entity) {
        RANDOM_NODE_RAD
    } else {
        RANDOM_NODE_RAD
    }
}

fn link_geometry(
    from_pos: Vec3,
    to_pos: Vec3,
    from_radius: f32,
    to_radius: f32,
) -> (Vec3, Quat, f32) {
    let delta = to_pos - from_pos;
    let distance = delta.length();
    let direction = delta.try_normalize().unwrap_or(Vec3::X);
    let start = from_pos + direction * from_radius.min(distance);
    let end = to_pos - direction * to_radius.min(distance);
    let length = (distance - from_radius - to_radius).max(0.0);
    (
        start.lerp(end, 0.5),
        Quat::from_rotation_z(delta.y.atan2(delta.x)),
        length,
    )
}
