//! Separator drags: the user's sizes (rules R5, R7; decision D3).

use obs_dock_layout::{Area, Cause};

use crate::dock_layout::fixture::{FULL, MIN_W, bottom, full_record, left, started};

#[test]
fn a_drag_in_a_full_window_records_the_screen() {
    let (mut model, _) = started(Some(&full_record()), "h1", &bottom(1600, 900, 200, &FULL));
    let dragged = bottom(
        1600,
        900,
        200,
        &[("scenes", 797), ("sources", 395), ("mixer", 396)],
    );
    assert_eq!(
        model.observe(&dragged, Cause::SeparatorDrag).resizes,
        vec![]
    );
    assert_eq!(
        model.record("h1"),
        "v1;h1;aB=200;dmixer=396;dscenes=797;dsources=395"
    );
}

#[test]
fn a_drag_in_a_clamped_row_keeps_the_untouched_shares() {
    // D3: wanted 600/600/400, shown at half size. The drag resizes Scenes
    // and Sources; Mixer keeps its share, and the two dragged docks share
    // the rest of the row's wanted total in their new ratio.
    let record = "v1;h1;aB=200;dmixer=400;dscenes=600;dsources=600";
    let (mut model, _) = started(
        Some(record),
        "h1",
        &bottom(
            1608,
            900,
            200,
            &[("scenes", 600), ("sources", 600), ("mixer", 400)],
        ),
    );
    let half = bottom(
        808,
        900,
        200,
        &[("scenes", 300), ("sources", 300), ("mixer", 200)],
    );
    model.observe(&half, Cause::Change);
    let dragged = bottom(
        808,
        900,
        200,
        &[("scenes", 350), ("sources", 250), ("mixer", 200)],
    );
    model.observe(&dragged, Cause::SeparatorDrag);
    assert_eq!(
        model.record("h1"),
        "v1;h1;aB=200;dmixer=400;dscenes=700;dsources=500"
    );
}

#[test]
fn a_dock_dragged_to_its_minimum_stays_there() {
    // R7 (#3740): a minimum on purpose is the user's size.
    let (mut model, _) = started(Some(&full_record()), "h1", &bottom(1600, 900, 200, &FULL));
    let dragged = bottom(
        1600,
        900,
        200,
        &[("scenes", MIN_W), ("sources", 1042), ("mixer", 396)],
    );
    model.observe(&dragged, Cause::SeparatorDrag);
    assert_eq!(model.sizes().length["scenes"], MIN_W);
    // A later window change keeps it there.
    let wider = bottom(
        2000,
        900,
        200,
        &[("scenes", 664), ("sources", 664), ("mixer", 664)],
    );
    let out = model.observe(&wider, Cause::Change);
    assert_eq!(out.resizes[0].sizes, vec![188, 1307, 497]);
}

#[test]
fn an_area_edge_drag_records_pixels() {
    let column = [("scenes", 440), ("sources", 440)];
    let (mut model, _) = started(
        Some("v1;h1;aL=280;dscenes=440;dsources=440"),
        "h1",
        &left(1600, 884, 280, &column),
    );
    model.observe(&left(1600, 884, 350, &column), Cause::SeparatorDrag);
    assert_eq!(model.sizes().depth[&Area::Left], 350);
}

#[test]
fn a_drag_that_changes_the_window_size_is_a_window_change() {
    let (mut model, _) = started(Some(&full_record()), "h1", &bottom(1600, 900, 200, &FULL));
    let squeezed = bottom(
        800,
        900,
        200,
        &[("scenes", 400), ("sources", 200), ("mixer", 192)],
    );
    model.observe(&squeezed, Cause::SeparatorDrag);
    assert_eq!(model.record("h1"), full_record());
}
