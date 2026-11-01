//! Safe signal handler: the broadcast table of `libobs/callback/signal.c.h`.
//!
//! A handler holds named signals, each with an ordered list of callbacks,
//! and a list of global callbacks that see every signal. Callbacks are
//! opaque [`CallbackKey`] values (function and data addresses); the caller
//! invokes them, so this core runs no foreign code itself.
//!
//! Locking is the C locking. Each signal, and the global list, has a
//! re-entrant lock that is held while its callbacks run. A callback may
//! connect, disconnect, signal or call [`remove_current`] on the same
//! handler from its own thread. Another thread that disconnects waits
//! until the running pass is done, so after `disconnect` returns, that
//! callback is not running.
//!
//! The running callback is a thread-local (signal, index) or (global
//! list, index) pair, as in C, not a pointer into the callback array. A
//! connect only pushes, and nothing is erased while a pass runs, so the
//! index stays valid.
//!
//! As in C (pinned by `test/cmocka/test_signal.c`):
//! - A callback connected while signalling runs in the same pass.
//! - The running callback is the innermost one. A signal callback clears
//!   the running global callback and a global callback clears the running
//!   signal callback; both are restored when the callback returns, so a
//!   [`remove_current`] after a nested signal marks the outer callback.
//! - Passes of one signal nest by count, and so do the global passes.
//!   Removed callbacks are erased only when the outermost pass ends. A
//!   disconnect while a pass runs only marks the callback.
//! - Global callbacks are found by callback and data, so one that
//!   disconnects itself while it runs is marked and erased after the pass.
//! - A `keep_ref` reference given back after a pass destroys the handler
//!   when it was the last one.
//!
//! Characterized, not endorsed: an unknown signal returns before the global
//! callbacks.

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, RwLock};

use crate::decl::{Decl, DeclParse, parse_decl};
use crate::name_table::NameTable;

/// A callback as the C API names it: function and data addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CallbackKey {
    pub func: usize,
    pub data: usize,
}

/// Result of [`SignalHandler::add`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddResult {
    Added,
    /// The declaration did not parse. `parse.warning` is what decl logs.
    Invalid(DeclParse),
    /// A signal with this name exists.
    Exists(Vec<u8>),
}

/// A lock that its owner thread can take again.
///
/// `std::sync::ReentrantLock` is not stable. The data a holder changes
/// sits in its own `Mutex`, taken only for short steps and never across a
/// callback, so taking this lock twice on one thread is safe.
#[derive(Default)]
struct ReentrantLock {
    state: Mutex<Owner>,
    released: Condvar,
}

/// The thread that holds a `ReentrantLock`, how often, and how many
/// threads wait for it.
#[derive(Default)]
struct Owner {
    thread: Option<usize>,
    depth: usize,
    waiting: usize,
}

struct ReentrantGuard<'a> {
    lock: &'a ReentrantLock,
}

thread_local! {
    /// A byte whose address names this thread while it lives. Cheaper than
    /// `thread::current().id()` on every lock (#3212).
    static THREAD_TAG: u8 = const { 0 };
}

/// This thread's owner id for a `ReentrantLock`. An owner id is only
/// stored while its thread holds the lock, and a thread can't exit while
/// holding it, so an address reused after a thread exits never matches a
/// stale owner.
fn thread_tag() -> usize {
    THREAD_TAG.with(|tag| core::ptr::from_ref(tag).addr())
}

impl ReentrantLock {
    fn lock(&self) -> ReentrantGuard<'_> {
        let me = thread_tag();
        let mut state = lock(&self.state);
        while state.thread.is_some_and(|owner| owner != me) {
            state.waiting += 1;
            state = self.released.wait(state).unwrap_or_else(|e| e.into_inner());
            state.waiting -= 1;
        }
        state.thread = Some(me);
        state.depth += 1;
        ReentrantGuard { lock: self }
    }
}

impl Drop for ReentrantGuard<'_> {
    // PERF GUARD #1748
    fn drop(&mut self) {
        let mut state = lock(&self.lock.state);
        state.depth -= 1;
        if state.depth == 0 {
            state.thread = None;
            // Only when a thread waits: on Linux every notify is a futex
            // syscall, and each signal releases two of these locks.
            if state.waiting > 0 {
                self.lock.released.notify_one();
            }
        }
    }
}

/// A poisoned lock still holds consistent data: no step here panics with
/// the data half-changed.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Clone, Copy)]
struct Entry {
    key: CallbackKey,
    remove: bool,
    keep_ref: bool,
}

#[derive(Default)]
struct Callbacks {
    entries: Vec<Entry>,
    /// Depth of the running passes of this signal.
    signalling: usize,
}

/// `struct signal_info`.
struct Signal {
    decl: Decl,
    pass: ReentrantLock,
    callbacks: Mutex<Callbacks>,
    /// Bumped on every change to `callbacks.entries` (#3350).
    generation: AtomicU64,
}

#[derive(Clone, Copy)]
struct GlobalEntry {
    key: CallbackKey,
    remove: bool,
}

#[derive(Default)]
struct GlobalCallbacks {
    entries: Vec<GlobalEntry>,
    /// Depth of the running global passes.
    signalling: usize,
}

#[derive(Default)]
struct Globals {
    pass: ReentrantLock,
    callbacks: Mutex<GlobalCallbacks>,
    /// Bumped on every change to `callbacks.entries` (#3350).
    generation: AtomicU64,
}

/// Entries a pass copies out under one lock (#3350).
const SNAPSHOT: usize = 8;

/// Marks a change to a callback list, so a running pass on this thread
/// reads the list again after the callback that made it.
fn bump(generation: &AtomicU64) {
    generation.fetch_add(1, Ordering::Relaxed);
}

thread_local! {
    /// The signal passes running on this thread, outermost first. Passes
    /// nest strictly on one thread, so each pushes on entry and pops on
    /// exit: one `Arc` clone per pass instead of one per callback (#3212).
    static SIGNAL_PASSES: RefCell<Vec<Arc<Signal>>> = const { RefCell::new(Vec::new()) };
    /// The global passes running on this thread, likewise.
    static GLOBAL_PASSES: RefCell<Vec<Arc<Globals>>> = const { RefCell::new(Vec::new()) };
    /// `current_signal_cb`, as (slot in `SIGNAL_PASSES`, index).
    static CURRENT_SIGNAL: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
    /// `current_global_cb`, as (slot in `GLOBAL_PASSES`, index).
    static CURRENT_GLOBAL: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

type Current = (Option<(usize, usize)>, Option<(usize, usize)>);

/// Pushes `item` on a pass stack for the life of the returned slot guard.
struct PassSlot<T: 'static> {
    stack: &'static std::thread::LocalKey<RefCell<Vec<Arc<T>>>>,
    slot: usize,
}

impl<T: 'static> PassSlot<T> {
    fn push(stack: &'static std::thread::LocalKey<RefCell<Vec<Arc<T>>>>, item: &Arc<T>) -> Self {
        let slot = stack.with(|s| {
            let mut s = s.borrow_mut();
            s.push(Arc::clone(item));
            s.len() - 1
        });
        Self { stack, slot }
    }
}

impl<T: 'static> Drop for PassSlot<T> {
    fn drop(&mut self) {
        self.stack.with(|s| s.borrow_mut().truncate(self.slot));
    }
}

/// Makes `current` the running callbacks of this thread and returns the
/// ones it replaces.
fn swap_current(current: Current) -> Current {
    let (signal, global) = current;
    (
        CURRENT_SIGNAL.with(|c| c.replace(signal)),
        CURRENT_GLOBAL.with(|c| c.replace(global)),
    )
}

/// `signal_handler_remove_current`: marks the running signal callback of
/// this thread, else its running global callback. Outside a callback it
/// does nothing.
pub fn remove_current() {
    if let Some((slot, i)) = CURRENT_SIGNAL.with(Cell::get) {
        let sig = SIGNAL_PASSES.with(|s| s.borrow().get(slot).cloned());
        if let Some(sig) = sig
            && let Some(entry) = lock(&sig.callbacks).entries.get_mut(i)
        {
            entry.remove = true;
            bump(&sig.generation);
        }
        return;
    }
    if let Some((slot, i)) = CURRENT_GLOBAL.with(Cell::get) {
        let globals = GLOBAL_PASSES.with(|s| s.borrow().get(slot).cloned());
        if let Some(globals) = globals
            && let Some(entry) = lock(&globals.callbacks).entries.get_mut(i)
        {
            entry.remove = true;
            bump(&globals.generation);
        }
    }
}

/// `struct signal_handler`, without its storage: the shim owns that.
pub struct SignalHandler {
    signals: RwLock<NameTable<Arc<Signal>>>,
    refs: AtomicI64,
    globals: Arc<Globals>,
}

impl Default for SignalHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalHandler {
    /// `signal_handler_create`: one reference, the creator's.
    pub fn new() -> Self {
        Self {
            signals: RwLock::new(NameTable::default()),
            refs: AtomicI64::new(1),
            globals: Arc::new(Globals::default()),
        }
    }

    /// The reference count, for tests.
    pub fn refs(&self) -> i64 {
        self.refs.load(Ordering::Acquire)
    }

    /// Drops one reference. True when it was the last one; the owner then
    /// frees the handler (`signal_handler_actually_destroy`).
    pub fn release(&self) -> bool {
        self.refs.fetch_sub(1, Ordering::AcqRel) == 1
    }

    fn find(&self, name: &[u8]) -> Option<Arc<Signal>> {
        self.signals
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .get(name)
            .cloned()
    }

    /// `signal_handler_add`.
    pub fn add(&self, decl_string: &[u8]) -> AddResult {
        let parsed = parse_decl(decl_string);
        let Some(decl) = parsed.decl.clone() else {
            return AddResult::Invalid(parsed);
        };
        let name = decl.name.clone();
        let signal = Arc::new(Signal {
            decl,
            pass: ReentrantLock::default(),
            callbacks: Mutex::new(Callbacks::default()),
            generation: AtomicU64::new(0),
        });
        let mut signals = self.signals.write().unwrap_or_else(|err| err.into_inner());
        if signals.insert(name.as_ref(), signal) {
            AddResult::Added
        } else {
            AddResult::Exists(name)
        }
    }

    /// The declaration of a signal, as parsed.
    pub fn decl(&self, name: &[u8]) -> Option<Decl> {
        self.find(name).map(|s| s.decl.clone())
    }

    /// `signal_handler_connect` (`keep_ref` false) and
    /// `signal_handler_connect_ref` (true). False when the signal is not
    /// found; C logs a warning then.
    pub fn connect(&self, signal: &[u8], key: CallbackKey, keep_ref: bool) -> bool {
        let Some(sig) = self.find(signal) else {
            return false;
        };
        let _pass = sig.pass.lock();
        if keep_ref {
            self.refs.fetch_add(1, Ordering::AcqRel);
        }
        let mut cbs = lock(&sig.callbacks);
        if keep_ref || !cbs.entries.iter().any(|e| e.key == key) {
            cbs.entries.push(Entry {
                key,
                remove: false,
                keep_ref,
            });
            bump(&sig.generation);
        }
        true
    }

    /// `signal_handler_disconnect`. While signalling, the first match is
    /// only marked. True when a `keep_ref` reference was dropped and it was
    /// the last one; the owner then frees the handler.
    pub fn disconnect(&self, signal: &[u8], key: CallbackKey) -> bool {
        let Some(sig) = self.find(signal) else {
            return false;
        };
        let keep_ref = {
            let _pass = sig.pass.lock();
            let mut cbs = lock(&sig.callbacks);
            let signalling = cbs.signalling != 0;
            bump(&sig.generation);
            match cbs.entries.iter().position(|e| e.key == key) {
                Some(i) if signalling => {
                    cbs.entries[i].remove = true;
                    false
                }
                Some(i) => cbs.entries.remove(i).keep_ref,
                None => false,
            }
        };
        keep_ref && self.release()
    }

    /// `signal_handler_connect_global`.
    pub fn connect_global(&self, key: CallbackKey) {
        let _pass = self.globals.pass.lock();
        let mut cbs = lock(&self.globals.callbacks);
        if global_find(&cbs.entries, key).is_none() {
            cbs.entries.push(GlobalEntry { key, remove: false });
            bump(&self.globals.generation);
        }
    }

    /// `signal_handler_disconnect_global`. While a global pass runs, the
    /// entry is only marked, so the running callback's index stays valid.
    pub fn disconnect_global(&self, key: CallbackKey) {
        let _pass = self.globals.pass.lock();
        let mut cbs = lock(&self.globals.callbacks);
        if let Some(i) = global_find(&cbs.entries, key) {
            bump(&self.globals.generation);
            if cbs.signalling != 0 {
                cbs.entries[i].remove = true;
            } else {
                cbs.entries.remove(i);
            }
        }
    }

    /// `signal_handler_signal`. `call` runs each live callback of the
    /// signal, then `call_global` each live global callback. An unknown
    /// signal calls nothing. True when a `keep_ref` reference given back
    /// after the pass was the last one; the owner then frees the handler.
    pub fn signal(
        &self,
        signal: &[u8],
        mut call: impl FnMut(CallbackKey),
        mut call_global: impl FnMut(CallbackKey),
    ) -> bool {
        let Some(sig) = self.find(signal) else {
            return false;
        };
        let mut remove_refs = 0;
        // No callbacks (none live, none marked for removal): skip the pass,
        // its reentrant lock and the pass stack (#3277). Nothing would run
        // or be swept, and a connect racing this check is the same as one
        // that waited for the pass to end; it would not run in this pass
        // either. The global pass below still runs.
        if !lock(&sig.callbacks).entries.is_empty() {
            let _pass = sig.pass.lock();
            let pass_slot = PassSlot::push(&SIGNAL_PASSES, &sig);
            lock(&sig.callbacks).signalling += 1;

            // Every writer holds `pass`, so while it is held only this
            // thread's callbacks change the list. Copy it out in chunks
            // under one lock, and read it again only after a callback that
            // changed it (#3350). A callback connected in this pass still
            // runs in it: the next chunk, or the re-read, picks it up.
            let slot = pass_slot.slot;
            let previous = swap_current((Some((slot, 0)), None));
            let mut snapshot = [None::<Entry>; SNAPSHOT];
            let mut i = 0;
            'pass: loop {
                let (count, generation) = {
                    let cbs = lock(&sig.callbacks);
                    let rest = cbs.entries.get(i..).unwrap_or_default();
                    let count = rest.len().min(SNAPSHOT);
                    for (to, from) in snapshot.iter_mut().zip(&rest[..count]) {
                        *to = Some(*from);
                    }
                    (count, sig.generation.load(Ordering::Relaxed))
                };
                if count == 0 {
                    break;
                }
                for entry in snapshot[..count].iter().flatten() {
                    let index = i;
                    i += 1;
                    if entry.remove {
                        continue;
                    }
                    CURRENT_SIGNAL.with(|c| c.set(Some((slot, index))));
                    call(entry.key);
                    if sig.generation.load(Ordering::Relaxed) != generation {
                        continue 'pass;
                    }
                }
            }
            swap_current(previous);

            let mut cbs = lock(&sig.callbacks);
            cbs.signalling -= 1;
            if cbs.signalling == 0 {
                for i in (0..cbs.entries.len()).rev() {
                    if cbs.entries[i].remove {
                        if cbs.entries[i].keep_ref {
                            remove_refs += 1;
                        }
                        cbs.entries.remove(i);
                    }
                }
                bump(&sig.generation);
            }
        }

        // No global callbacks: skip the pass and its locks (#3212). Nothing
        // would run, and a connect from another thread racing this check
        // is the same as one that waited for the pass to end; it would not
        // have run in this pass either.
        if !lock(&self.globals.callbacks).entries.is_empty() {
            let globals = &self.globals;
            let _pass = globals.pass.lock();
            let pass_slot = PassSlot::push(&GLOBAL_PASSES, globals);
            lock(&globals.callbacks).signalling += 1;
            // As in the signal pass above (#3350).
            let slot = pass_slot.slot;
            let previous = swap_current((None, Some((slot, 0))));
            let mut snapshot = [None::<GlobalEntry>; SNAPSHOT];
            let mut i = 0;
            'pass: loop {
                let (count, generation) = {
                    let cbs = lock(&globals.callbacks);
                    let rest = cbs.entries.get(i..).unwrap_or_default();
                    let count = rest.len().min(SNAPSHOT);
                    for (to, from) in snapshot.iter_mut().zip(&rest[..count]) {
                        *to = Some(*from);
                    }
                    (count, globals.generation.load(Ordering::Relaxed))
                };
                if count == 0 {
                    break;
                }
                for entry in snapshot[..count].iter().flatten() {
                    let index = i;
                    i += 1;
                    if entry.remove {
                        continue;
                    }
                    CURRENT_GLOBAL.with(|c| c.set(Some((slot, index))));
                    call_global(entry.key);
                    if globals.generation.load(Ordering::Relaxed) != generation {
                        continue 'pass;
                    }
                }
            }
            swap_current(previous);
            let mut cbs = lock(&globals.callbacks);
            cbs.signalling -= 1;
            if cbs.signalling == 0 {
                cbs.entries.retain(|entry| !entry.remove);
                bump(&globals.generation);
            }
        }

        (0..remove_refs).any(|_| self.release())
    }
}

/// `signal_get_global_callback_idx`: the first entry with this callback
/// and data, marked or running or not.
fn global_find(entries: &[GlobalEntry], key: CallbackKey) -> Option<usize> {
    entries.iter().position(|e| e.key == key)
}
