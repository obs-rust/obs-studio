//! The `BasicWindow/DockSizes` record (decision D6).

use obs_dock_layout::{Area, Model, Sizes, decode, encode};

#[test]
fn round_trip_with_names_that_need_escaping() {
    let mut sizes = Sizes::default();
    sizes.depth.insert(Area::Left, 280);
    sizes.depth.insert(Area::Bottom, 200);
    sizes.length.insert("scenes".to_owned(), 597);
    sizes.length.insert("a;b=c%d".to_owned(), 7);
    let text = encode("abc123", &sizes);
    assert_eq!(text, "v1;abc123;aL=280;aB=200;da%3Bb%3Dc%25d=7;dscenes=597");
    assert_eq!(decode(&text), Some(("abc123".to_owned(), sizes)));
}

#[test]
fn a_record_without_sizes_reads_back() {
    assert_eq!(decode("v1;h"), Some(("h".to_owned(), Sizes::default())));
}

#[test]
fn anything_else_is_no_record() {
    for text in [
        "",
        "v2;h;dscenes=1",
        "v1",
        "v1;h;dscenes",
        "v1;h;dscenes=x",
        "v1;h;dscenes=0",
        "v1;h;dscenes=-5",
        "v1;h;dscenes=1048577",
        "v1;h;aX=10",
        "v1;h;xscenes=10",
        "v1;h;da%2=1",
        "v1;h;da%41=1",
    ] {
        assert_eq!(decode(text), None, "{text:?}");
    }
}

#[test]
fn start_uses_a_record_only_for_its_blob() {
    let mut model = Model::new();
    assert!(model.start(Some("v1;h1;dscenes=597"), "h1"));
    assert_eq!(model.sizes().length.get("scenes"), Some(&597));
    // Upstream OBS wrote DockState in between: the sizes are stale.
    assert!(!model.start(Some("v1;h1;dscenes=597"), "h2"));
    assert!(model.sizes().length.is_empty());
    assert!(!model.start(None, "h1"));
    assert!(!model.start(Some("garbage"), "h1"));
}

#[test]
fn the_record_names_the_blob_it_is_saved_with() {
    let mut model = Model::new();
    model.start(Some("v1;h1;aB=200;dscenes=597"), "h1");
    assert_eq!(model.record("h9"), "v1;h9;aB=200;dscenes=597");
}
