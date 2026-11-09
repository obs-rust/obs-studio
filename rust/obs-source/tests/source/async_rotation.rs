use obs_source::async_rotation::normalize;

#[test]
fn test_async_rotation_normalize_characterized_angles() {
    for (input, expected) in [
        (0, 0), (90, 90), (180, 180), (270, 270), (-90, 270),
        (-180, 180), (-270, 90), (360, 0), (450, 90), (-450, 270),
        (720, 0), (-360, 0), (1, 1), (-1, 359), (45, 45), (-45, 315),
        (135, 135), (315, 315), (359, 359), (-359, 1), (361, 1),
        (i64::MAX, 7), (i64::MIN, 352), (i64::MIN + 1, 353),
        (i64::MAX - 7, 0), (i64::MAX - 97, 270), (i64::MAX - 187, 180),
        (i64::MIN + 8, 0), (i64::MIN + 98, 90), (i64::MIN + 188, 180),
        (i64::from(i32::MAX), 127), (i64::from(i32::MIN), 232),
    ] {
        assert_eq!(normalize(input), expected, "rotation {input}");
        assert_eq!(normalize(input) % 180 != 0, input % 180 != 0);
    }
}

#[test]
fn test_async_rotation_normalize_changing_rotation() {
    let mut rotation = 0;
    for (input, expected) in [(-180, 180), (450, 90), (-90, 270), (0, 0)] {
        assert_eq!(rotation, normalize(rotation));
        rotation = normalize(input);
        assert_eq!(rotation, expected);
    }
}
