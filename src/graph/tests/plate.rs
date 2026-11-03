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
