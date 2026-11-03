use super::*;

#[test]
fn capsule_boundary_distance_matches_sides_and_end_caps() {
    let capsule = EndpointShape::horizontal_capsule(20.0, 36.0);

    assert!((capsule.boundary_distance(Vec2::Y) - 20.0).abs() < 0.001);
    assert!((capsule.boundary_distance(Vec2::X) - 38.0).abs() < 0.001);

    let diagonal = Vec2::new(1.0, 1.0).normalize();
    let point = diagonal * capsule.boundary_distance(diagonal);
    let cap_center = Vec2::new(18.0, 0.0);
    assert!((point.distance(cap_center) - 20.0).abs() < 0.001);
}
