//! Safe core for `libobs/util/profiler.c`.
//!
//! The C ABI shim lives in [`crate::ffi::profiler`]. Names are keyed by
//! pointer identity, as in C. `profile_end` pulls a timestamp from the
//! caller's clock once on entry (and again for each mismatched frame it
//! closes), which is where `profiler.c` calls `os_gettime_ns`.
//!
//! The two `LOG_ERROR` lines `profiler.c` prints are reported through the
//! `log` callback of [`profile_end`] instead of calling `blog` from here; the
//! shim turns them into the C calls (the same split as [`crate::pipe`] uses
//! for a failed `CreateProcessW`).

use core::cell::{Cell, RefCell};
use core::ffi::{CStr, c_char};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// A profile name. Equality is the pointer address, not the bytes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct ProfileName(usize);

impl ProfileName {
    /// The address of a NUL-terminated string that lives for the whole
    /// process. The profiler reads names back as C strings.
    pub fn from_static(name: &'static CStr) -> Self {
        Self(name.as_ptr() as usize)
    }

    /// # Safety
    ///
    /// `name` must stay valid for as long as the profiler stores it, which
    /// is what `profile_start` requires of its caller.
    pub fn new(name: *const c_char) -> Self {
        Self(name as usize)
    }

    pub fn as_ptr(self) -> *const c_char {
        self.0 as *const c_char
    }

    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

/// One `(time_delta, count)` bucket from a snapshot.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimeEntry {
    pub time_delta: u64,
    pub count: u64,
}

/// One node of a snapshot, in registration / first-seen order.
#[derive(Clone, Debug, Default)]
pub struct SnapshotEntry {
    pub name: ProfileName,
    pub times: Vec<TimeEntry>,
    pub min_time: u64,
    pub max_time: u64,
    pub overall_count: u64,
    pub times_between_calls: Vec<TimeEntry>,
    pub expected_time_between_calls: u64,
    pub min_time_between_calls: u64,
    pub max_time_between_calls: u64,
    pub overall_between_calls_count: u64,
    pub children: Vec<SnapshotEntry>,
}

/// Roots of a profiler snapshot.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub roots: Vec<SnapshotEntry>,
}

/// What `profile_end` hands to its log callback.
///
/// `profiler.c` prints both through `blog` at `LOG_ERROR`; the shim owns that
/// call, this only says what happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileLog {
    /// No active profile on this thread.
    NoActive,
    /// The frame being ended has a different name.
    Mismatch {
        started: ProfileName,
        ended: ProfileName,
    },
}

/// Owned copies of formatted profile names.
///
/// Each store returns a distinct pointer, including for equal bytes. The
/// pointers stay valid until the store is dropped.
#[derive(Default)]
pub struct NameStore {
    names: Mutex<Vec<Vec<u8>>>,
}

impl NameStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies `text` and returns the stored C string.
    pub fn store(&self, text: &str) -> *const c_char {
        let mut bytes = Vec::with_capacity(text.len() + 1);
        bytes.extend_from_slice(text.as_bytes());
        bytes.push(0);
        self.keep(bytes)
    }

    /// Adopts the bytes of a NUL-terminated C string, NUL included.
    ///
    /// The shim reads the `bmalloc` buffer that `profiler_name_store_push`
    /// takes ownership of and frees it; an empty slice is the null pointer, as
    /// in C.
    pub fn adopt_bytes(&self, bytes: Vec<u8>) -> *const c_char {
        self.keep(bytes)
    }

    fn keep(&self, bytes: Vec<u8>) -> *const c_char {
        let ptr = if bytes.is_empty() {
            core::ptr::null()
        } else {
            bytes.as_ptr().cast()
        };
        self.names.lock().unwrap().push(bytes);
        ptr
    }
}

/// Boxes a store for the C ABI.
///
/// The shim drops the box again (`profiler_name_store_free`): the core never
/// handles the raw pointer.
pub fn name_store_create() -> *mut NameStore {
    Box::into_raw(Box::new(NameStore::new()))
}

#[derive(Clone, Copy, Default)]
struct Slot {
    probes: usize,
    time_delta: u64,
    count: u64,
}

/// Open-addressed map from `profiler.c` (`add_hashmap_entry` and its grow).
struct TimesTable {
    size: usize,
    occupied: usize,
    entries: Vec<Slot>,
    old_start_index: usize,
    old_occupied: usize,
    old_entries: Option<Vec<Slot>>,
}

impl TimesTable {
    fn new(size: usize) -> Self {
        Self {
            size,
            occupied: 0,
            entries: vec![Slot::default(); size],
            old_start_index: 0,
            old_occupied: 0,
            old_entries: None,
        }
    }

    fn add(&mut self, mut usec: u64, mut count: u64) {
        let mut probes = 1usize;
        let mut start = (usec % self.size as u64) as usize;
        loop {
            let idx = (start + probes) % self.size;
            if self.entries[idx].probes == 0 {
                self.entries[idx] = Slot {
                    probes,
                    time_delta: usec,
                    count,
                };
                self.occupied += 1;
                return;
            }
            if self.entries[idx].time_delta == usec {
                self.entries[idx].count += count;
                return;
            }
            if self.entries[idx].probes >= probes {
                probes += 1;
                continue;
            }
            if (self.occupied as f64) / (self.size as f64) > 0.7 {
                self.grow(usec, count);
                return;
            }
            let old = self.entries[idx];
            self.entries[idx] = Slot {
                probes,
                time_delta: usec,
                count,
            };
            probes = old.probes;
            count = old.count;
            usec = old.time_delta;
            start = (usec % self.size as u64) as usize;
            probes += 1;
        }
    }

    /// Moves at most eight old entries when `limit_items` is set, otherwise
    /// all of them. An empty leftover is freed on the next call, as in C.
    fn migrate(&mut self, limit_items: bool) {
        if self.old_entries.is_none() {
            return;
        }
        if self.old_occupied == 0 {
            self.old_entries = None;
            return;
        }
        let mut moved = 0usize;
        loop {
            if limit_items && moved >= 8 {
                return;
            }
            if self.old_occupied == 0 {
                return;
            }
            let idx = self.old_start_index;
            let Some(slot) = self.old_entries.as_ref().and_then(|e| e.get(idx).copied()) else {
                return;
            };
            self.old_start_index += 1;
            moved += 1;
            if slot.probes == 0 {
                continue;
            }
            self.old_occupied -= 1;
            self.add(slot.time_delta, slot.count);
        }
    }

    fn grow(&mut self, usec: u64, count: u64) {
        self.migrate(false);
        let old_occupied = self.occupied;
        let old_entries = core::mem::take(&mut self.entries);
        let doubled = self.size.wrapping_mul(2);
        let new_size = if doubled < 16 { 16 } else { doubled };
        *self = Self::new(new_size);
        self.old_occupied = old_occupied;
        self.old_entries = Some(old_entries);
        self.add(usec, count);
    }

    fn copy_times(&mut self) -> (Vec<TimeEntry>, u64, u64, u64) {
        self.migrate(false);
        let mut min = u64::MAX;
        let mut max = 0u64;
        let mut calls = 0u64;
        let mut items = Vec::new();
        for slot in &self.entries {
            if slot.probes == 0 {
                continue;
            }
            items.push(TimeEntry {
                time_delta: slot.time_delta,
                count: slot.count,
            });
            calls += slot.count;
            if min >= slot.time_delta {
                min = slot.time_delta;
            }
            if max <= slot.time_delta {
                max = slot.time_delta;
            }
        }
        // `profiler_time_entry_compare` sorts `time_delta` descending. Keys
        // are unique, so the order does not depend on `qsort` being unstable.
        items.sort_by_key(|entry| core::cmp::Reverse(entry.time_delta));
        (items, min, max, calls)
    }
}

struct ProfileEntry {
    name: ProfileName,
    times: TimesTable,
    expected: u64,
    between: TimesTable,
    children: Vec<ProfileEntry>,
}

impl ProfileEntry {
    fn new(name: ProfileName) -> Self {
        Self {
            name,
            times: TimesTable::new(1),
            expected: 0,
            between: TimesTable::new(1),
            children: Vec::new(),
        }
    }

    fn child_index(&mut self, name: ProfileName) -> usize {
        if let Some(i) = self.children.iter().position(|c| c.name == name) {
            return i;
        }
        self.children.push(Self::new(name));
        self.children.len() - 1
    }

    fn snapshot_of(&mut self) -> SnapshotEntry {
        let (times, min_time, max_time, overall_count) = self.times.copy_times();
        let mut out = SnapshotEntry {
            name: self.name,
            times,
            min_time,
            max_time,
            overall_count,
            expected_time_between_calls: self.expected,
            ..SnapshotEntry::default()
        };
        if self.expected != 0 {
            let (between, min_b, max_b, overall_b) = self.between.copy_times();
            out.times_between_calls = between;
            out.min_time_between_calls = min_b;
            out.max_time_between_calls = max_b;
            out.overall_between_calls_count = overall_b;
        }
        out.children = self
            .children
            .iter_mut()
            .map(ProfileEntry::snapshot_of)
            .collect();
        out
    }
}

/// One call in the thread's tree. Children are a first-child/next-sibling
/// list in call order, so a frame allocates nothing (#3213).
struct Node {
    name: ProfileName,
    start: u64,
    end: u64,
    parent: Option<usize>,
    first_child: Option<usize>,
    last_child: Option<usize>,
    next_sibling: Option<usize>,
}

struct Active {
    nodes: Vec<Node>,
}

impl Active {
    fn root_start(&self) -> u64 {
        self.nodes.first().map(|n| n.start).unwrap_or(0)
    }
}

struct Root {
    name: ProfileName,
    entry: Mutex<ProfileEntry>,
    /// When the previous call of this root started.
    prev_start: Mutex<Option<u64>>,
}

struct Globals {
    enabled: bool,
    roots: Vec<Arc<Root>>,
}

/// `Globals::enabled`, readable without the lock. Written under the lock.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Bumped whenever recorded roots are dropped, so a thread's cached root
/// is not reused after [`profiler_free`].
static ROOTS_GENERATION: AtomicU64 = AtomicU64::new(0);

static GLOBAL: Mutex<Globals> = Mutex::new(Globals {
    enabled: false,
    roots: Vec::new(),
});

thread_local! {
    static THREAD_ENABLED: Cell<bool> = const { Cell::new(true) };
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
    static STACK: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    /// The node buffer of the last finished root, cleared, for the next one.
    static SPARE: RefCell<Vec<Node>> = const { RefCell::new(Vec::new()) };
    /// The root this thread merged into last, with the generation it was
    /// looked up in.
    static LAST_ROOT: RefCell<Option<(ProfileName, u64, Arc<Root>)>> = const { RefCell::new(None) };
}

pub fn thread_enabled() -> bool {
    THREAD_ENABLED.with(Cell::get)
}

fn set_thread_enabled(enabled: bool) {
    THREAD_ENABLED.with(|c| c.set(enabled));
}

fn diff_ns_to_usec(prev: u64, next: u64) -> u64 {
    next.wrapping_sub(prev).wrapping_add(500) / 1000
}

fn lock_globals() -> MutexGuard<'static, Globals> {
    GLOBAL.lock().unwrap()
}

fn get_root(globals: &mut Globals, name: ProfileName) -> Arc<Root> {
    if let Some(root) = globals.roots.iter().find(|r| r.name == name) {
        return Arc::clone(root);
    }
    let root = Arc::new(Root {
        name,
        entry: Mutex::new(ProfileEntry::new(name)),
        prev_start: Mutex::new(None),
    });
    globals.roots.push(Arc::clone(&root));
    root
}

/// Enables recording. The root mutex stays for the process; see
/// [`profiler_free`].
pub fn profiler_start() {
    let mut globals = lock_globals();
    globals.enabled = true;
    ENABLED.store(true, Ordering::Release);
}

pub fn profiler_stop() {
    let mut globals = lock_globals();
    globals.enabled = false;
    ENABLED.store(false, Ordering::Release);
}

/// Drops recorded roots and disables the profiler.
///
/// The C root mutex is destroyed here. This port keeps the process-lifetime
/// mutex, so a later [`profiler_start`] is defined instead of locking a
/// destroyed mutex.
pub fn profiler_free() {
    let mut globals = lock_globals();
    globals.enabled = false;
    ENABLED.store(false, Ordering::Release);
    globals.roots.clear();
    ROOTS_GENERATION.fetch_add(1, Ordering::AcqRel);
}

struct TestClock {
    times: Vec<u64>,
    pos: usize,
}

/// Timestamps the C profiler shims read when a test arms them.
/// `None` means the real `os_gettime_ns`.
static TEST_CLOCK: Mutex<Option<TestClock>> = Mutex::new(None);

/// `true` only while [`TEST_CLOCK`] is `Some`. Production loads this
/// relaxed and does not take the mutex.
static TEST_CLOCK_ARMED: AtomicBool = AtomicBool::new(false);

/// Arms the clock the profiler shims read.
///
/// `None` restores `os_gettime_ns`. A slice is copied and read from the
/// start. Past the end the shim returns 0, matching the oracle clock.
/// This is not a C export. Cargo integration tests link this crate as a
/// library, so the hook cannot live behind `#[cfg(test)]`.
pub fn testing_set_clock(times: Option<&[u64]>) {
    let mut guard = TEST_CLOCK.lock().unwrap();
    let armed = times.is_some();
    *guard = times.map(|times| TestClock {
        times: times.to_vec(),
        pos: 0,
    });
    // The clock is in place before the flag. A relaxed load that then
    // takes this mutex sees the write.
    TEST_CLOCK_ARMED.store(armed, Ordering::Release);
}

/// Next scripted timestamp, or `None` when the test clock is unset.
///
/// An exhausted clock returns `Some(0)` so the shim does not fall through
/// to the real clock. The relaxed load is the idle path: `profile_start`
/// and `profile_end` call this on every sample, and the mutex is taken
/// only while a test has armed the clock.
pub(crate) fn testing_clock_tick() -> Option<u64> {
    if !TEST_CLOCK_ARMED.load(Ordering::Relaxed) {
        return None;
    }
    let mut guard = TEST_CLOCK.lock().unwrap();
    let clock = guard.as_mut()?;
    if clock.pos < clock.times.len() {
        let now = clock.times[clock.pos];
        clock.pos += 1;
        Some(now)
    } else {
        Some(0)
    }
}

/// Clears roots, this thread's call stack, and the test clock.
///
/// The parity harness calls this between cases. It is not a C export:
/// `profiler_free` leaves a thread-local frame alone, and the C mutex cannot
/// be reused after that call.
pub fn testing_reset() {
    testing_set_clock(None);
    profiler_free();
    ACTIVE.with(|slot| *slot.borrow_mut() = None);
    STACK.with(|slot| slot.borrow_mut().clear());
    LAST_ROOT.with(|slot| *slot.borrow_mut() = None);
    set_thread_enabled(true);
}

pub fn profile_reenable_thread() {
    if thread_enabled() {
        return;
    }
    let enabled = lock_globals().enabled;
    set_thread_enabled(enabled);
}

pub fn profile_register_root(name: ProfileName, expected_time_between_calls_ns: u64) {
    let mut globals = lock_globals();
    if !globals.enabled {
        drop(globals);
        set_thread_enabled(false);
        return;
    }
    let root = get_root(&mut globals, name);
    let usec = expected_time_between_calls_ns.wrapping_add(500) / 1000;
    root.entry.lock().unwrap().expected = usec;
}

pub fn profile_start(name: ProfileName, now_ns: u64) {
    if !thread_enabled() {
        return;
    }
    ACTIVE.with(|active_slot| {
        STACK.with(|stack_slot| {
            let mut active_ref = active_slot.borrow_mut();
            let active = active_ref.get_or_insert_with(|| Active {
                nodes: SPARE.with(|spare| std::mem::take(&mut *spare.borrow_mut())),
            });
            let mut stack = stack_slot.borrow_mut();
            let parent = stack.last().copied();
            let idx = active.nodes.len();
            active.nodes.push(Node {
                name,
                start: now_ns,
                end: 0,
                parent,
                first_child: None,
                last_child: None,
                next_sibling: None,
            });
            if let Some(p) = parent {
                match active.nodes[p].last_child.replace(idx) {
                    Some(prev) => active.nodes[prev].next_sibling = Some(idx),
                    None => active.nodes[p].first_child = Some(idx),
                }
            }
            stack.push(idx);
        });
    });
}

fn top_name() -> Option<ProfileName> {
    STACK.with(|stack_slot| {
        let stack = stack_slot.borrow();
        let idx = *stack.last()?;
        ACTIVE.with(|active_slot| {
            active_slot
                .borrow()
                .as_ref()
                .map(|active| active.nodes[idx].name)
        })
    })
}

/// Ends the active call.
///
/// `clock` is read immediately, including when this thread is disabled, and
/// once more for every inner frame a mismatched end closes.
///
/// `log` receives the `LOG_ERROR` lines `profiler.c` prints, in the C's order.
pub fn profile_end(
    name: ProfileName,
    clock: &mut dyn FnMut() -> u64,
    log: &mut dyn FnMut(ProfileLog),
) {
    let now = clock();
    if !thread_enabled() {
        return;
    }
    let Some(mut active) = ACTIVE.with(|slot| slot.borrow_mut().take()) else {
        log(ProfileLog::NoActive);
        return;
    };
    let Some(cur) = STACK.with(|slot| slot.borrow().last().copied()) else {
        ACTIVE.with(|slot| *slot.borrow_mut() = Some(active));
        log(ProfileLog::NoActive);
        return;
    };
    if active.nodes[cur].name.is_null() {
        active.nodes[cur].name = name;
    }
    if active.nodes[cur].name != name {
        log(ProfileLog::Mismatch {
            started: active.nodes[cur].name,
            ended: name,
        });
        let mut parent = active.nodes[cur].parent;
        while let Some(p) = parent {
            if active.nodes[p].parent.is_some() && active.nodes[p].name != name {
                parent = active.nodes[p].parent;
            } else {
                break;
            }
        }
        let found = matches!(parent, Some(p) if active.nodes[p].name == name);
        ACTIVE.with(|slot| *slot.borrow_mut() = Some(active));
        if !found {
            return;
        }
        while top_name() != Some(name) {
            let Some(top) = top_name() else {
                return;
            };
            profile_end(top, clock, log);
        }
        active = ACTIVE
            .with(|slot| slot.borrow_mut().take())
            .expect("profile frame");
    }
    let idx = STACK
        .with(|slot| slot.borrow_mut().pop())
        .expect("profile frame");
    active.nodes[idx].end = now;
    let is_root = active.nodes[idx].parent.is_none();
    if !is_root {
        ACTIVE.with(|slot| *slot.borrow_mut() = Some(active));
        return;
    }
    merge_context(active);
}

fn merge_call(entry: &mut ProfileEntry, active: &Active, idx: usize, prev_start: Option<u64>) {
    let mut child = active.nodes[idx].first_child;
    while let Some(c) = child {
        let ci = entry.child_index(active.nodes[c].name);
        merge_call(&mut entry.children[ci], active, c, None);
        child = active.nodes[c].next_sibling;
    }
    if entry.expected != 0
        && let Some(prev) = prev_start
    {
        entry.between.migrate(true);
        entry
            .between
            .add(diff_ns_to_usec(prev, active.nodes[idx].start), 1);
    }
    entry.times.migrate(true);
    entry.times.add(
        diff_ns_to_usec(active.nodes[idx].start, active.nodes[idx].end),
        1,
    );
}

fn merge_context(mut active: Active) {
    if let Some(root) = root_for(active.nodes[0].name) {
        let prev_start = root.prev_start.lock().unwrap().replace(active.root_start());
        let mut entry = root.entry.lock().unwrap();
        merge_call(&mut entry, &active, 0, prev_start);
    } else {
        set_thread_enabled(false);
    }
    // Keep the buffer for this thread's next root.
    active.nodes.clear();
    SPARE.with(|spare| *spare.borrow_mut() = active.nodes);
}

/// The root to merge `name` into, or `None` when the profiler is off.
///
/// A thread usually ends the same root every time, so it keeps that root
/// and skips the global lock and the root scan while no
/// [`profiler_free`] has dropped the roots since.
fn root_for(name: ProfileName) -> Option<Arc<Root>> {
    if !ENABLED.load(Ordering::Acquire) {
        return None;
    }
    let generation = ROOTS_GENERATION.load(Ordering::Acquire);
    let cached = LAST_ROOT.with(|slot| match &*slot.borrow() {
        Some((n, g, root)) if *n == name && *g == generation => Some(Arc::clone(root)),
        _ => None,
    });
    if cached.is_some() {
        return cached;
    }
    let mut globals = lock_globals();
    if !globals.enabled {
        return None;
    }
    let root = get_root(&mut globals, name);
    let generation = ROOTS_GENERATION.load(Ordering::Acquire);
    drop(globals);
    LAST_ROOT.with(|slot| *slot.borrow_mut() = Some((name, generation, Arc::clone(&root))));
    Some(root)
}

pub fn profile_snapshot_create() -> Snapshot {
    let globals = lock_globals();
    let mut roots = Vec::with_capacity(globals.roots.len());
    for root in &globals.roots {
        roots.push(root.entry.lock().unwrap().snapshot_of());
    }
    Snapshot { roots }
}
