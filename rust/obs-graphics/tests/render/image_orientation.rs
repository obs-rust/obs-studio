use obs_graphics::image_orientation::{matrix_bytes, orientation};

#[test]
fn metadata_wins_even_zero_or_out_of_range() {
    for value in [-7, 0, 1, 8, 99] {
        assert_eq!(
            orientation(
                || Some(value),
                || panic!("matrix queried"),
                |_| panic!("converted")
            ),
            value
        );
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
    let bytes: Vec<_> = matrix
        .into_iter()
        .flat_map(i32::to_ne_bytes)
        .chain([99])
        .collect();
    assert_eq!(matrix_bytes(&bytes), Some(matrix));
    assert_eq!(
        orientation(
            || None,
            || matrix_bytes(&bytes),
            |m| {
                assert_eq!(*m, matrix);
                7
            }
        ),
        7
    );
}

// Mirrors test_image_exif_orientation's dimensions and four corner colours
// after selection. Codec decoding and the BGRA/BGRX output contract stay Tier 2.
#[test]
fn test_image_exif_orientation_dimensions_and_corners() {
    use obs_graphics::image_orient::{orient_copy, orient_swaps};
    const W: usize = 32;
    const H: usize = 16;
    let corners = [
        [0, 1, 2, 3],
        [0, 1, 2, 3],
        [1, 0, 3, 2],
        [3, 2, 1, 0],
        [2, 3, 0, 1],
        [0, 2, 1, 3],
        [2, 0, 3, 1],
        [3, 1, 2, 0],
        [1, 3, 0, 2],
    ];
    let source: Vec<u8> = (0..H)
        .flat_map(|y| {
            (0..W).flat_map(move |x| {
                let colour = u8::from(x >= W / 2) + 2 * u8::from(y >= H / 2);
                [colour, colour, colour, 255]
            })
        })
        .collect();
    for (value, expected) in corners.into_iter().enumerate() {
        let selected = orientation(
            || (value != 0).then_some(value as i32),
            || None,
            |_| panic!("no matrix"),
        );
        let (cx, cy) = if orient_swaps(selected) {
            (H, W)
        } else {
            (W, H)
        };
        let expected_size = if value >= 5 { (H, W) } else { (W, H) };
        assert_eq!((cx, cy), expected_size, "orientation {value}");
        let mut pixels = vec![0; source.len()];
        orient_copy(&mut pixels, &source, W, H, selected);
        for ((x, y), colour) in [(1, 1), (cx - 2, 1), (1, cy - 2), (cx - 2, cy - 2)]
            .into_iter()
            .zip(expected)
        {
            assert_eq!(pixels[(y * cx + x) * 4], colour, "orientation {value}");
        }
    }
}
