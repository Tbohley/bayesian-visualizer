use std::collections::HashMap;

use super::{Plate, PlateBorder, PlateBounds, PlateDraft, Selected};
use crate::constants::*;
use crate::nodes::{GraphNode, NodeLabel, RandomNode, ScalarNode, SelectedIndicator, replace_node_label};
use crate::sidebar::ReloadSidebar;
use bevy::prelude::*;

impl PlateBounds {
    pub fn from_points(a: Vec2, b: Vec2) -> Self {
        Self {
            min: a.min(b),
            max: a.max(b),
        }
    }

    pub fn center(self) -> Vec2 {
        (self.min + self.max) / 2.0
    }

    pub fn size(self) -> Vec2 {
        self.max - self.min
    }

    pub fn contains_point(self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    pub fn contains_bounds(self, other: Self) -> bool {
        self.contains_point(other.min) && self.contains_point(other.max)
    }

    pub fn is_substantial(self) -> bool {
        let size = self.size();
        size.x >= MIN_PLATE_EXTENT && size.y >= MIN_PLATE_EXTENT
    }

    fn translate(&mut self, delta: Vec2) {
        self.min += delta;
        self.max += delta;
    }
}


pub fn on_plate_drag_start(
    event: On<Pointer<DragStart>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    graph_nodes: Query<&GraphNode>,
) {
    let Some(position) = event.hit.position else {
        return;
    };
    let start = position.truncate();

    let mut id = 1;
    while graph_nodes.iter().any(|node| node.0 == id) {
        id += 1;
    }

    let border_mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let border_material = materials.add(PLATE_COLOR);
    let plate = commands
        .spawn((
            Plate {
                origin: start,
                bounds: PlateBounds::from_points(start, start),
                data: super::Dataset { name: "No dataset".to_string(), n: 10, data: HashMap::new() },
                mapping: HashMap::new()
            },
            GraphNode(id),
            PlateDraft,
            Pickable::IGNORE,
            Visibility::default(),
            Transform::from_xyz(start.x, start.y, PLATE_Z),
        ))
        .observe(on_plate_click)
        .observe(on_completed_plate_drag)
        .observe(on_completed_plate_drag_end)
        .id();

    commands.entity(plate).with_children(|parent| {
        for edge in [
            PlateBorder::Top,
            PlateBorder::Right,
            PlateBorder::Bottom,
            PlateBorder::Left,
        ] {
            parent.spawn((
                edge,
                Pickable {
                    should_block_lower: true,
                    is_hoverable: true,
                },
                Mesh2d(border_mesh.clone()),
                MeshMaterial2d(border_material.clone()),
                Transform::default(),
            ));
        }
    });
}

fn on_completed_plate_drag(
    event: On<Pointer<Drag>>,
    mut plates: Query<(&mut Plate, &mut Transform), Without<PlateDraft>>,
) {
    let Ok((mut plate, mut transform)) = plates.get_mut(event.event_target()) else {
        return;
    };
    let delta = Vec2::new(event.delta.x, -event.delta.y);

    plate.origin += delta;
    plate.bounds.translate(delta);
    transform.translation.x += delta.x;
    transform.translation.y += delta.y;
}

fn on_completed_plate_drag_end(
    event: On<Pointer<DragEnd>>,
    mut commands: Commands,
    mut plates: Query<&mut Plate, Without<PlateDraft>>,
    nodes: Query<(Entity, &Transform), Or<(With<RandomNode>, With<ScalarNode>)>>,
) {
    let Ok(mut plate) = plates.get_mut(event.event_target()) else {
        return;
    };

    let bounds = plate.bounds;
    plate.mapping.retain(|entity, _| {
        nodes
            .get(*entity)
            .is_ok_and(|(_, transform)| bounds.contains_point(transform.translation.truncate()))
    });
    for (entity, transform) in &nodes {
        if bounds.contains_point(transform.translation.truncate()) {
            plate.mapping
                .entry(entity)
                .or_insert_with(|| "unobserved".to_string());
        }
    }
    commands.trigger(ReloadSidebar);
}

pub fn on_plate_drag(
    event: On<Pointer<Drag>>,
    plate: Single<(Entity, &mut Plate, &mut Transform), With<PlateDraft>>,
    mut borders: Query<(&PlateBorder, &ChildOf, &mut Transform), Without<Plate>>,
) {
    let (plate_entity, mut plate, mut transform) = plate.into_inner();
    let start = plate.origin;
    let current = start + Vec2::new(event.distance.x, -event.distance.y);
    let bounds = PlateBounds::from_points(start, current);
    let center = bounds.center();
    let size = bounds.size();

    plate.bounds = bounds;
    transform.translation.x = center.x;
    transform.translation.y = center.y;

    for (edge, child_of, mut border_transform) in &mut borders {
        if child_of.parent() != plate_entity {
            continue;
        }

        let half_width = size.x / 2.0;
        let half_height = size.y / 2.0;
        match edge {
            PlateBorder::Top => {
                border_transform.translation = Vec3::new(0.0, half_height, 0.0);
                border_transform.scale =
                    Vec3::new(size.x + PLATE_BORDER_THICKNESS, PLATE_BORDER_THICKNESS, 1.0);
            }
            PlateBorder::Right => {
                border_transform.translation = Vec3::new(half_width, 0.0, 0.0);
                border_transform.scale = Vec3::new(PLATE_BORDER_THICKNESS, size.y, 1.0);
            }
            PlateBorder::Bottom => {
                border_transform.translation = Vec3::new(0.0, -half_height, 0.0);
                border_transform.scale =
                    Vec3::new(size.x + PLATE_BORDER_THICKNESS, PLATE_BORDER_THICKNESS, 1.0);
            }
            PlateBorder::Left => {
                border_transform.translation = Vec3::new(-half_width, 0.0, 0.0);
                border_transform.scale = Vec3::new(PLATE_BORDER_THICKNESS, size.y, 1.0);
            }
        }
    }
}

pub fn on_plate_drag_end(
    _event: On<Pointer<DragEnd>>,
    mut commands: Commands,
    plate: Single<(Entity, &mut Plate), With<PlateDraft>>,
    labels: Query<(Entity, &NodeLabel, &ChildOf)>,
    nodes: Query<(Entity, &Transform), Or<(With<RandomNode>, With<ScalarNode>)>>,
) {
    let (entity, mut plate) = plate.into_inner();
    if plate.bounds.is_substantial() {
        for (node_entity, transform) in &nodes {
            if plate.bounds.contains_point(transform.translation.truncate()) {
                plate.mapping.insert(node_entity, "unobserved".to_string());
            }
        }

        commands.entity(entity).remove::<PlateDraft>();
        replace_node_label(&mut commands,entity,format!("N"), &labels, Some(&plate))
    } else {
        commands.entity(entity).despawn();
    }
}

fn on_plate_click(
    mut event: On<Pointer<Click>>,
    mut commands: Commands,
    selected: Option<Single<Entity, With<Selected>>>,
    selection_indicators: Query<(Entity, &ChildOf), With<SelectedIndicator>>,
) {
    event.propagate(false);
    if event.duration.as_millis() >= 200 || event.count != 1 {
        return;
    }

    if let Some(selected) = selected {
        let selected = *selected;
        commands.entity(selected).remove::<Selected>();
        for (indicator, child_of) in &selection_indicators {
            if child_of.parent() == selected {
                commands.entity(indicator).despawn();
            }
        }
    }

    commands.entity(event.event_target()).insert(Selected);
    commands.trigger(ReloadSidebar);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_pointer_jitter_as_a_plate() {
        let jitter = PlateBounds::from_points(Vec2::ZERO, Vec2::new(MIN_PLATE_EXTENT - 1.0, 20.0));
        let plate = PlateBounds::from_points(Vec2::ZERO, Vec2::splat(MIN_PLATE_EXTENT));

        assert!(!jitter.is_substantial());
        assert!(plate.is_substantial());
    }

    #[test]
    fn translating_a_plate_preserves_its_size() {
        let mut bounds = PlateBounds::from_points(Vec2::new(10.0, 20.0), Vec2::new(50.0, 80.0));
        let size = bounds.size();

        bounds.translate(Vec2::new(-15.0, 25.0));

        assert_eq!(bounds.size(), size);
        assert_eq!(bounds.min, Vec2::new(-5.0, 45.0));
        assert_eq!(bounds.max, Vec2::new(35.0, 105.0));
    }
}
