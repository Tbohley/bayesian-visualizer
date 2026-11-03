//! Canvas camera navigation, hit testing, and fitting the model.
use super::*;

pub fn camera_scale(projection: &Projection) -> f32 {
    if let Projection::Orthographic(p) = projection {
        p.scale
    } else {
        1.
    }
}

pub(super) fn zoom_at(
    camera: &Camera,
    projection: &mut Projection,
    transform: &mut Transform,
    pointer: Vec2,
    factor: f32,
) {
    let Some(size) = camera.logical_viewport_size() else {
        return;
    };
    apply_zoom(projection, transform, size, pointer, factor);
}

pub(super) fn apply_zoom(
    projection: &mut Projection,
    transform: &mut Transform,
    size: Vec2,
    pointer: Vec2,
    factor: f32,
) {
    let Projection::Orthographic(p) = projection else {
        return;
    };
    let old = p.scale;
    p.scale = (old * factor).clamp(0.1, 8.);
    let offset = Vec2::new(pointer.x - size.x / 2., size.y / 2. - pointer.y);
    transform.translation += (offset * (old - p.scale)).extend(0.);
}

/// Hit-test UI in physical pixels so scroll and global panning cannot leak through panels.
pub fn over_ui(
    pointer: Vec2,
    ui: &Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Option<&Pickable>,
    )>,
    scale: f32,
) -> bool {
    ui.iter().any(|(node, t, v, p)| {
        v.get()
            && !p.is_some_and(|p| !p.should_block_lower)
            && node.contains_point(*t, pointer * scale)
    })
}

pub(super) fn wheel_navigation(
    mut wheel: MessageReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window>,
    ui: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Option<&Pickable>,
    )>,
    mut camera: Single<(&Camera, &mut Projection, &mut Transform), With<Camera2d>>,
) {
    let events: Vec<_> = wheel.read().copied().collect();
    let Some(pointer) = window.cursor_position() else {
        return;
    };
    if over_ui(pointer, &ui, window.scale_factor()) {
        return;
    }
    let (cam, projection, transform) = &mut *camera;
    for e in events {
        let amount = if e.unit == MouseScrollUnit::Line {
            32.
        } else {
            1.
        };
        if keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
            zoom_at(
                cam,
                projection,
                transform,
                pointer,
                (-e.y * amount * 0.004).exp(),
            );
        } else {
            let scale = camera_scale(projection);
            transform.translation += Vec3::new(-e.x * amount * scale, e.y * amount * scale, 0.);
        }
    }
}

pub fn on_canvas_drag_start(
    event: On<Pointer<DragStart>>,
    mut editor: ResMut<Editor>,
    keys: Res<ButtonInput<KeyCode>>,
    canvas: Query<(), With<Canvas>>,
) {
    editor.panning = event.button == bevy::picking::pointer::PointerButton::Primary
        && (keys.pressed(KeyCode::Space)
            || (canvas.contains(event.event_target()) && editor.tool == Tool::Select));
    editor.dragged = false;
}

pub fn on_canvas_drag(
    event: On<Pointer<Drag>>,
    mut editor: ResMut<Editor>,
    mut camera: Single<(&Projection, &mut Transform), With<Camera2d>>,
) {
    editor.dragged |= event.distance.length() > 3.;
    if !editor.panning {
        return;
    }
    let (p, t) = &mut *camera;
    let delta = Vec2::new(-event.delta.x, event.delta.y) * camera_scale(p);
    t.translation += delta.extend(0.);
}

pub fn on_canvas_drag_end(_: On<Pointer<DragEnd>>, mut editor: ResMut<Editor>) {
    editor.panning = false;
}

pub(super) fn follow_canvas(
    window: Single<&Window>,
    camera: Single<(&Transform, &Projection), (With<Camera2d>, Without<Canvas>)>,
    mut canvas: Single<&mut Transform, With<Canvas>>,
) {
    let (t, p) = *camera;
    canvas.translation = Vec3::new(t.translation.x, t.translation.y, -100.);
    let scale = camera_scale(p);
    canvas.scale = Vec3::new(
        window.width() * scale * 3.,
        window.height() * scale * 3.,
        1.,
    );
}

/// Reserve both sidebars and the histogram before they open, so fitting remains stable.
pub(super) fn fit_model(
    projection: &mut OrthographicProjection,
    transform: &mut Transform,
    min: Vec2,
    max: Vec2,
    window: Vec2,
) {
    let padding = 40.;
    let available_min = Vec2::new(SIDEBAR_WIDTH + padding, padding);
    let available_max = Vec2::new(
        window.x - SIDEBAR_WIDTH - padding,
        window.y - crate::data_vis::HISTOGRAM_PANEL_HEIGHT - padding,
    );
    let size = (available_max - available_min).max(Vec2::splat(40.));
    projection.scale = ((max.x - min.x) / size.x)
        .max((max.y - min.y) / size.y)
        .clamp(0.1, 8.);
    let screen_center = available_min + size / 2.;
    let offset = Vec2::new(
        screen_center.x - window.x / 2.,
        window.y / 2. - screen_center.y,
    );
    let center = (min + max) / 2. - offset * projection.scale;
    transform.translation.x = center.x;
    transform.translation.y = center.y;
}
