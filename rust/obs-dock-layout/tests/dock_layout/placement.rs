//! Placement changes by the user: kept, also with a window change at the
//! same time (rule R4, #5230).

use obs_dock_layout::{Area, Cause};

use crate::dock_layout::fixture::{FULL, applied, bottom, full_record, lengths, settle, started};

#[test]
fn hide_and_show_round_trip() {
    let (mut model, _) = started(Some(&full_record()), "h1", &bottom(1600, 900, 200, &FULL));
    // Qt gives the hidden Mixer's space to its neighbour.
    let mut hidden = bottom(
        1600,
        900,
        200,
        &[("scenes", 597), ("sources", 995), ("mixer", 0)],
    );
    hidden.docks[2].shown = false;
    let hidden = settle(&mut model, hidden);
    assert_eq!(lengths(&hidden, Area::Bottom), vec![797, 795]);
    // Shown again, Qt gives it some space; the model puts the shares back.
    let shown = settle(
        &mut model,
        bottom(
            1600,
            900,
            200,
            &[("scenes", 597), ("sources", 595), ("mixer", 396)],
        ),
    );
    assert_eq!(lengths(&shown, Area::Bottom), vec![597, 595, 396]);
    assert_eq!(model.record("h1"), full_record());
}

#[test]
fn floating_a_dock_while_the_window_retiles_keeps_the_sizes() {
    // P7: a tiling window manager halves the tile in the same turn. The
    // squeezed row is not the user's layout.
    let (mut model, _) = started(Some(&full_record()), "h1", &bottom(1600, 900, 200, &FULL));
    let mut floated = bottom(
        800,
        900,
        200,
        &[("scenes", 396), ("sources", 396), ("mixer", 400)],
    );
    floated.docks[2].floating = true;
    let floated = settle(&mut model, floated);
    // 597:595 of 792 px is within a pixel of what Qt shows: no resize.
    assert_eq!(lengths(&floated, Area::Bottom), vec![396, 396]);
    assert_eq!(model.record("h1"), full_record());
}

#[test]
fn a_dock_moved_to_another_area_gets_a_new_length() {
    let (mut model, _) = started(Some(&full_record()), "h1", &bottom(1600, 900, 200, &FULL));
    // Mixer goes to the left area alone; Scenes and Sources share the row.
    let mut moved = bottom(
        1600,
        900,
        200,
        &[("scenes", 797), ("sources", 795), ("mixer", 400)],
    );
    moved.docks[2].area = Some(Area::Left);
    moved.docks[2].rect = obs_dock_layout::Rect {
        x: 0,
        y: 0,
        width: 300,
        height: 696,
    };
    model.observe(&moved, Cause::Change);
    assert!(!model.sizes().length.contains_key("mixer"));
    // Back in the row, it is learned from the screen, in the row's scale.
    let back = bottom(
        1600,
        900,
        200,
        &[("scenes", 597), ("sources", 595), ("mixer", 396)],
    );
    let out = model.observe(&back, Cause::Change);
    assert_eq!(out.resizes, vec![]);
    assert_eq!(model.sizes().length["mixer"], 396);
}

#[test]
fn a_plugin_dock_created_after_the_restore_keeps_its_recorded_length() {
    // E14: the dock is in the record but not in the restored snapshot.
    let record = "v1;h1;aB=200;dmixer=396;dplugin=300;dscenes=597;dsources=595";
    let (mut model, _) = started(Some(record), "h1", &bottom(1600, 900, 200, &FULL));
    let added = bottom(
        1600,
        900,
        200,
        &[
            ("scenes", 450),
            ("sources", 450),
            ("mixer", 300),
            ("plugin", 384),
        ],
    );
    let out = model.observe(&added, Cause::Change);
    assert_eq!(
        lengths(&applied(&added, &out), Area::Bottom),
        vec![501, 499, 332, 252]
    );
    assert_eq!(model.record("h1"), record);
}

#[test]
fn a_tab_click_keeps_the_sizes() {
    let (mut model, _) = started(Some(&full_record()), "h1", &bottom(1600, 900, 200, &FULL));
    let mut tabbed = bottom(
        1600,
        900,
        200,
        &[
            ("scenes", 597),
            ("sources", 595),
            ("mixer", 396),
            ("audio", 396),
        ],
    );
    tabbed.docks[2].tabs = vec!["audio".into()];
    tabbed.docks[3].tabs = vec!["mixer".into()];
    tabbed.docks[3].rect = tabbed.docks[2].rect;
    tabbed.docks[3].shown = false;
    model.observe(&tabbed, Cause::Change);
    let mut clicked = tabbed.clone();
    clicked.docks[2].shown = false;
    clicked.docks[3].shown = true;
    let out = model.observe(&clicked, Cause::Change);
    assert_eq!(out.resizes, vec![]);
    assert_eq!(model.sizes().length["audio"], 396);
}
