//! Window changes: they put the wanted sizes back and never change them
//! (rules R2, R3, R9; decision D1).

use obs_dock_layout::{Area, Axis, Cause, Resize};

use crate::dock_layout::fixture::{
    FULL, MIN_W, applied, bottom, full_record, left, lengths, settle, started,
};

#[test]
fn a_layout_restored_at_its_size_needs_no_resize() {
    let snap = bottom(1600, 900, 200, &FULL);
    let (mut model, out) = started(Some(&full_record()), "h1", &snap);
    assert_eq!(out.resizes, vec![]);
    assert!(!out.restore_again);
    assert!(!model.saves_restored_blob());
    // The model's own fixed point: the same snapshot again changes nothing.
    assert_eq!(model.observe(&snap, Cause::Change).resizes, vec![]);
    assert_eq!(model.record("h1"), full_record());
}

#[test]
fn a_size_change_that_is_not_a_drag_is_not_the_users() {
    let snap = bottom(1600, 900, 200, &FULL);
    let (mut model, _) = started(Some(&full_record()), "h1", &snap);
    let moved = bottom(
        1600,
        900,
        200,
        &[("scenes", 300), ("sources", 892), ("mixer", 396)],
    );
    let out = model.observe(&moved, Cause::Change);
    assert_eq!(model.record("h1"), full_record());
    // It is put back.
    assert_eq!(
        lengths(&applied(&moved, &out), Area::Bottom),
        vec![597, 595, 396]
    );
}

#[test]
fn narrow_then_wide_gets_the_shares_back() {
    // E2 (#2225): Qt shares the space out equally when the window grows
    // after a clamp.
    let (mut model, _) = started(Some(&full_record()), "h1", &bottom(1600, 900, 200, &FULL));
    let narrow = bottom(
        470,
        900,
        200,
        &[("scenes", MIN_W), ("sources", MIN_W), ("mixer", MIN_W)],
    );
    let out = model.observe(&narrow, Cause::Change);
    assert_eq!(
        out.resizes,
        vec![],
        "too small: wait for the window to grow"
    );
    let wide = bottom(
        1600,
        900,
        200,
        &[("scenes", 529), ("sources", 529), ("mixer", 530)],
    );
    let out = model.observe(&wide, Cause::Change);
    assert_eq!(
        out.resizes,
        vec![Resize {
            axis: Axis::Horizontal,
            docks: vec!["scenes".into(), "sources".into(), "mixer".into()],
            sizes: vec![597, 595, 396],
        }]
    );
    assert_eq!(model.record("h1"), full_record());
}

#[test]
fn a_half_size_window_gets_the_shares_of_the_row() {
    let (mut model, _) = started(Some(&full_record()), "h1", &bottom(1600, 900, 200, &FULL));
    let half = settle(
        &mut model,
        bottom(
            800,
            900,
            200,
            &[("scenes", 264), ("sources", 264), ("mixer", 264)],
        ),
    );
    assert_eq!(lengths(&half, Area::Bottom), vec![298, 296, 198]);
    // Quit while narrow (E3): the wanted sizes are saved, not the screen.
    assert_eq!(model.record("h1"), full_record());
}

#[test]
fn a_dock_area_keeps_its_pixels_on_a_bigger_window() {
    // D1: the left column keeps 280 px; the preview gets the extra space.
    let column = [("scenes", 440), ("sources", 440)];
    let (mut model, _) = started(
        Some("v1;h1;aL=280;dscenes=440;dsources=440"),
        "h1",
        &left(1600, 884, 280, &column),
    );
    // The preview is at its minimum: no room for the column.
    let narrow = left(MIN_W + 4 + 100, 884, MIN_W, &column);
    assert_eq!(model.observe(&narrow, Cause::Change).resizes, vec![]);
    // Qt gives all of the new width to the preview.
    let wide = left(2000, 884, MIN_W, &column);
    assert_eq!(
        model.observe(&wide, Cause::Change).resizes,
        vec![Resize {
            axis: Axis::Horizontal,
            docks: vec!["scenes".into()],
            sizes: vec![280],
        }]
    );
}

#[test]
fn a_hidden_window_decides_nothing() {
    // R9: tray, minimized, not shown yet.
    let (mut model, _) = started(Some(&full_record()), "h1", &bottom(1600, 900, 200, &FULL));
    let mut hidden = bottom(0, 0, 0, &[]);
    hidden.visible = false;
    assert_eq!(model.observe(&hidden, Cause::Change).resizes, vec![]);
    assert_eq!(model.observe(&hidden, Cause::SeparatorDrag).resizes, vec![]);
    // The next visible snapshot compares with the last visible one.
    assert_eq!(
        model
            .observe(&bottom(1600, 900, 200, &FULL), Cause::Change)
            .resizes,
        vec![]
    );
    assert_eq!(model.record("h1"), full_record());
}

#[test]
fn extreme_numbers_never_panic() {
    // The C ABI passes whatever Qt (or a broken caller) reports; the
    // tests run with overflow checks.
    let values = [i32::MIN, -1, 0, 1, obs_dock_layout::LIMIT, i32::MAX];
    for &v in &values {
        for &w in &values {
            let mut model = obs_dock_layout::Model::new();
            model.start(Some("v1;h;aB=1048576;dscenes=1;dsources=1048576"), "h");
            let mut snap = bottom(1600, 900, 200, &[("scenes", 597), ("sources", 595)]);
            snap.width = v;
            snap.central.width = w;
            snap.central_min_width = v;
            snap.docks[0].rect = obs_dock_layout::Rect {
                x: v,
                y: 700,
                width: w,
                height: v,
            };
            snap.docks[1].rect = obs_dock_layout::Rect {
                x: w,
                y: 700,
                width: v,
                height: w,
            };
            snap.docks[0].min_width = v;
            snap.docks[1].min_height = w;
            let mut other = snap.clone();
            other.docks[0].rect.width = v;
            other.docks[1].rect.width = w;
            for (s, cause) in [
                (&snap, Cause::Restored),
                (&other, Cause::Change),
                (&snap, Cause::SeparatorDrag),
                (&other, Cause::SeparatorDrag),
                (&snap, Cause::Change),
            ] {
                let _ = model.observe(s, cause);
            }
            let _ = model.record("h");
        }
    }
}
