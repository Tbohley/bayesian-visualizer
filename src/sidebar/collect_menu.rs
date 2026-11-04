//! Ordered scalar inputs for the Collect vector operation.
use super::*;

#[derive(Clone, Copy, Debug)]
pub enum CollectInputEdit {
    Add,
    Remove(usize),
    Move { from: usize, to: usize },
}

#[derive(Event)]
pub struct EditCollectInputs {
    pub node: Entity,
    pub edit: CollectInputEdit,
}

#[derive(Component)]
pub(crate) struct CollectInputRow {
    node: Entity,
    index: usize,
}

#[derive(Component)]
struct CollectInputHandle {
    node: Entity,
    index: usize,
}

pub(super) fn build_input_actions(
    commands: &mut Commands,
    row: Entity,
    node: Entity,
    index: usize,
    count: usize,
) {
    commands.entity(row).insert((
        CollectInputRow { node, index },
        Name::new(format!("collect_element_{index}")),
    ));
    let handle = commands
        .spawn((
            Name::new(format!("collect_drag_{index}")),
            CollectInputHandle { node, index },
            Node {
                position_type: PositionType::Absolute,
                right: px(34),
                top: px(5),
                width: px(26),
                height: px(26),
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(px(5)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.65, 0.65, 0.65)),
        ))
        .observe(|mut e: On<Pointer<Press>>| e.propagate(false))
        .observe(start_drag)
        .observe(move_drag)
        .observe(end_drag)
        .id();
    commands.entity(handle).with_children(|grip| {
        for _ in 0..3 {
            grip.spawn((
                Node {
                    column_gap: px(3),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|line| {
                for _ in 0..2 {
                    line.spawn((
                        Node {
                            width: px(3),
                            height: px(3),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BackgroundColor(Color::BLACK),
                        Pickable::IGNORE,
                    ));
                }
            });
        }
    });
    commands.entity(row).add_child(handle);
    if count > 1 {
        let remove = commands
            .spawn((
                Button,
                Name::new(format!("collect_Remove_{index}")),
                Node {
                    position_type: PositionType::Absolute,
                    right: px(5),
                    top: px(5),
                    width: px(24),
                    height: px(26),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(px(5)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.35, 0.35, 0.35)),
                children![(
                    Text::new("×"),
                    TextFont {
                        font_size: FontSize::Px(18.),
                        ..text_font()
                    },
                    TextColor(Color::WHITE),
                    Pickable::IGNORE
                )],
            ))
            .observe(|mut e: On<Pointer<Press>>| e.propagate(false))
            .observe(move |mut e: On<Pointer<Click>>, mut commands: Commands| {
                e.propagate(false);
                if e.button == bevy::picking::pointer::PointerButton::Primary {
                    commands.trigger(EditCollectInputs {
                        node,
                        edit: CollectInputEdit::Remove(index),
                    });
                }
            })
            .id();
        commands.entity(row).add_child(remove);
    }
}

/// Resting layout stays intact during a gesture. Transforms preview the new
/// order, then a single model edit on release preserves one-step undo.
#[derive(Resource)]
pub(crate) struct CollectDrag {
    node: Entity,
    handle: Entity,
    from: usize,
    to: usize,
    offset: Vec2,
    ui_per_screen: f32,
    slots: Vec<DragSlot>,
}

struct DragSlot {
    entity: Entity,
    center: Vec2,
    size: Vec2,
}

fn start_drag(
    mut event: On<Pointer<DragStart>>,
    handles: Query<&CollectInputHandle>,
    rows: Query<(Entity, &CollectInputRow, &ComputedNode, &UiGlobalTransform)>,
    window: Single<&Window>,
    mut commands: Commands,
) {
    event.propagate(false);
    if event.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    let Ok(handle) = handles.get(event.event_target()) else {
        return;
    };
    let mut slots = rows
        .iter()
        .filter(|(_, row, _, _)| row.node == handle.node)
        .collect::<Vec<_>>();
    slots.sort_by_key(|(_, row, _, _)| row.index);
    if slots.len() <= handle.index {
        return;
    }
    let ui_per_screen = slots[handle.index].2.inverse_scale_factor() * window.scale_factor();
    let slots = slots
        .into_iter()
        .map(|(entity, _, computed, transform)| DragSlot {
            entity,
            center: transform.translation * computed.inverse_scale_factor(),
            size: computed.size() * computed.inverse_scale_factor(),
        })
        .collect::<Vec<_>>();
    commands
        .entity(slots[handle.index].entity)
        .insert(GlobalZIndex(1200));
    commands.trigger(CloseContextMenus);
    commands.insert_resource(CollectDrag {
        node: handle.node,
        handle: event.event_target(),
        from: handle.index,
        to: handle.index,
        offset: Vec2::ZERO,
        ui_per_screen,
        slots,
    });
}

impl CollectDrag {
    fn update(&mut self, distance: Vec2) {
        self.offset = distance * self.ui_per_screen;
        let center_y = self.slots[self.from].center.y + self.offset.y;
        // Crossing a resting center opens a gap at that index. Using resting
        // centers avoids jitter as the neighboring cards animate out of the way.
        self.to = self
            .slots
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != self.from)
            .filter(|(_, slot)| {
                center_y > slot.center.y || (self.offset.y > 0. && center_y == slot.center.y)
            })
            .count();
    }

    fn offsets(&self) -> Vec<(Entity, f32)> {
        let mut order = (0..self.slots.len()).collect::<Vec<_>>();
        order.remove(self.from);
        order.insert(self.to, self.from);
        let mut top = self.slots[0].center.y - self.slots[0].size.y * 0.5;
        order
            .into_iter()
            .map(|index| {
                let slot = &self.slots[index];
                let offset = top + slot.size.y * 0.5 - slot.center.y;
                top += slot.size.y + 8.;
                (slot.entity, offset)
            })
            .collect()
    }
}

fn move_drag(mut event: On<Pointer<Drag>>, drag: Option<ResMut<CollectDrag>>) {
    event.propagate(false);
    if let Some(mut drag) = drag {
        if drag.handle == event.event_target() {
            drag.update(event.distance);
        }
    }
}

fn end_drag(
    mut event: On<Pointer<DragEnd>>,
    drag: Option<ResMut<CollectDrag>>,
    mut commands: Commands,
) {
    event.propagate(false);
    let Some(mut drag) = drag else {
        return;
    };
    if drag.handle != event.event_target() {
        return;
    }
    drag.update(event.distance);
    commands
        .entity(drag.slots[drag.from].entity)
        .remove::<GlobalZIndex>();
    commands.remove_resource::<CollectDrag>();
    // Releasing well outside the list cancels the gesture.
    let slot = &drag.slots[drag.from];
    let center = slot.center + drag.offset;
    let first = &drag.slots[0];
    let last = drag.slots.last().unwrap();
    if drag.offset.x.abs() <= slot.size.x * 0.5
        && center.y >= first.center.y - first.size.y
        && center.y <= last.center.y + last.size.y
    {
        commands.trigger(EditCollectInputs {
            node: drag.node,
            edit: CollectInputEdit::Move {
                from: drag.from,
                to: drag.to,
            },
        });
    }
}

pub(crate) fn animate_drag(
    drag: Option<Res<CollectDrag>>,
    time: Res<Time>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut rows: Query<(Entity, &mut UiTransform, Option<&GlobalZIndex>), With<CollectInputRow>>,
    mut commands: Commands,
) {
    let had_drag = drag.is_some();
    let cancelled = keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape));
    let drag = drag.filter(|drag| !cancelled && rows.contains(drag.slots[drag.from].entity));
    if had_drag && drag.is_none() {
        commands.remove_resource::<CollectDrag>();
    }
    let offsets = drag.as_ref().map(|drag| drag.offsets()).unwrap_or_default();
    let blend = 1. - (-22. * time.delta_secs()).exp();
    for (entity, mut transform, z_index) in &mut rows {
        if let Some(drag) = drag
            .as_ref()
            .filter(|drag| entity == drag.slots[drag.from].entity)
        {
            let translation = Val2::px(drag.offset.x, drag.offset.y);
            if transform.translation != translation {
                transform.translation = translation;
            }
            if transform.scale != Vec2::splat(1.025) {
                transform.scale = Vec2::splat(1.025);
            }
        } else {
            if z_index.is_some() {
                commands.entity(entity).remove::<GlobalZIndex>();
            }
            let target = offsets
                .iter()
                .find(|(e, _)| *e == entity)
                .map(|(_, y)| *y)
                .unwrap_or(0.);
            let current = match transform.translation.y {
                Val::Px(y) => y,
                _ => 0.,
            };
            let next = current + (target - current) * blend;
            let translation = Val2::px(
                0.,
                if (next - target).abs() < 0.1 {
                    target
                } else {
                    next
                },
            );
            if transform.translation != translation {
                transform.translation = translation;
            }
            if transform.scale != Vec2::ONE {
                transform.scale = Vec2::ONE;
            }
        }
    }
}

pub(super) fn build_add_input(commands: &mut Commands, sidebar: Entity, node: Entity) {
    let button = commands
        .spawn((
            Name::new("add_collect_input"),
            Button,
            Node {
                align_self: AlignSelf::FlexStart,
                min_height: px(28),
                padding: UiRect::axes(px(10), px(5)),
                margin: px(4).bottom(),
                flex_shrink: 0.,
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(Color::srgb(0.20, 0.36, 0.32)),
            children![(
                Text::new("+ Add element"),
                TextFont {
                    font_size: FontSize::Px(13.),
                    ..text_font()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE
            )],
        ))
        .observe(|mut e: On<Pointer<Press>>| e.propagate(false))
        .observe(
            move |mut event: On<Pointer<Click>>, mut commands: Commands| {
                event.propagate(false);
                if event.button == bevy::picking::pointer::PointerButton::Primary {
                    commands.trigger(EditCollectInputs {
                        node,
                        edit: CollectInputEdit::Add,
                    });
                }
            },
        )
        .id();
    commands.entity(sidebar).add_child(button);
}

pub fn edit_collect_inputs(
    event: On<EditCollectInputs>,
    mut nodes: Query<&mut ComputeNode>,
    links: Query<(Entity, &GraphLink), Without<UnfinishedLink>>,
    mut commands: Commands,
    inference: Option<Res<crate::bevy_to_fugue::InferenceResultResource>>,
) {
    let Ok(mut node) = nodes.get_mut(event.node) else {
        return;
    };
    if node.operation != Operation::Collect {
        return;
    }
    let count = node.params.len();
    let valid = match event.edit {
        CollectInputEdit::Add => true,
        CollectInputEdit::Remove(index) => count > 1 && index < count,
        CollectInputEdit::Move { from, to } => from < count && to < count && from != to,
    };
    if !valid
        || global::graph_change_requires_confirmation(&mut commands, inference.as_deref(), None)
    {
        return;
    }
    match event.edit {
        CollectInputEdit::Add => node.params.push(ParamValue("element", None)),
        CollectInputEdit::Move { from, to } => {
            let param = node.params.remove(from);
            node.params.insert(to, param);
        }
        CollectInputEdit::Remove(index) => {
            if let Some(source) = node.params.remove(index).1 {
                if !node.params.iter().any(|param| param.1 == Some(source)) {
                    for (entity, link) in &links {
                        if link.from == source && link.to == Some(event.node) {
                            commands.entity(entity).despawn();
                        }
                    }
                }
            }
        }
    }
    commands.trigger(ReloadSidebar);
}

#[cfg(test)]
#[path = "tests/collect_menu.rs"]
mod tests;
