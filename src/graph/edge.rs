use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use crate::constants::*;
use crate::graph::*;

//update arrow transforms connecting to dragged node
pub fn on_node_drag (
    event: On<Pointer<Drag>>,
    mut transforms: Query<&mut Transform>,
    mut mesh_query: Query<&mut Mesh2d>,
    mut graph_links: Query<(Entity, &mut GraphLink), Without<UnfinishedLink>>,
    mut meshes: ResMut<Assets<Mesh>>
) {
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
        if event.event_target() == link_component.from || event.event_target() == link_component.to.unwrap() {
            let (new_transform, new_mesh) = link_transform_helper(&link_component, &transforms, &mut meshes).unwrap();
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
        [-hw, hs, 0.0],   // 0: shaft top-left
        [-hw, -hs, 0.0],  // 1: shaft bottom-left
        [tx, hs, 0.0],    // 2: shaft top-right
        [tx, -hs, 0.0],   // 3: shaft bottom-right
        [tx, hh, 0.0],    // 4: head top
        [tx, -hh, 0.0],   // 5: head bottom
        [hw, 0.0, 0.0],   // 6: tip
    ];

    let indices = vec![0u32, 1, 2, 2, 1, 3, 4, 5, 6];

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
    .with_inserted_indices(Indices::U32(indices))
}


//helper function to compute arrow transform
pub fn link_transform_helper(
    link: &GraphLink,
    transforms: &Query<&mut Transform>,
    meshes: &mut ResMut<Assets<Mesh>>

) -> Option<(Transform, Mesh2d)> {
    let to = link.to?;

    let from_pos = transforms.get(link.from).ok()?.translation;
    let to_pos = transforms.get(to).ok()?.translation;

    let delta = to_pos - from_pos;
    let angle = delta.y.atan2(delta.x);
    let midpoint = from_pos.lerp(to_pos, 0.5);
    let length = from_pos.distance(to_pos) - (2.0 * NODE_RAD);

    Some(((Transform {
            translation: midpoint,
            rotation: Quat::from_rotation_z(angle),
            scale: Vec3::new(1.0, 1.0, 1.0)}),
        (Mesh2d(meshes.add(arrow_mesh(length))))))
}
