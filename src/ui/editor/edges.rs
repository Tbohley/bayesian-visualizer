//! Edge inspection, selection styling, and the in-progress edge preview.
use super::*;

pub fn on_edge_click(
    mut event: On<Pointer<Click>>,
    editor: Res<Editor>,
    keys: Res<ButtonInput<KeyCode>>,
    links: Query<(), With<GraphLink>>,
    selected: Query<Entity, With<Selected>>,
    indicators: Query<Entity, With<SelectedIndicator>>,
    mut commands: Commands,
) {
    if !links.contains(event.event_target())
        || editor.tool != Tool::Select
        || keys.pressed(KeyCode::Space)
        || editor.dragged
    {
        return;
    }
    event.propagate(false);
    for e in &selected {
        commands.entity(e).remove::<Selected>();
    }
    for e in &indicators {
        commands.entity(e).despawn();
    }
    commands.entity(event.event_target()).insert(Selected);
    commands.trigger(crate::sidebar::ReloadSidebar);
    commands.trigger(crate::data_vis::CloseHistogramPanel);
}

pub(super) fn highlight_edges(
    links: Query<(&MeshMaterial2d<ColorMaterial>, Has<Selected>), With<GraphLink>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (handle, selected) in &links {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            let color = if selected {
                Color::srgb(0.20, 0.36, 0.83)
            } else {
                ARROW_COLOR
            };
            if material.color != color {
                material.color = color;
            }
        }
    }
}

pub(super) fn update_edge_preview(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    links: Query<(Entity, &GraphLink), With<UnfinishedLink>>,
    nodes: Query<&Transform, With<GraphNode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut last: Local<Option<Vec2>>,
) {
    if links.is_empty() {
        *last = None;
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if *last == Some(cursor) {
        return;
    }
    *last = Some(cursor);
    let (camera, t) = *camera;
    let Ok(target) = camera.viewport_to_world_2d(t, cursor) else {
        return;
    };
    for (e, link) in &links {
        let Ok(start) = nodes.get(link.from) else {
            continue;
        };
        let (translation, rotation, length) = link_geometry(
            start.translation,
            target.extend(1.),
            EndpointShape::circle(20.),
            EndpointShape::circle(0.),
        );
        commands.entity(e).insert((
            Mesh2d(meshes.add(arrow_mesh(length))),
            MeshMaterial2d(materials.add(Color::srgb(0.4, 0.5, 0.7))),
            Transform {
                translation,
                rotation,
                ..default()
            },
            Pickable::IGNORE,
        ));
    }
}
