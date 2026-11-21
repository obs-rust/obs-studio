//! Layouts as the frontend's adapter reports them: a bottom row of docks
//! (the default OBS layout) and a left column, with Qt's 4 px separators.
//! The numbers come from `test_dock_state_restart` (#5359) where it
//! printed them: a 1600 px wide window with Scenes, Sources and Mixer
//! saved at 597/595/396 px.

use obs_dock_layout::{Area, Axis, Cause, Dock, Model, Output, Rect, Snapshot};

pub const SEP: i32 = 4;
pub const MIN_W: i32 = 150;
pub const MIN_H: i32 = 100;
/// The preview's minimum size.
pub const CENTRAL_MIN: i32 = 100;

/// The bottom row of the default layout at full size.
pub const FULL: [(&str, i32); 3] = [("scenes", 597), ("sources", 595), ("mixer", 396)];

pub fn dock(name: &str, area: Area, rect: Rect) -> Dock {
    Dock {
        name: name.to_owned(),
        area: Some(area),
        floating: false,
        shown: true,
        rect,
        min_width: MIN_W,
        min_height: MIN_H,
        tabs: Vec::new(),
    }
}

/// A window `width` x `height` with a bottom row `depth` px high.
pub fn bottom(width: i32, height: i32, depth: i32, lengths: &[(&str, i32)]) -> Snapshot {
    let mut x = 0;
    let docks = lengths
        .iter()
        .map(|&(name, w)| {
            let d = dock(
                name,
                Area::Bottom,
                Rect {
                    x,
                    y: height - depth,
                    width: w,
                    height: depth,
                },
            );
            x += w + SEP;
            d
        })
        .collect();
    let mut snap = Snapshot {
        width,
        height,
        visible: true,
        central: Rect::default(),
        central_min_width: CENTRAL_MIN,
        central_min_height: CENTRAL_MIN,
        docks,
    };
    relayout(&mut snap);
    snap
}

/// A window with a left column `depth` px wide, its docks stacked.
pub fn left(width: i32, height: i32, depth: i32, lengths: &[(&str, i32)]) -> Snapshot {
    let mut y = 0;
    let docks = lengths
        .iter()
        .map(|&(name, h)| {
            let d = dock(
                name,
                Area::Left,
                Rect {
                    x: 0,
                    y,
                    width: depth,
                    height: h,
                },
            );
            y += h + SEP;
            d
        })
        .collect();
    let mut snap = Snapshot {
        width,
        height,
        visible: true,
        central: Rect::default(),
        central_min_width: CENTRAL_MIN,
        central_min_height: CENTRAL_MIN,
        docks,
    };
    relayout(&mut snap);
    snap
}

/// The lengths of the shown docks of one area, in order.
pub fn lengths(snap: &Snapshot, area: Area) -> Vec<i32> {
    let axis = area.row_axis();
    let mut docks: Vec<&Dock> = snap
        .docks
        .iter()
        .filter(|d| d.area == Some(area) && d.shown && !d.floating)
        .collect();
    docks.sort_by_key(|d| match axis {
        Axis::Horizontal => d.rect.x,
        Axis::Vertical => d.rect.y,
    });
    docks
        .iter()
        .map(|d| match axis {
            Axis::Horizontal => d.rect.width,
            Axis::Vertical => d.rect.height,
        })
        .collect()
}

/// What Qt shows after the adapter applied `out`: the sizes set, the docks
/// of each area laid out again from the area's edge.
pub fn applied(snap: &Snapshot, out: &Output) -> Snapshot {
    let mut next = snap.clone();
    for resize in &out.resizes {
        for (name, &size) in resize.docks.iter().zip(&resize.sizes) {
            let d = next.docks.iter_mut().find(|d| &d.name == name).unwrap();
            match resize.axis {
                Axis::Horizontal => d.rect.width = size,
                Axis::Vertical => d.rect.height = size,
            }
        }
    }
    relayout(&mut next);
    next
}

/// Lays the shown docks of the bottom and left areas out again: in order,
/// from the area's edge, each area as deep as its first dock.
pub fn relayout(snap: &mut Snapshot) {
    let height = snap.height;
    let mut central = Rect {
        x: 0,
        y: 0,
        width: snap.width,
        height: snap.height,
    };
    for area in [Area::Bottom, Area::Left] {
        let axis = area.row_axis();
        let mut docks: Vec<&mut Dock> = snap
            .docks
            .iter_mut()
            .filter(|d| d.area == Some(area) && d.shown && !d.floating)
            .collect();
        docks.sort_by_key(|d| match axis {
            Axis::Horizontal => d.rect.x,
            Axis::Vertical => d.rect.y,
        });
        let Some(depth) = docks.first().map(|d| match area {
            Area::Bottom => d.rect.height,
            _ => d.rect.width,
        }) else {
            continue;
        };
        // The preview fills what the dock areas leave.
        match area {
            Area::Bottom => central.height -= depth + SEP,
            _ => {
                central.x += depth + SEP;
                central.width -= depth + SEP;
            }
        }
        let mut at = 0;
        for d in docks {
            match area {
                Area::Bottom => {
                    d.rect.x = at;
                    d.rect.height = depth;
                    d.rect.y = height - depth;
                    at += d.rect.width + SEP;
                }
                _ => {
                    d.rect.y = at;
                    d.rect.width = depth;
                    d.rect.x = 0;
                    at += d.rect.height + SEP;
                }
            }
        }
    }
    snap.central = central;
}

/// A model started from `record` for blob `hash`, with `snap` restored.
pub fn started(record: Option<&str>, hash: &str, snap: &Snapshot) -> (Model, Output) {
    let mut model = Model::new();
    model.start(record, hash);
    let out = model.observe(snap, Cause::Restored);
    (model, out)
}

/// The record of the full-size bottom row for blob `h1`.
pub fn full_record() -> String {
    "v1;h1;aB=200;dmixer=396;dscenes=597;dsources=595".to_owned()
}

/// Window changes and resizes until the model has nothing more to apply.
pub fn settle(model: &mut Model, snap: Snapshot) -> Snapshot {
    let mut snap = snap;
    for _ in 0..4 {
        let out = model.observe(&snap, Cause::Change);
        assert!(!out.restore_again);
        if out.resizes.is_empty() {
            return snap;
        }
        snap = applied(&snap, &out);
    }
    panic!("the model did not settle: {snap:?}");
}
