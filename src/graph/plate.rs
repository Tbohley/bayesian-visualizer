use super::Selected;
use crate::nodes::{GraphNode, SelectedIndicator};
use crate::sidebar::ReloadSidebar;
use bevy::prelude::*;
use crate::constants::*;

#[derive(Clone, Copy, Debug)]
pub struct PlateBounds {
    pub min: Vec2,
    pub max: Vec2,
}

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
}

#[derive(Component, Debug)]
pub struct Plate {
    pub origin: Vec2,
    pub bounds: PlateBounds,
}

#[derive(Component)]
pub struct PlateDraft;

#[derive(Component, Clone, Copy)]
pub(crate) enum PlateBorder {
    Top,
    Right,
    Bottom,
    Left,
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
            },
            GraphNode(id),
            PlateDraft,
            Pickable::IGNORE,
            Transform::from_xyz(start.x, start.y, PLATE_Z),
        ))
        .observe(on_plate_click)
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
    plate: Single<(Entity, &Plate), With<PlateDraft>>,
) {
    let (entity, plate) = plate.into_inner();
    if plate.bounds.is_substantial() {
        commands.entity(entity).remove::<PlateDraft>();
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
}
