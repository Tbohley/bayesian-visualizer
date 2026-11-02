//! Model snapshots use stable graph IDs; undo never retains stale Bevy entities.
use crate::{bevy_to_fugue::*, data_vis::*, graph::*, nodes::*, sidebar::*};
use bevy::{input_focus::InputFocus, prelude::*};
use std::{collections::HashMap, sync::atomic::Ordering};

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Random(Option<String>, String, Vec<(&'static str, Option<u32>)>),
    Compute(Operation, Vec<(&'static str, Option<u32>)>),
    Scalar(f64, Option<String>),
    Plate(PlateBounds, Dataset, Vec<(u32, String)>),
}
#[derive(Clone, Debug, Default, PartialEq)]
struct Snapshot {
    nodes: Vec<(u32, Vec3, Value)>,
    links: Vec<(u32, u32)>,
}
#[derive(Resource, Default)]
pub struct History {
    past: Vec<Snapshot>,
    current: Snapshot,
    future: Vec<Snapshot>,
    dirty: bool,
}
impl History {
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
    fn commit(&mut self, snapshot: Snapshot) {
        if self.current == snapshot {
            return;
        }
        let previous = std::mem::replace(&mut self.current, snapshot);
        self.past.push(previous);
        if self.past.len() > 100 {
            self.past.remove(0);
        }
        self.future.clear();
    }
    fn step(&mut self, redo: bool) -> Option<Snapshot> {
        let next = if redo {
            self.future.pop()
        } else {
            self.past.pop()
        }?;
        let old = std::mem::replace(&mut self.current, next.clone());
        if redo {
            self.past.push(old);
        } else {
            self.future.push(old);
        }
        Some(next)
    }
}
#[derive(Resource, Default)]
pub struct HistoryRequest(pub Option<bool>);
pub struct HistoryPlugin;
impl Plugin for HistoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<History>()
            .init_resource::<HistoryRequest>()
            .add_systems(Last, track_history);
    }
}
fn capture(world: &mut World) -> Snapshot {
    let mut query = world.query::<(
        Entity,
        &GraphNode,
        &Transform,
        Option<&RandomNode>,
        Option<&ComputeNode>,
        Option<&ScalarNode>,
        Option<&Plate>,
    )>();
    let ids: HashMap<_, _> = query.iter(world).map(|(e, id, ..)| (e, id.0)).collect();
    let params = |p: &[ParamValue]| {
        p.iter()
            .map(|p| (p.0, p.1.and_then(|e| ids.get(&e).copied())))
            .collect()
    };
    let mut snapshot = Snapshot::default();
    for (_, id, t, r, c, s, p) in query.iter(world) {
        let value = if let Some(r) = r {
            Value::Random(r.name.clone(), r.dist_type.clone(), params(&r.params))
        } else if let Some(c) = c {
            Value::Compute(c.operation, params(&c.params))
        } else if let Some(s) = s {
            Value::Scalar(s.val, s.name.clone())
        } else if let Some(p) = p {
            let mut mappings: Vec<_> = p
                .mapping
                .iter()
                .filter_map(|(e, s)| Some((*ids.get(e)?, s.clone())))
                .collect();
            mappings.sort();
            Value::Plate(p.bounds, p.data.clone(), mappings)
        } else {
            continue;
        };
        snapshot.nodes.push((id.0, t.translation, value));
    }
    snapshot.nodes.sort_by_key(|n| n.0);
    let mut links = world.query_filtered::<&GraphLink, Without<UnfinishedLink>>();
    snapshot.links = links
        .iter(world)
        .filter_map(|l| Some((*ids.get(&l.from)?, *ids.get(&l.to?)?)))
        .collect();
    snapshot.links.sort();
    snapshot
}
fn track_history(world: &mut World) {
    // Copy model data only after a graph change; coalesce held-pointer edits.
    let request = world.resource_mut::<HistoryRequest>().0.take();
    let pressed = world
        .resource::<ButtonInput<MouseButton>>()
        .pressed(MouseButton::Left);
    let changed = {
        let mut query = world.query::<(
            Ref<GraphNode>,
            Ref<Transform>,
            Option<Ref<RandomNode>>,
            Option<Ref<ComputeNode>>,
            Option<Ref<ScalarNode>>,
            Option<Ref<Plate>>,
        )>();
        query.iter(world).any(|(n, t, r, c, s, p)| {
            n.is_changed()
                || t.is_changed()
                || r.is_some_and(|v| v.is_changed())
                || c.is_some_and(|v| v.is_changed())
                || s.is_some_and(|v| v.is_changed())
                || p.is_some_and(|v| v.is_changed())
        })
    };
    // Also track deletion and edges. The small ID list avoids copying large plate datasets every frame.
    let count = {
        let mut q = world.query_filtered::<Entity, (
            Without<UnfinishedLink>,
            Or<(With<GraphNode>, With<GraphLink>)>,
        )>();
        q.iter(world).count()
    };
    let previous_count = {
        let h = world.resource::<History>();
        h.current.nodes.len() + h.current.links.len()
    };
    let links_changed = {
        let mut q = world.query_filtered::<Ref<GraphLink>, Without<UnfinishedLink>>();
        q.iter(world).any(|l| l.is_changed())
    };
    let dirty =
        world.resource::<History>().dirty || changed || links_changed || count != previous_count;
    world.resource_mut::<History>().dirty = dirty;
    if pressed && request.is_none() {
        return;
    }
    if dirty || request.is_some() {
        let snapshot = capture(world);
        world.resource_mut::<History>().commit(snapshot);
        world.resource_mut::<History>().dirty = false;
    }
    if let Some(redo) = request {
        if let Some(snapshot) = world.resource_mut::<History>().step(redo) {
            restore(world, &snapshot);
        }
    }
}
fn restore(world: &mut World, snapshot: &Snapshot) {
    if let Some(job) = world.get_resource::<InferenceJob>() {
        job.control.discard_result.store(true, Ordering::Relaxed);
        job.control.cancel_requested.store(true, Ordering::Relaxed);
    }
    let entities = {
        let mut q = world.query_filtered::<Entity, Or<(
            With<GraphNode>,
            With<GraphLink>,
            With<ReducedViewLink>,
            With<PlateDraft>,
            With<LocalSidebar>,
        )>>();
        q.iter(world).collect::<Vec<_>>()
    };
    for e in entities {
        world.despawn(e);
    }
    world.remove_resource::<GraphIRResource>();
    world.remove_resource::<InferenceResultResource>();
    world.remove_resource::<InferenceStatusResource>();
    world.remove_resource::<SampleSelections>();
    world.remove_resource::<PlateIndexSelections>();
    world.resource_mut::<InputFocus>().clear();
    let mut meshes = world.remove_resource::<Assets<Mesh>>().unwrap();
    let mut materials = world.remove_resource::<Assets<ColorMaterial>>().unwrap();
    let mut entities = HashMap::new();
    let mut positions = HashMap::new();
    let mut shapes = HashMap::new();
    {
        let mut commands = world.commands();
        for (id, position, value) in &snapshot.nodes {
            let entity = match value {
                Value::Random(name, dist, _) => spawn_random(
                    &mut commands,
                    *position,
                    *id,
                    RandomNode {
                        name: name.clone(),
                        dist_type: dist.clone(),
                        params: vec![],
                    },
                    &mut meshes,
                    &mut materials,
                ),
                Value::Compute(operation, _) => spawn_compute(
                    &mut commands,
                    *position,
                    *id,
                    ComputeNode {
                        operation: *operation,
                        params: vec![],
                    },
                    &mut meshes,
                    &mut materials,
                ),
                Value::Scalar(val, name) => spawn_scalar(
                    &mut commands,
                    *position,
                    *id,
                    ScalarNode {
                        val: *val,
                        name: name.clone(),
                    },
                    &mut meshes,
                    &mut materials,
                ),
                Value::Plate(..) => continue,
            };
            commands
                .entity(entity)
                .insert(Transform::from_translation(*position));
            entities.insert(*id, entity);
            positions.insert(*id, *position);
            shapes.insert(
                *id,
                match value {
                    Value::Random(name, ..) => EndpointShape::horizontal_capsule(
                        crate::RANDOM_NODE_RAD,
                        random_node_straight_length(
                            &name.clone().unwrap_or_else(|| id.to_string()),
                        ),
                    ),
                    Value::Compute(..) => EndpointShape::circle(crate::COMPUTE_NODE_RAD),
                    _ => EndpointShape::circle(crate::SCALAR_NODE_RAD),
                },
            );
        }
        for (id, position, value) in &snapshot.nodes {
            let params = |p: &Vec<(&'static str, Option<u32>)>| {
                p.iter()
                    .map(|(name, id)| {
                        ParamValue(name, id.and_then(|id| entities.get(&id).copied()))
                    })
                    .collect::<Vec<_>>()
            };
            match value {
                Value::Random(name, dist, p) => {
                    commands.entity(entities[id]).insert(RandomNode {
                        name: name.clone(),
                        dist_type: dist.clone(),
                        params: params(p),
                    });
                }
                Value::Compute(op, p) => {
                    commands.entity(entities[id]).insert(ComputeNode {
                        operation: *op,
                        params: params(p),
                    });
                }
                Value::Plate(bounds, data, mapping) => {
                    let entity = spawn_completed_plate(
                        &mut commands,
                        *id,
                        Plate {
                            origin: bounds.center(),
                            bounds: *bounds,
                            data: data.clone(),
                            mapping: mapping
                                .iter()
                                .filter_map(|(id, s)| Some((*entities.get(id)?, s.clone())))
                                .collect(),
                        },
                        &mut meshes,
                        &mut materials,
                    );
                    commands
                        .entity(entity)
                        .insert(Transform::from_translation(*position));
                }
                _ => {}
            }
        }
        for (from, to) in &snapshot.links {
            if let (Some(a), Some(b)) = (entities.get(from), entities.get(to)) {
                spawn_finished_link(
                    &mut commands,
                    *a,
                    *b,
                    positions[from],
                    positions[to],
                    shapes[from],
                    shapes[to],
                    &mut meshes,
                    &mut materials,
                );
            }
        }
        commands.trigger(CloseContextMenus);
        commands.trigger(CloseHistogramPanel);
        commands.trigger(SetInferenceControlsEnabled(false));
        commands.trigger(ReloadSidebar);
    }
    world.insert_resource(meshes);
    world.insert_resource(materials);
    world.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model(value: f64) -> Snapshot {
        Snapshot {
            nodes: vec![(1, Vec3::ZERO, Value::Scalar(value, None))],
            links: vec![],
        }
    }
    #[test]
    fn undo_redo_and_branching() {
        let mut h = History::default();
        h.commit(model(1.));
        h.commit(model(2.));
        assert_eq!(h.step(false), Some(model(1.)));
        assert_eq!(h.step(true), Some(model(2.)));
        h.step(false);
        h.commit(model(3.));
        assert!(!h.can_redo());
        assert_eq!(h.step(false), Some(model(1.)));
    }
    #[test]
    fn no_op_does_not_consume_history_or_clear_redo() {
        let mut h = History::default();
        h.commit(model(1.));
        h.commit(model(2.));
        h.step(false);
        h.commit(model(1.));
        assert_eq!(h.past.len(), 1);
        assert!(h.can_redo());
    }
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            HistoryPlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<ColorMaterial>>()
        .init_resource::<InputFocus>()
        .init_resource::<ButtonInput<MouseButton>>();
        app
    }
    #[test]
    fn restores_entity_references_in_parameters_links_and_observations() {
        let mut app = test_app();
        let a = app
            .world_mut()
            .spawn((
                GraphNode(1),
                Transform::from_xyz(10., 10., 1.),
                ScalarNode {
                    val: 2.,
                    name: None,
                },
            ))
            .id();
        let b = app
            .world_mut()
            .spawn((
                GraphNode(2),
                Transform::from_xyz(50., 10., 1.),
                ComputeNode {
                    operation: Operation::Add,
                    params: vec![ParamValue("a", Some(a)), ParamValue("b", Some(a))],
                },
            ))
            .id();
        app.world_mut().spawn(GraphLink {
            from: a,
            to: Some(b),
        });
        app.world_mut().spawn((
            GraphNode(3),
            Transform::from_xyz(30., 10., crate::PLATE_Z),
            Plate {
                origin: Vec2::ZERO,
                bounds: PlateBounds::from_points(Vec2::ZERO, Vec2::splat(80.)),
                data: Dataset {
                    name: "example".into(),
                    n: 1,
                    data: HashMap::from([("x".into(), vec![4.])]),
                },
                mapping: HashMap::from([(a, "x".into())]),
            },
        ));
        app.update();
        let expected = capture(app.world_mut());
        app.world_mut().resource_mut::<HistoryRequest>().0 = Some(false);
        app.update();
        assert!(capture(app.world_mut()).nodes.is_empty());
        app.world_mut().resource_mut::<HistoryRequest>().0 = Some(true);
        app.update();
        assert_eq!(capture(app.world_mut()), expected);
        assert!(app.world().get_entity(a).is_err());
        // A quiet frame after restoration must preserve redo/undo and stable IDs.
        app.update();
        assert_eq!(app.world().resource::<History>().past.len(), 1);
    }
    #[test]
    fn one_drag_is_one_undo_and_camera_moves_are_excluded() {
        let mut app = test_app();
        let a = app
            .world_mut()
            .spawn((
                GraphNode(1),
                Transform::default(),
                ScalarNode {
                    val: 2.,
                    name: None,
                },
            ))
            .id();
        let camera = app.world_mut().spawn(Transform::default()).id();
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        for x in [10., 20., 30.] {
            app.world_mut()
                .get_mut::<Transform>(a)
                .unwrap()
                .translation
                .x = x;
            app.update();
        }
        assert_eq!(app.world().resource::<History>().past.len(), 1);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert_eq!(app.world().resource::<History>().past.len(), 2);
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .translation
            .x = 100.;
        app.update();
        assert_eq!(app.world().resource::<History>().past.len(), 2);
        app.world_mut().resource_mut::<HistoryRequest>().0 = Some(false);
        app.update();
        let snapshot = capture(app.world_mut());
        assert_eq!(snapshot.nodes[0].1, Vec3::ZERO);
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation.x,
            100.
        );
    }
}
