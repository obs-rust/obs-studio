use obs_graphics::image_orientation::{matrix_bytes, orientation};

#[test]
fn metadata_wins_even_zero_or_out_of_range() {
    for value in [-7, 0, 1, 8, 99] {
        assert_eq!(orientation(|| Some(value), || panic!("matrix queried"), |_| panic!("converted")), value);
    }
}

#[test]
fn missing_or_short_matrix_has_no_orientation() {
    assert_eq!(orientation(|| None, || None, |_| panic!("converted")), 0);
    for len in 0..36 {
        assert_eq!(matrix_bytes(&[0; 36][..len]), None);
    }
}

#[test]
fn copies_all_nine_elements_and_ignores_trailing_bytes() {
    let matrix = [i32::MIN, -65536, -1, 0, 1, 65536, i32::MAX, 42, 9];
    let bytes: Vec<_> = matrix.into_iter().flat_map(i32::to_ne_bytes).chain([99]).collect();
    assert_eq!(matrix_bytes(&bytes), Some(matrix));
    assert_eq!(orientation(|| None, || matrix_bytes(&bytes), |m| { assert_eq!(*m, matrix); 7 }), 7);
}
