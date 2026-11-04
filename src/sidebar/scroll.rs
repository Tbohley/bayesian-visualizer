//! Scroll containers used by long parameter lists and operation menus.
use bevy::{input::mouse::MouseScrollUnit, picking::events::Scroll, prelude::*};

pub(crate) fn scroll_panel(mut event: On<Pointer<Scroll>>, mut panels: Query<&mut ScrollPosition>) {
    let Ok(mut position) = panels.get_mut(event.event_target()) else {
        return;
    };
    let step = if event.unit == MouseScrollUnit::Line {
        32.
    } else {
        1.
    };
    position.0.y = (position.0.y - event.y * step).max(0.);
    event.propagate(false);
}
