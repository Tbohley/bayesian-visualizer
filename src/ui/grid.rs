//! A viewport-sized dot mesh, aligned to world coordinates for pan/zoom reference.
use super::editor::camera_scale;
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

#[derive(Component)]
pub(super) struct CanvasGrid;

pub(super) fn spawn_grid(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        CanvasGrid,
        Name::new("Canvas dot grid"),
        Mesh2d(meshes.add(Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        ))),
        MeshMaterial2d(materials.add(Color::srgb(0.84, 0.85, 0.87))),
        Transform::from_xyz(0., 0., -99.),
        NoFrustumCulling,
        Pickable::IGNORE,
    ));
}

pub(super) fn update_grid(
    window: Single<&Window>,
    camera: Single<(&Transform, &Projection), With<Camera2d>>,
    grid: Single<&Mesh2d, With<CanvasGrid>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut previous: Local<Option<(IVec2, IVec2, f32, f32)>>,
) {
    let (transform, projection) = *camera;
    let scale = camera_scale(projection);
    // Keep a 40-unit lattice, showing every second/fourth dot when zoomed far
    // out so the background stays light and mesh size stays bounded.
    let spacing = 40. * 2.0_f32.powf((20. * scale / 40.).log2().ceil().max(0.));
    let center = transform.translation.truncate();
    let half_size = Vec2::new(window.width(), window.height()) * scale * 0.5;
    let first = ((center - half_size) / spacing).floor().as_ivec2() - IVec2::ONE;
    let last = ((center + half_size) / spacing).ceil().as_ivec2() + IVec2::ONE;
    let key = (first, last, spacing, scale);
    if *previous == Some(key) {
        return;
    }
    let Some(mut mesh) = meshes.get_mut(&grid.0) else {
        return;
    };
    let dot_count = ((last.x - first.x + 1) * (last.y - first.y + 1)) as usize;
    let mut positions = Vec::with_capacity(dot_count * 9);
    let mut indices = Vec::with_capacity(dot_count * 24);
    // Dots stay about one screen pixel in radius while their spacing zooms.
    let radius = scale;
    for y in first.y..=last.y {
        for x in first.x..=last.x {
            let center = Vec2::new(x as f32, y as f32) * spacing;
            let base = positions.len() as u32;
            positions.push([center.x, center.y, 0.]);
            for segment in 0..8 {
                let angle = segment as f32 * std::f32::consts::TAU / 8.;
                let point = center + Vec2::new(angle.cos(), angle.sin()) * radius;
                positions.push([point.x, point.y, 0.]);
                indices.extend_from_slice(&[
                    base,
                    base + 1 + segment,
                    base + 1 + (segment + 1) % 8,
                ]);
            }
        }
    }
    *mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices));
    *previous = Some(key);
}
