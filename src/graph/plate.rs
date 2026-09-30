use std::collections::HashMap;

use super::{
    Plate, PlateBorder, PlateBounds, PlateDraft, PlateIndexHandle, PlateIndexLabel, Selected,
};
use crate::bevy_to_fugue::{GraphIRResource, InferenceStatusResource};
use crate::constants::*;
use crate::data_vis::{HistogramSubject, OpenHistogramPanel};
use crate::nodes::{GraphNode, RandomNode, ScalarNode, SelectedIndicator};
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
    reduced_view: Res<super::ReducedView>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if reduced_view.active {
        return;
    }
    let Some(position) = event.hit.position else {
        return;
    };
    let start = position.truncate();

    let plate = commands
        .spawn((
            Plate {
                origin: start,
                bounds: PlateBounds::from_points(start, start),
                data: super::Dataset {
                    name: "No dataset".to_string(),
                    n: 10,
                    data: HashMap::new(),
                },
                mapping: HashMap::new(),
            },
            PlateDraft,
            Pickable::IGNORE,
            Visibility::default(),
            Transform::from_xyz(start.x, start.y, PLATE_Z),
        ))
        .observe(on_plate_click)
        .observe(on_completed_plate_drag)
        .observe(on_completed_plate_drag_end)
        .id();

    add_plate_borders(&mut commands, plate, None, &mut meshes, &mut materials);
}

pub fn spawn_completed_plate(
    commands: &mut Commands,
    node_num: u32,
    plate: Plate,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) -> Entity {
    let bounds = plate.bounds;
    let size = bounds.size();
    let plate_entity = commands
        .spawn((
            GraphNode(node_num),
            plate,
            Pickable::IGNORE,
            Visibility::default(),
            Transform::from_xyz(bounds.center().x, bounds.center().y, PLATE_Z),
        ))
        .observe(on_plate_click)
        .observe(on_completed_plate_drag)
        .observe(on_completed_plate_drag_end)
        .id();

    add_plate_borders(commands, plate_entity, Some(size), meshes, materials);
    spawn_plate_handle(commands, plate_entity, size);
    plate_entity
}

fn spawn_plate_handle(commands: &mut Commands, plate: Entity, size: Vec2) {
    let handle = commands
        .spawn((
            PlateIndexHandle,
            Sprite::from_color(PLATE_COLOR, Vec2::splat(24.0)),
            Pickable {
                should_block_lower: true,
                is_hoverable: true,
            },
            Transform::from_xyz(size.x / 2.0 - 12.0, -size.y / 2.0 + 12.0, 3.0),
        ))
        .observe(on_plate_handle_click)
        .id();
    commands.entity(handle).with_child((
        PlateIndexLabel,
        Text2d::new("A"),
        TextColor(Color::WHITE),
        TextFont {
            font_size: px(14).into(),
            ..text_font()
        },
        Pickable::IGNORE,
        Transform::from_xyz(0.0, 0.0, 1.0),
    ));
    commands.entity(plate).add_child(handle);
}

pub fn plate_index_label(mut index: usize) -> String {
    let mut label = String::new();
    loop {
        label.insert(0, (b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    label
}

pub fn relabel_plates(
    changed: Query<(), Or<(Added<PlateIndexHandle>, Changed<GraphNode>)>>,
    mut removed: RemovedComponents<Plate>,
    plates: Query<(Entity, &GraphNode), With<Plate>>,
    handles: Query<&ChildOf, With<PlateIndexHandle>>,
    mut labels: Query<(&ChildOf, &mut Text2d), With<PlateIndexLabel>>,
) {
    if changed.is_empty() && removed.read().next().is_none() {
        return;
    }
    let mut ordered = plates.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|(_, id)| id.0);
    let ranks = ordered
        .into_iter()
        .enumerate()
        .map(|(rank, (entity, _))| (entity, plate_index_label(rank)))
        .collect::<HashMap<_, _>>();
    for (child_of, mut text) in &mut labels {
        if let Ok(handle_parent) = handles.get(child_of.parent())
            && let Some(label) = ranks.get(&handle_parent.parent())
        {
            text.0.clone_from(label);
        }
    }
}

fn on_plate_handle_click(
    mut event: On<Pointer<Click>>,
    mut commands: Commands,
    parents: Query<&ChildOf, With<PlateIndexHandle>>,
    node_ids: Query<&GraphNode, With<Plate>>,
    selected: Option<Single<Entity, With<Selected>>>,
    selection_indicators: Query<(Entity, &ChildOf), With<SelectedIndicator>>,
    compiled: Option<Res<GraphIRResource>>,
    inference: Option<Res<InferenceStatusResource>>,
) {
    if event.count != 1 {
        return;
    }
    event.propagate(false);
    let Ok(parent) = parents.get(event.event_target()) else {
        return;
    };
    let plate = parent.parent();
    if let Some(selected) = selected {
        let selected = *selected;
        commands.entity(selected).remove::<Selected>();
        for (indicator, parent) in &selection_indicators {
            if parent.parent() == selected {
                commands.entity(indicator).despawn();
            }
        }
    }
    commands.entity(plate).insert(Selected);
    commands.trigger(ReloadSidebar);
    if inference.is_some()
        && compiled.is_some()
        && let Ok(id) = node_ids.get(plate)
    {
        commands.trigger(OpenHistogramPanel {
            subject: HistogramSubject::PlateIndex(id.0),
            clear_toasts: true,
        });
    }
}

pub fn update_plate_handle_colors(
    selected: Query<Entity, (With<Plate>, With<Selected>)>,
    added_handles: Query<(), Added<PlateIndexHandle>>,
    mut handles: Query<(&ChildOf, &mut Sprite), With<PlateIndexHandle>>,
    mut previous: Local<Option<Entity>>,
) {
    let selected = selected.iter().next();
    if *previous == selected && added_handles.is_empty() {
        return;
    }
    *previous = selected;
    for (parent, mut sprite) in &mut handles {
        sprite.color = if selected == Some(parent.parent()) {
            Color::srgb(0.18, 0.38, 0.75)
        } else {
            PLATE_COLOR
        };
    }
}

fn add_plate_borders(
    commands: &mut Commands,
    plate: Entity,
    size: Option<Vec2>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) {
    let border_mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let border_material = materials.add(PLATE_COLOR);

    commands.entity(plate).with_children(|parent| {
        let transforms = size.map(|size| {
            [
                (
                    PlateBorder::Top,
                    Vec3::new(0.0, size.y / 2.0, 0.0),
                    Vec3::new(size.x + PLATE_BORDER_THICKNESS, PLATE_BORDER_THICKNESS, 1.0),
                ),
                (
                    PlateBorder::Right,
                    Vec3::new(size.x / 2.0, 0.0, 0.0),
                    Vec3::new(PLATE_BORDER_THICKNESS, size.y, 1.0),
                ),
                (
                    PlateBorder::Bottom,
                    Vec3::new(0.0, -size.y / 2.0, 0.0),
                    Vec3::new(size.x + PLATE_BORDER_THICKNESS, PLATE_BORDER_THICKNESS, 1.0),
                ),
                (
                    PlateBorder::Left,
                    Vec3::new(-size.x / 2.0, 0.0, 0.0),
                    Vec3::new(PLATE_BORDER_THICKNESS, size.y, 1.0),
                ),
            ]
        });
        for (index, edge) in [
            PlateBorder::Top,
            PlateBorder::Right,
            PlateBorder::Bottom,
            PlateBorder::Left,
        ]
        .into_iter()
        .enumerate()
        {
            let transform = transforms
                .map(|transforms| {
                    let (_, translation, scale) = transforms[index];
                    Transform {
                        translation,
                        scale,
                        ..default()
                    }
                })
                .unwrap_or_default();
            parent.spawn((
                edge,
                Pickable {
                    should_block_lower: true,
                    is_hoverable: true,
                },
                Mesh2d(border_mesh.clone()),
                MeshMaterial2d(border_material.clone()),
                transform,
            ));
        }
    });
}

fn on_completed_plate_drag(
    event: On<Pointer<Drag>>,
    reduced_view: Res<super::ReducedView>,
    mut plates: Query<(&mut Plate, &mut Transform), Without<PlateDraft>>,
) {
    if reduced_view.active {
        return;
    }
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
            plate
                .mapping
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
    nodes: Query<(Entity, &Transform), Or<(With<RandomNode>, With<ScalarNode>)>>,
    graph_nodes: Query<&GraphNode>,
) {
    let (entity, mut plate) = plate.into_inner();
    if plate.bounds.is_substantial() {
        let mut id = 1;
        while graph_nodes.iter().any(|node| node.0 == id) {
            id += 1;
        }
        for (node_entity, transform) in &nodes {
            if plate
                .bounds
                .contains_point(transform.translation.truncate())
            {
                plate.mapping.insert(node_entity, "unobserved".to_string());
            }
        }

        commands
            .entity(entity)
            .insert(GraphNode(id))
            .remove::<PlateDraft>();
        spawn_plate_handle(&mut commands, entity, plate.bounds.size());
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

    #[test]
    fn plate_letters_are_deterministic_beyond_z() {
        assert_eq!(plate_index_label(0), "A");
        assert_eq!(plate_index_label(25), "Z");
        assert_eq!(plate_index_label(26), "AA");
        assert_eq!(plate_index_label(27), "AB");
        assert_eq!(plate_index_label(701), "ZZ");
        assert_eq!(plate_index_label(702), "AAA");
    }
}
