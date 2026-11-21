//! W2: the same user changes under a floating and under a tiling window
//! manager save the same record. The tiling profile adds window changes:
//! the start in a small tile and two retiles (#3740), a retile when a dock
//! floats (#5230), and another window that opens and closes.

use obs_dock_layout::{Area, Cause, Model, Rect, Snapshot};

use crate::dock_layout::fixture::{
    MIN_W, applied, bottom, dock, full_record, lengths, relayout, settle,
};

/// The window resized to `width` as Qt shows it: the docks in the bottom
/// row keep their proportions while they fit, and stop at their minimum.
fn resized(now: &Snapshot, width: i32) -> Snapshot {
    let row: Vec<(String, i32)> = now
        .docks
        .iter()
        .filter(|d| d.area == Some(Area::Bottom) && d.shown && !d.floating)
        .map(|d| (d.name.clone(), d.rect.width))
        .collect();
    let seps = 4 * (row.len() as i32 - 1);
    let sum: i32 = row.iter().map(|r| r.1).sum();
    let room = width - seps;
    let scaled: Vec<(&str, i32)> = row
        .iter()
        .map(|(n, w)| (n.as_str(), (w * room / sum).max(MIN_W)))
        .collect();
    let mut next = bottom(width, now.height, 200, &scaled);
    next.docks
        .extend(now.docks.iter().filter(|d| d.floating).cloned());
    next
}

/// A window change, settled.
fn window(model: &mut Model, now: &Snapshot, width: i32) -> Snapshot {
    settle(model, resized(now, width))
}

/// The session: a drag of the Scenes/Sources separator, Mixer floated,
/// quit. `tiling` adds the window changes of a tiling window manager.
fn session(tiling: bool) -> String {
    let mut model = Model::new();
    model.start(Some(&full_record()), "h1");
    let full = bottom(
        1600,
        900,
        200,
        &[("scenes", 597), ("sources", 595), ("mixer", 396)],
    );
    let mut now = if tiling { resized(&full, 1000) } else { full };
    let out = model.observe(&now, Cause::Restored);
    now = applied(&now, &out);
    if tiling {
        now = window(&mut model, &now, 1200);
        now = window(&mut model, &now, 1600);
    }
    let back = lengths(&now, Area::Bottom);
    assert!(
        back.iter()
            .zip([597, 595, 396])
            .all(|(a, b)| (a - b).abs() <= 1),
        "{back:?}"
    );
    // Drag: Scenes takes 200 px from Sources.
    let mut dragged = now.clone();
    dragged.docks[0].rect.width += 200;
    dragged.docks[1].rect.width -= 200;
    relayout(&mut dragged);
    model.observe(&dragged, Cause::SeparatorDrag);
    now = dragged;
    if tiling {
        now = window(&mut model, &now, 800);
        now = window(&mut model, &now, 1600);
    }
    // Float Mixer; a tiling window manager halves the tile at once.
    let rect = Rect {
        x: 100,
        y: 100,
        width: 396,
        height: 200,
    };
    let mut floated = now.clone();
    floated.docks.retain(|d| d.name != "mixer");
    floated.docks.push(obs_dock_layout::Dock {
        floating: true,
        ..dock("mixer", Area::Bottom, rect)
    });
    relayout(&mut floated);
    let width = if tiling { 800 } else { 1600 };
    now = window(&mut model, &floated, width);
    if tiling {
        let _ = window(&mut model, &now, 1600);
    }
    model.record("h1")
}

#[test]
fn both_profiles_save_the_same_record() {
    let floating = session(false);
    assert_eq!(floating, "v1;h1;aB=200;dmixer=396;dscenes=797;dsources=395");
    assert_eq!(session(true), floating);
}
