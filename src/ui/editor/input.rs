//! Keyboard shortcuts, gesture state, and cursor feedback.
use super::*;

pub(super) fn reset_gesture(mouse: Res<ButtonInput<MouseButton>>, mut editor: ResMut<Editor>) {
    if mouse.just_pressed(MouseButton::Left) {
        editor.dragged = false;
        editor.panning = false;
    }
}

pub(super) fn editor_keys(
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    editable: Query<(), With<EditableText>>,
    mut editor: ResMut<Editor>,
    mut commands: Commands,
    unfinished: Query<Entity, Or<(With<UnfinishedLink>, With<PlateDraft>, With<NodeDragGhost>)>>,
    mut drag: ResMut<NodeDrag>,
    mut history: ResMut<crate::ui::history::HistoryRequest>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        editor.tool = Tool::Select;
        drag.active = None;
        editor.panning = false;
        editor.dragged = false;
        for e in &unfinished {
            commands.entity(e).despawn();
        }
        commands.trigger(CloseContextMenus);
    }
    if focus.get().is_some_and(|e| editable.contains(e)) {
        return;
    }
    if keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]) {
        if keys.just_pressed(KeyCode::KeyZ) {
            history.0 = Some(keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]));
        } else if keys.just_pressed(KeyCode::KeyY) {
            history.0 = Some(true);
        }
    }
}

pub(super) fn update_editor_cursor(
    mut commands: Commands,
    editor: Res<Editor>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<(Entity, &Window)>,
    ui: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Option<&Pickable>,
    )>,
    unfinished: Query<(), With<UnfinishedLink>>,
    cursors: Res<LinkCursors>,
) {
    let (entity, w) = *window;
    let over_panel = w
        .cursor_position()
        .is_some_and(|p| over_ui(p, &ui, w.scale_factor()));
    if !over_panel
        && !editor.panning
        && !keys.pressed(KeyCode::Space)
        && (editor.tool == Tool::Edge || !unfinished.is_empty())
    {
        let finishing = !unfinished.is_empty();
        commands
            .entity(entity)
            .insert(CursorIcon::Custom(CustomCursor::Image(CustomCursorImage {
                handle: if finishing {
                    cursors.finish.clone()
                } else {
                    cursors.start.clone()
                },
                texture_atlas: None,
                flip_x: false,
                flip_y: false,
                rect: None,
                hotspot: if finishing { (16, 11) } else { (16, 1) },
            })));
        return;
    }
    let icon = if w
        .cursor_position()
        .is_some_and(|p| over_ui(p, &ui, w.scale_factor()))
    {
        SystemCursorIcon::Default
    } else if editor.panning {
        SystemCursorIcon::Grabbing
    } else if keys.pressed(KeyCode::Space) {
        SystemCursorIcon::Grab
    } else if editor.tool != Tool::Select || !unfinished.is_empty() {
        SystemCursorIcon::Crosshair
    } else {
        SystemCursorIcon::Default
    };
    commands.entity(entity).insert(CursorIcon::System(icon));
}
