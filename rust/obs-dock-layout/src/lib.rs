//! The dock layout model of the main window (obs-rust/obs-studio#5230).
//!
//! Rule: the saved layout changes only when the user changes it. A window
//! change (a resize, a maximize, a tiling window manager that retiles) never
//! changes what is saved and never undoes a change by the user.
//!
//! The frontend's Qt adapter gives this model a [`Snapshot`] of the docks
//! after each change, with its [`Cause`]. The model keeps the sizes the user
//! wants ([`Sizes`]: the depth of each dock area in pixels, and the length of
//! each dock in its row, applied as its share of the row, decision D1), and
//! returns the [`Resize`] calls that put them back where they fit. It has no
//! Qt and no platform code: the decisions run on every CI target (W1, W5).
//!
//! After the start, the adapter never restores an old layout again, with one
//! exception: a layout restored with no known sizes (the first start after
//! the upgrade, a layout written by upstream OBS, Reset Docks, a service
//! layout) is restored again at each window change until the window shows it
//! unclamped or the user changes something ([`Output::restore_again`]).
//! Saving: [`Model::saves_restored_blob`] and [`Model::record`].

#![deny(unsafe_code)]

use std::collections::BTreeMap;

#[allow(unsafe_code)]
pub mod ffi;
mod record;

pub use record::{Sizes, decode, encode};

/// A size within this many pixels of another is the same size: Qt rounds
/// when it lays out docks, and a resize for 1 px starts retile loops.
pub const TOLERANCE: i32 = 1;

/// Coordinates and sizes are clamped to +-`LIMIT` px, so no sum or product
/// the model forms can overflow, whatever the caller passes.
pub const LIMIT: i32 = 1 << 20;

fn sane(v: i32) -> i32 {
    v.clamp(-LIMIT, LIMIT)
}

/// A dock area of the main window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Area {
    Left,
    Right,
    Top,
    Bottom,
}

/// Width ([`Axis::Horizontal`]) or height ([`Axis::Vertical`]), as Qt's
/// `Qt::Orientation` in `QMainWindow::resizeDocks`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Area {
    /// The direction in which the docks of one row stand side by side: one
    /// above the other on the left and right, next to each other at the
    /// top and bottom.
    pub fn row_axis(self) -> Axis {
        match self {
            Self::Left | Self::Right => Axis::Vertical,
            Self::Top | Self::Bottom => Axis::Horizontal,
        }
    }

    /// The direction of the area's depth: the width of a side area, the
    /// height of a top or bottom one.
    pub fn depth_axis(self) -> Axis {
        match self.row_axis() {
            Axis::Horizontal => Axis::Vertical,
            Axis::Vertical => Axis::Horizontal,
        }
    }
}

/// A rectangle in the main window's coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// One dock as the adapter sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dock {
    /// The dock's object name: its key in the saved layout.
    pub name: String,
    /// Its dock area, `None` when Qt reports none.
    pub area: Option<Area>,
    pub floating: bool,
    /// On screen: not hidden, and the current tab of its tab group.
    pub shown: bool,
    pub rect: Rect,
    pub min_width: i32,
    pub min_height: i32,
    /// The other docks of its tab group, sorted.
    pub tabs: Vec<String>,
}

impl Dock {
    /// Its area when it takes part in the layout: shown and not floating.
    fn docked(&self) -> Option<Area> {
        if self.floating || !self.shown {
            None
        } else {
            self.area
        }
    }

    fn size(&self, axis: Axis) -> i32 {
        match axis {
            Axis::Horizontal => self.rect.width,
            Axis::Vertical => self.rect.height,
        }
    }

    fn pos(&self, axis: Axis) -> i32 {
        match axis {
            Axis::Horizontal => self.rect.x,
            Axis::Vertical => self.rect.y,
        }
    }

    fn min(&self, axis: Axis) -> i32 {
        match axis {
            Axis::Horizontal => self.min_width,
            Axis::Vertical => self.min_height,
        }
    }

    /// Pushed to its minimum along `axis`.
    fn at_min(&self, axis: Axis) -> bool {
        self.size(axis) <= self.min(axis) + TOLERANCE
    }

    /// Held at its minimum by the window along `axis`. Across its area's
    /// depth that needs the central widget at its minimum too: a column the
    /// user made as narrow as it goes, beside a preview with room, is the
    /// user's width, not a clamp.
    fn clamped(&self, snap: &Snapshot, axis: Axis) -> bool {
        self.at_min(axis)
            && !(self.docked().is_some_and(|area| area.depth_axis() == axis)
                && snap.room(axis) > TOLERANCE)
    }
}

/// The main window and its docks after a change.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    pub width: i32,
    pub height: i32,
    /// The window is on screen. A hidden or minimized window decides
    /// nothing (rule R9).
    pub visible: bool,
    /// The central widget (the preview): the room a dock area can grow
    /// into is its size above its minimum.
    pub central: Rect,
    pub central_min_width: i32,
    pub central_min_height: i32,
    pub docks: Vec<Dock>,
}

impl Snapshot {
    /// The snapshot with every number within +-[`LIMIT`].
    fn sane(&self) -> Self {
        let rect = |r: Rect| Rect {
            x: sane(r.x),
            y: sane(r.y),
            width: sane(r.width),
            height: sane(r.height),
        };
        Self {
            width: sane(self.width),
            height: sane(self.height),
            visible: self.visible,
            central: rect(self.central),
            central_min_width: sane(self.central_min_width),
            central_min_height: sane(self.central_min_height),
            docks: self
                .docks
                .iter()
                .map(|d| Dock {
                    rect: rect(d.rect),
                    min_width: sane(d.min_width),
                    min_height: sane(d.min_height),
                    ..d.clone()
                })
                .collect(),
        }
    }

    /// How much a dock area can grow along `axis` before the central widget
    /// reaches its minimum.
    fn room(&self, axis: Axis) -> i32 {
        match axis {
            Axis::Horizontal => self.central.width - self.central_min_width,
            Axis::Vertical => self.central.height - self.central_min_height,
        }
        .max(0)
    }
}

/// What caused a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// Anything that is not one of the others: a window change, a dock
    /// moved, floated, docked, tabbed, shown or hidden, a tab clicked.
    Change,
    /// The user dragged a separator (mouse press to release).
    SeparatorDrag,
    /// The adapter restored a layout (after [`Model::start`] or
    /// [`Output::restore_again`]).
    Restored,
}

/// One `QMainWindow::resizeDocks(docks, sizes, axis)` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resize {
    pub axis: Axis,
    pub docks: Vec<String>,
    pub sizes: Vec<i32>,
}

/// What the adapter does after a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Output {
    /// Restore the layout given to [`Model::start`] again, then send a
    /// [`Cause::Restored`] snapshot. No resizes come with it.
    pub restore_again: bool,
    pub resizes: Vec<Resize>,
}

/// Docks of one area side by side along the area's row axis.
struct Row<'a> {
    area: Area,
    docks: Vec<&'a Dock>,
}

/// The docks of one area whose depth is the area's: all start at the same
/// edge and have the same depth.
struct AreaDocks<'a> {
    area: Area,
    docks: Vec<&'a Dock>,
}

const AREAS: [Area; 4] = [Area::Left, Area::Right, Area::Top, Area::Bottom];

/// The rows of a snapshot with two docks or more.
fn rows(snap: &Snapshot) -> Vec<Row<'_>> {
    let mut by_edge: BTreeMap<(Area, i32), Vec<&Dock>> = BTreeMap::new();
    for dock in &snap.docks {
        if let Some(area) = dock.docked() {
            by_edge
                .entry((area, dock.pos(area.depth_axis())))
                .or_default()
                .push(dock);
        }
    }
    by_edge
        .into_iter()
        .filter(|(_, docks)| docks.len() > 1)
        .map(|((area, _), mut docks)| {
            docks.sort_by_key(|d| d.pos(area.row_axis()));
            Row { area, docks }
        })
        .collect()
}

/// The areas whose docks all share one depth.
fn areas(snap: &Snapshot) -> Vec<AreaDocks<'_>> {
    AREAS
        .iter()
        .filter_map(|&area| {
            let docks: Vec<&Dock> = snap
                .docks
                .iter()
                .filter(|d| d.docked() == Some(area))
                .collect();
            let first = docks.first()?;
            let axis = area.depth_axis();
            docks
                .iter()
                .all(|d| d.pos(axis) == first.pos(axis) && d.size(axis) == first.size(axis))
                .then_some(AreaDocks { area, docks })
        })
        .collect()
}

/// Where each dock is, without sizes: what only a user change (or Reset
/// Docks, or a plugin) changes. With `floating_geometry`, a floating dock's
/// rectangle counts too.
fn placement(snap: &Snapshot, floating_geometry: bool) -> Vec<String> {
    let mut out: Vec<String> = snap
        .docks
        .iter()
        .map(|dock| {
            let order = dock.docked().map_or(-1, |area| {
                let mut same: Vec<&Dock> = snap
                    .docks
                    .iter()
                    .filter(|d| d.docked() == Some(area))
                    .collect();
                same.sort_by(|a, b| {
                    (a.rect.y, a.rect.x, &a.name).cmp(&(b.rect.y, b.rect.x, &b.name))
                });
                same.iter()
                    .position(|d| d.name == dock.name)
                    .map_or(-1, |i| i as i64)
            });
            let geometry = if floating_geometry && dock.floating {
                format!("{:?}", dock.rect)
            } else {
                String::new()
            };
            format!(
                "{} {:?} floating={} shown={} order={order} tabs={} {geometry}",
                dock.name,
                dock.area,
                dock.floating,
                dock.shown,
                dock.tabs.join(",")
            )
        })
        .collect();
    out.sort();
    out
}

/// Rounds `value * num / den` to the nearest integer, with `den > 0`.
fn scale(value: i32, num: i64, den: i64) -> i32 {
    let v = i64::from(value) * num;
    i32::try_from((2 * v + den) / (2 * den)).unwrap_or(i32::MAX)
}

/// The decisions for one main window.
#[derive(Debug, Default)]
pub struct Model {
    sizes: Sizes,
    /// [`Model::start`] got a record for its blob.
    record_ok: bool,
    /// Restore the blob again at each window change: its sizes are not
    /// known yet, and the user has changed nothing.
    pending: bool,
    last: Option<Snapshot>,
    /// The area each dock was last docked in.
    home: BTreeMap<String, Area>,
}

impl Model {
    pub fn new() -> Self {
        Self::default()
    }

    /// The adapter is about to restore a layout blob whose SHA-256 is
    /// `blob_hash`: at the start (with the saved `DockSizes` record), at
    /// Reset Docks and for a service layout (no record). Whether the record
    /// belongs to this blob and is used.
    pub fn start(&mut self, record: Option<&str>, blob_hash: &str) -> bool {
        self.last = None;
        self.home.clear();
        match record.and_then(decode) {
            Some((hash, sizes)) if hash == blob_hash => {
                self.sizes = sizes;
                self.record_ok = true;
                self.pending = false;
            }
            _ => {
                self.sizes = Sizes::default();
                self.record_ok = false;
                self.pending = true;
            }
        }
        self.record_ok
    }

    /// The window after a change.
    pub fn observe(&mut self, snap: &Snapshot, cause: Cause) -> Output {
        let snap = &snap.sane();
        if !snap.visible {
            return Output::default();
        }
        let prev = self.last.replace(snap.clone());
        let out = match (cause, prev) {
            (Cause::Restored, _) | (_, None) => self.restored(snap),
            (Cause::SeparatorDrag, Some(prev))
                if (prev.width, prev.height) == (snap.width, snap.height) =>
            {
                // A press and release that resized nothing is no change.
                if self.dragged(&prev, snap) {
                    self.pending = false;
                }
                Output::default()
            }
            (_, Some(prev)) => self.changed(&prev, snap),
        };
        for dock in &snap.docks {
            if let Some(area) = dock.docked() {
                self.home.insert(dock.name.clone(), area);
            }
        }
        out
    }

    /// Quit (or a save) writes the blob given to [`Model::start`], not the
    /// live layout: the user changed nothing, and the window may show that
    /// blob squeezed into a small tile.
    pub fn saves_restored_blob(&self) -> bool {
        self.pending
    }

    /// The `DockSizes` record for the blob saved with it.
    pub fn record(&self, blob_hash: &str) -> String {
        encode(blob_hash, &self.sizes)
    }

    /// The sizes the user wants.
    pub fn sizes(&self) -> &Sizes {
        &self.sizes
    }

    fn restored(&mut self, snap: &Snapshot) -> Output {
        self.pending = !self.record_ok || self.missing(snap);
        if self.pending {
            return self.learn_if_unclamped(snap);
        }
        self.learn(snap);
        self.apply(snap)
    }

    fn changed(&mut self, prev: &Snapshot, snap: &Snapshot) -> Output {
        if self.pending {
            if placement(prev, true) != placement(snap, true) {
                self.pending = false;
            } else if (prev.width, prev.height) != (snap.width, snap.height) {
                return Output {
                    restore_again: true,
                    resizes: Vec::new(),
                };
            } else {
                return self.learn_if_unclamped(snap);
            }
        }
        for dock in &snap.docks {
            let before = prev.docks.iter().find(|d| d.name == dock.name);
            let moved = dock
                .docked()
                .zip(self.home.get(&dock.name))
                .is_some_and(|(now, home)| now != *home);
            if moved || before.is_some_and(|b| b.tabs != dock.tabs) {
                self.sizes.length.remove(&dock.name);
            }
        }
        self.learn(snap);
        self.apply(snap)
    }

    /// A docked dock in a row, or an area, with no known size.
    fn missing(&self, snap: &Snapshot) -> bool {
        rows(snap).iter().any(|row| {
            row.docks
                .iter()
                .any(|d| !self.sizes.length.contains_key(&d.name))
        }) || areas(snap)
            .iter()
            .any(|a| !self.sizes.depth.contains_key(&a.area))
    }

    fn learn_if_unclamped(&mut self, snap: &Snapshot) -> Output {
        let clamped = snap.docks.iter().any(|d| {
            d.docked().is_some()
                && (d.clamped(snap, Axis::Horizontal) || d.clamped(snap, Axis::Vertical))
        });
        if clamped {
            return Output::default();
        }
        self.pending = false;
        self.learn(snap);
        self.apply(snap)
    }

    /// Takes the sizes of docks and areas with none yet from the screen,
    /// where they are not clamped. A dock that joins a row is rescaled to
    /// the row's known sizes.
    fn learn(&mut self, snap: &Snapshot) {
        for row in rows(snap) {
            let axis = row.area.row_axis();
            let (known, unknown): (Vec<&Dock>, Vec<&Dock>) = row
                .docks
                .iter()
                .partition(|d| self.sizes.length.contains_key(&d.name));
            if unknown.is_empty() || row.docks.iter().any(|d| d.at_min(axis)) {
                continue;
            }
            let wanted: i64 = known
                .iter()
                .map(|d| i64::from(self.sizes.length[&d.name]))
                .sum();
            let shown: i64 = known.iter().map(|d| i64::from(d.size(axis))).sum();
            for dock in unknown {
                let px = if shown > 0 {
                    scale(dock.size(axis), wanted, shown).max(1)
                } else {
                    dock.size(axis)
                };
                self.sizes.length.insert(dock.name.clone(), px);
            }
        }
        for area in areas(snap) {
            let axis = area.area.depth_axis();
            if !self.sizes.depth.contains_key(&area.area)
                && !area.docks.iter().any(|d| d.clamped(snap, axis))
            {
                self.sizes.depth.insert(area.area, area.docks[0].size(axis));
            }
        }
    }

    /// A separator drag: the docks and areas it resized get new sizes, the
    /// others keep theirs (decision D3). A dock dragged to its minimum stays
    /// there (R7). In a clamped row, the new sizes are rescaled to the
    /// row's wanted total, so the untouched docks keep their shares.
    /// Whether the drag resized a dock or an area.
    fn dragged(&mut self, prev: &Snapshot, snap: &Snapshot) -> bool {
        let mut changed = false;
        for row in rows(snap) {
            let axis = row.area.row_axis();
            let before: Option<Vec<i32>> = row
                .docks
                .iter()
                .map(|d| {
                    prev.docks
                        .iter()
                        .find(|p| p.name == d.name && p.docked() == d.docked())
                        .map(|p| p.size(axis))
                })
                .collect();
            let Some(before) = before else {
                for d in &row.docks {
                    self.sizes.length.insert(d.name.clone(), d.size(axis));
                }
                changed = true;
                continue;
            };
            let touched: Vec<bool> = row
                .docks
                .iter()
                .zip(&before)
                .map(|(d, b)| (d.size(axis) - b).abs() > TOLERANCE)
                .collect();
            if !touched.contains(&true) {
                continue;
            }
            changed = true;
            self.drag_row(&row, axis, &touched);
        }
        for area in areas(snap) {
            let axis = area.area.depth_axis();
            let first = area.docks[0];
            let before = prev
                .docks
                .iter()
                .find(|p| p.name == first.name && p.docked() == first.docked())
                .map(|p| p.size(axis));
            if before.is_none_or(|b| (first.size(axis) - b).abs() > TOLERANCE) {
                self.sizes.depth.insert(area.area, first.size(axis));
                changed = true;
            }
        }
        changed
    }

    fn drag_row(&mut self, row: &Row<'_>, axis: Axis, touched: &[bool]) {
        let wanted: Option<Vec<i32>> = row
            .docks
            .iter()
            .map(|d| self.sizes.length.get(&d.name).copied())
            .collect();
        let rescaled = wanted.and_then(|wanted| {
            let total: i64 = wanted.iter().map(|&w| i64::from(w)).sum();
            let kept: i64 = wanted
                .iter()
                .zip(touched)
                .filter(|(_, t)| !**t)
                .map(|(&w, _)| i64::from(w))
                .sum();
            let shown: i64 = row
                .docks
                .iter()
                .zip(touched)
                .filter(|(_, t)| **t)
                .map(|(d, _)| i64::from(d.size(axis)))
                .sum();
            (total > kept && shown > 0).then_some((total - kept, shown))
        });
        for (dock, &t) in row.docks.iter().zip(touched) {
            match rescaled {
                Some((room, shown)) if t => {
                    let px = scale(dock.size(axis), room, shown).max(1);
                    self.sizes.length.insert(dock.name.clone(), px);
                }
                Some(_) => {}
                None => {
                    self.sizes.length.insert(dock.name.clone(), dock.size(axis));
                }
            }
        }
    }

    /// The resizes that put the wanted sizes back where they fit.
    fn apply(&self, snap: &Snapshot) -> Output {
        let mut resizes = Vec::new();
        for area in areas(snap) {
            let Some(&depth) = self.sizes.depth.get(&area.area) else {
                continue;
            };
            let axis = area.area.depth_axis();
            let first = area.docks[0];
            let min = area.docks.iter().map(|d| d.min(axis)).max().unwrap_or(0);
            let now = first.size(axis);
            // Grow only into the room the preview has; a smaller window
            // keeps the area clamped until it grows again.
            let target = depth.min(now + snap.room(axis)).max(min);
            if (target - now).abs() > TOLERANCE {
                resizes.push(Resize {
                    axis,
                    docks: vec![first.name.clone()],
                    sizes: vec![target],
                });
            }
        }
        for row in rows(snap) {
            if let Some(resize) = self.apply_row(&row) {
                resizes.push(resize);
            }
        }
        Output {
            restore_again: false,
            resizes,
        }
    }

    /// The row's wanted lengths as shares of its length on screen.
    fn apply_row(&self, row: &Row<'_>) -> Option<Resize> {
        let axis = row.area.row_axis();
        let wanted: Vec<i32> = row
            .docks
            .iter()
            .map(|d| self.sizes.length.get(&d.name).copied())
            .collect::<Option<_>>()?;
        let total: i64 = row.docks.iter().map(|d| i64::from(d.size(axis))).sum();
        let sum: i64 = wanted.iter().map(|&w| i64::from(w)).sum();
        if total <= 0 || sum <= 0 {
            return None;
        }
        let mut sizes = Vec::with_capacity(wanted.len());
        let (mut acc, mut placed) = (0_i64, 0_i32);
        for &w in &wanted {
            acc += i64::from(w);
            let end = scale(i32::try_from(total).ok()?, acc, sum);
            sizes.push(end - placed);
            placed = end;
        }
        let fits = row.docks.iter().zip(&sizes).all(|(d, &s)| s >= d.min(axis));
        let differs = row
            .docks
            .iter()
            .zip(&sizes)
            .any(|(d, &s)| (s - d.size(axis)).abs() > TOLERANCE);
        (fits && differs).then(|| Resize {
            axis,
            docks: row.docks.iter().map(|d| d.name.clone()).collect(),
            sizes,
        })
    }
}
