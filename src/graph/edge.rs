use crate::constants::*;
use crate::graph::*;
use crate::nodes::{
    ComputeNode, GraphNode, RandomNode, ScalarNode, random_node_label,
    random_node_straight_length,
};
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
    node_ids: Query<&GraphNode>,
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
            let from_shape = endpoint_shape(
                link_component.from,
                &random_nodes,
                &compute_nodes,
                &scalar_nodes,
                &node_ids,
            );
            let to_shape = endpoint_shape(
                link_component.to.unwrap(),
                &random_nodes,
                &compute_nodes,
                &scalar_nodes,
                &node_ids,
            );
            let (new_transform, new_mesh) = link_transform_helper(
                &link_component,
                &transforms,
                &mut meshes,
                from_shape,
                to_shape,
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

/// Reposition links after a rename changes a random node's capsule width.
pub fn refresh_links_for_resized_random_nodes(
    changed_random_nodes: Query<Entity, Changed<RandomNode>>,
    mut graph_links: Query<
        (&GraphLink, &mut Transform, &mut Mesh2d),
        (Without<UnfinishedLink>, Without<GraphNode>),
    >,
    node_transforms: Query<&Transform, With<GraphNode>>,
    random_nodes: Query<&RandomNode>,
    compute_nodes: Query<&ComputeNode>,
    scalar_nodes: Query<&ScalarNode>,
    node_ids: Query<&GraphNode>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if changed_random_nodes.is_empty() {
        return;
    }

    for (link, mut transform, mut mesh) in &mut graph_links {
        let Some(to) = link.to else {
            continue;
        };
        if !changed_random_nodes.contains(link.from) && !changed_random_nodes.contains(to) {
            continue;
        }
        let (Ok(from_transform), Ok(to_transform)) =
            (node_transforms.get(link.from), node_transforms.get(to))
        else {
            continue;
        };
        let from_shape = endpoint_shape(
            link.from,
            &random_nodes,
            &compute_nodes,
            &scalar_nodes,
            &node_ids,
        );
        let to_shape = endpoint_shape(
            to,
            &random_nodes,
            &compute_nodes,
            &scalar_nodes,
            &node_ids,
        );
        let (translation, rotation, length) = link_geometry(
            from_transform.translation,
            to_transform.translation,
            from_shape,
            to_shape,
        );
        transform.translation = translation;
        transform.rotation = rotation;
        mesh.0 = meshes.add(arrow_mesh(length));
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
    from_shape: EndpointShape,
    to_shape: EndpointShape,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) -> Entity {
    spawn_link_visual(
        commands,
        GraphLink { from, to: Some(to) },
        from_pos,
        to_pos,
        from_shape,
        to_shape,
        meshes,
        materials,
    )
}

pub fn spawn_link_visual<B: Bundle>(
    commands: &mut Commands,
    marker: B,
    from_pos: Vec3,
    to_pos: Vec3,
    from_shape: EndpointShape,
    to_shape: EndpointShape,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) -> Entity {
    let (translation, rotation, length) = link_geometry(
        from_pos,
        to_pos,
        from_shape,
        to_shape,
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
    from_shape: EndpointShape,
    to_shape: EndpointShape,
) -> Option<(Transform, Mesh2d)> {
    let to = link.to?;

    let from_pos = transforms.get(link.from).ok()?.translation;
    let to_pos = transforms.get(to).ok()?.translation;

    let (translation, rotation, length) = link_geometry(
        from_pos,
        to_pos,
        from_shape,
        to_shape,
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

#[derive(Clone, Copy, Debug)]
pub struct EndpointShape {
    radius: f32,
    straight_length: f32,
}

impl EndpointShape {
    pub const fn circle(radius: f32) -> Self {
        Self {
            radius,
            straight_length: 0.0,
        }
    }

    pub const fn horizontal_capsule(radius: f32, straight_length: f32) -> Self {
        Self {
            radius,
            straight_length,
        }
    }

    fn boundary_distance(self, direction: Vec2) -> f32 {
        let direction = direction.normalize_or_zero();
        let half_straight = self.straight_length / 2.0;
        if half_straight == 0.0 || direction == Vec2::ZERO {
            return self.radius;
        }

        // A ray exits through a horizontal side when it reaches y = +/-radius
        // before the rounded end cap begins.
        if direction.y != 0.0 {
            let side_distance = self.radius / direction.y.abs();
            if side_distance * direction.x.abs() <= half_straight {
                return side_distance;
            }
        }

        // Otherwise intersect the circle centered at the relevant end of the
        // capsule's straight section.
        let discriminant =
            (self.radius * self.radius - half_straight.powi(2) * direction.y.powi(2))
                .max(0.0);
        half_straight * direction.x.abs() + discriminant.sqrt()
    }
}

pub fn endpoint_shape(
    entity: Entity,
    random_nodes: &Query<&RandomNode>,
    compute_nodes: &Query<&ComputeNode>,
    scalar_nodes: &Query<&ScalarNode>,
    node_ids: &Query<&GraphNode>,
) -> EndpointShape {
    if scalar_nodes.contains(entity) {
        EndpointShape::circle(SCALAR_NODE_RAD)
    } else if compute_nodes.contains(entity) {
        EndpointShape::circle(COMPUTE_NODE_RAD)
    } else if let (Ok(random), Ok(node_id)) = (random_nodes.get(entity), node_ids.get(entity)) {
        let label = random_node_label(random, node_id.0);
        EndpointShape::horizontal_capsule(
            RANDOM_NODE_RAD,
            random_node_straight_length(&label),
        )
    } else {
        EndpointShape::circle(RANDOM_NODE_RAD)
    }
}

fn link_geometry(
    from_pos: Vec3,
    to_pos: Vec3,
    from_shape: EndpointShape,
    to_shape: EndpointShape,
) -> (Vec3, Quat, f32) {
    let delta = to_pos - from_pos;
    let distance = delta.length();
    let direction = delta.try_normalize().unwrap_or(Vec3::X);
    let from_offset = from_shape.boundary_distance(direction.xy());
    let to_offset = to_shape.boundary_distance(direction.xy());
    let start = from_pos + direction * from_offset.min(distance);
    let end = to_pos - direction * to_offset.min(distance);
    let length = (distance - from_offset - to_offset).max(0.0);
    (
        start.lerp(end, 0.5),
        Quat::from_rotation_z(delta.y.atan2(delta.x)),
        length,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capsule_boundary_distance_matches_sides_and_end_caps() {
        let capsule = EndpointShape::horizontal_capsule(20.0, 36.0);

        assert!((capsule.boundary_distance(Vec2::Y) - 20.0).abs() < 0.001);
        assert!((capsule.boundary_distance(Vec2::X) - 38.0).abs() < 0.001);

        let diagonal = Vec2::new(1.0, 1.0).normalize();
        let point = diagonal * capsule.boundary_distance(diagonal);
        let cap_center = Vec2::new(18.0, 0.0);
        assert!((point.distance(cap_center) - 20.0).abs() < 0.001);
    }
}
