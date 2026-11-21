//! A layout restored with no known sizes: the first start after the
//! upgrade, a layout written by upstream OBS, Reset Docks, a service
//! layout. It is restored again at each window change until the window
//! shows it unclamped or the user changes something (#3740, P2, P5, P6).

use obs_dock_layout::{Area, Cause, Rect};

use crate::dock_layout::fixture::{CENTRAL_MIN, MIN_W, SEP, bottom, full_record, left, started};

/// The full-size blob restored into a 470 px tile: every dock clamped.
fn clamped() -> obs_dock_layout::Snapshot {
    bottom(
        470,
        900,
        200,
        &[("scenes", MIN_W), ("sources", MIN_W), ("mixer", MIN_W)],
    )
}

#[test]
fn a_clamped_start_without_sizes_restores_again_on_each_window_change() {
    let (mut model, out) = started(None, "h1", &clamped());
    assert_eq!(out.resizes, vec![]);
    assert!(model.saves_restored_blob());
    // A retile: Qt shares the new width out equally. Restore again.
    let grown = bottom(
        800,
        900,
        200,
        &[("scenes", 262), ("sources", 262), ("mixer", 264)],
    );
    let out = model.observe(&grown, Cause::Change);
    assert!(out.restore_again);
    assert_eq!(out.resizes, vec![]);
    assert!(model.saves_restored_blob());
    // The blob restored at 800 px fits: its sizes are the user's.
    let restored = bottom(
        800,
        900,
        200,
        &[("scenes", 299), ("sources", 298), ("mixer", 195)],
    );
    let out = model.observe(&restored, Cause::Restored);
    assert_eq!(out.resizes, vec![]);
    assert!(!model.saves_restored_blob());
    assert_eq!(
        model.record("h2"),
        "v1;h2;aB=200;dmixer=195;dscenes=299;dsources=298"
    );
    // From now on window changes never restore again.
    let full = bottom(
        1600,
        900,
        200,
        &[("scenes", 529), ("sources", 529), ("mixer", 530)],
    );
    let out = model.observe(&full, Cause::Change);
    assert!(!out.restore_again);
    assert_eq!(out.resizes[0].sizes, vec![600, 597, 391]);
}

#[test]
fn quit_while_still_clamped_saves_the_restored_blob() {
    let (model, _) = started(None, "h1", &clamped());
    assert!(model.saves_restored_blob());
    assert_eq!(model.record("h1"), "v1;h1");
}

#[test]
fn the_first_placement_change_ends_the_restores() {
    // P2, P5: a dock moved, or Reset Docks, in a small window is kept.
    let (mut model, _) = started(None, "h1", &clamped());
    let mut hidden = clamped();
    hidden.docks[2].shown = false;
    let out = model.observe(&hidden, Cause::Change);
    assert!(!out.restore_again);
    assert!(!model.saves_restored_blob());
    let grown = bottom(
        800,
        900,
        200,
        &[("scenes", 398), ("sources", 398), ("mixer", 0)],
    );
    let mut grown = grown;
    grown.docks[2].shown = false;
    assert!(!model.observe(&grown, Cause::Change).restore_again);
}

#[test]
fn moving_a_floating_dock_ends_the_restores() {
    // P6: a floating dock's position is the user's.
    let mut start = clamped();
    start.docks.push(obs_dock_layout::Dock {
        floating: true,
        ..crate::dock_layout::fixture::dock(
            "chat",
            Area::Right,
            Rect {
                x: 50,
                y: 50,
                width: 300,
                height: 400,
            },
        )
    });
    let (mut model, _) = started(None, "h1", &start);
    let mut moved = start.clone();
    moved.docks[3].rect.x = 500;
    assert!(!model.observe(&moved, Cause::Change).restore_again);
    assert!(!model.saves_restored_blob());
}

#[test]
fn a_drag_ends_the_restores() {
    let (mut model, _) = started(None, "h1", &clamped());
    let dragged = bottom(
        470,
        900,
        260,
        &[("scenes", MIN_W), ("sources", MIN_W), ("mixer", MIN_W)],
    );
    model.observe(&dragged, Cause::SeparatorDrag);
    assert!(!model.saves_restored_blob());
    assert_eq!(model.sizes().depth[&Area::Bottom], 260);
}

#[test]
fn a_layout_written_by_upstream_in_between_starts_without_sizes() {
    // E25: the record's hash is for another blob.
    let (model, _) = started(Some(&full_record()), "upstream", &clamped());
    assert!(model.saves_restored_blob());
    assert!(model.sizes().length.is_empty());
}

#[test]
fn a_record_that_misses_a_docked_dock_restores_until_it_is_learned() {
    let with_new = bottom(
        1600,
        900,
        200,
        &[
            ("scenes", 450),
            ("sources", 450),
            ("mixer", 300),
            ("new", 384),
        ],
    );
    let (model, out) = started(Some(&full_record()), "h1", &with_new);
    // Unclamped: learned at once, in the row's scale, so the screen
    // already shows the shares.
    assert!(!model.saves_restored_blob());
    assert_eq!(model.sizes().length["new"], 508);
    assert_eq!(out.resizes, vec![]);
}

#[test]
fn a_click_on_a_separator_that_resizes_nothing_is_no_change() {
    let (mut model, _) = started(None, "h1", &clamped());
    model.observe(&clamped(), Cause::SeparatorDrag);
    assert!(model.saves_restored_blob());
    let grown = bottom(
        800,
        900,
        200,
        &[("scenes", 262), ("sources", 262), ("mixer", 264)],
    );
    assert!(model.observe(&grown, Cause::Change).restore_again);
}

#[test]
fn a_dock_that_appears_during_the_restores_ends_them() {
    // Open for step 3: the model cannot tell a plugin's load-time dock
    // from a browser dock the user added. The adapter sends the Restored
    // snapshot after the load-time docks exist.
    let (mut model, _) = started(None, "h1", &clamped());
    let mut more = clamped();
    more.docks.push(crate::dock_layout::fixture::dock(
        "plugin",
        Area::Right,
        Rect {
            x: 300,
            y: 0,
            width: 160,
            height: 600,
        },
    ));
    assert!(!model.observe(&more, Cause::Change).restore_again);
    assert!(!model.saves_restored_blob());
}

#[test]
fn a_side_column_at_its_minimum_with_room_is_learned() {
    // The user left the column as narrow as it goes, and the preview has
    // room: that width is the user's, not a clamp. Seen with real Qt
    // (test_dock_state_restart): the record never got the column's depth,
    // so every start had "no sizes" and the record was ignored.
    let column = [("scenes", 440), ("sources", 440)];
    let (model, out) = started(None, "h1", &left(1600, 884, MIN_W, &column));
    assert!(!out.restore_again);
    assert!(!model.saves_restored_blob());
    assert_eq!(model.sizes().depth.get(&Area::Left), Some(&MIN_W));
}

#[test]
fn a_side_column_at_its_minimum_without_room_is_a_clamp() {
    // The preview is at its minimum too: the window is too small.
    let column = [("scenes", 440), ("sources", 440)];
    let snap = left(MIN_W + SEP + CENTRAL_MIN, 884, MIN_W, &column);
    let (model, _) = started(None, "h1", &snap);
    assert!(model.saves_restored_blob());
    assert_eq!(model.sizes().depth.get(&Area::Left), None);
}
