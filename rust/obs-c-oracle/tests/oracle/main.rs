//! obs-c-oracle integration tests: oracle (one module per former tests/*.rs file, #968).

mod parity_cases;
mod private_symbols;

#[test]
fn encoder_metadata_links_without_obs_output() {
    unsafe extern "C" {
        fn oracle_encoder_metadata_layout(layout: *mut usize);
    }
    let mut layout = [0; 11];
    // SAFETY: the oracle writes exactly eleven size_t values into this array.
    unsafe { oracle_encoder_metadata_layout(layout.as_mut_ptr()) };
    assert!(layout[0] > 0);
    assert_eq!(&layout[9..], &[0, 1]);
}
