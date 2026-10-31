//! Safe procedure table.
//!
//! `libobs/callback/proc.c.h` stores callbacks under the name parsed from a
//! declaration. This core stores the name bytes and the callback address.
//! The first registration of a name wins. Parsing, `blog`, and the call
//! itself stay in the shim: a Tier 1 test does not invoke a raw function
//! pointer. `bnum_allocs` and a null handler stay with
//! `test/cmocka/test_proc.c`.
//!
//! [`ProcHandler::prepare_call`] drops the table lock before it returns, so
//! the caller can add, call, or destroy the same handler from the callback.
//! `None` is missing even when a procedure is registered. The shim is what
//! faults in `strcmp` for that case.

use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::name_table::NameTable;

struct Proc {
    func: usize,
    data: usize,
}

/// What [`ProcHandler::add`] did with a name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddOutcome {
    /// The name was not in the table. The callback is stored.
    Added,
    /// The name was already stored. The first callback stays.
    Exists,
}

/// A lookup that has not called the procedure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparedCall {
    /// No procedure has that name.
    Missing,
    /// Copied callback address and data. The caller invokes it.
    Ready { func: usize, data: usize },
}

/// Name to callback table. The first registration of a name wins.
pub struct ProcHandler {
    procs: RwLock<NameTable<Proc>>,
}

impl Default for ProcHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcHandler {
    pub fn new() -> Self {
        Self {
            procs: RwLock::new(NameTable::default()),
        }
    }

    /// Register `func` under `name`. A second add of the same bytes is
    /// [`AddOutcome::Exists`] and does not replace the first callback.
    /// Comparison is the raw bytes, the same result as `strcmp` for a
    /// NUL-terminated name.
    pub fn add(&self, name: &[u8], func: usize, data: usize) -> AddOutcome {
        if self.write().insert(name, Proc { func, data }) {
            AddOutcome::Added
        } else {
            AddOutcome::Exists
        }
    }

    /// Copy the callback for `name`. The lock is released before this
    /// returns. `None` is [`PreparedCall::Missing`].
    pub fn prepare_call(&self, name: Option<&[u8]>) -> PreparedCall {
        let Some(name) = name else {
            return PreparedCall::Missing;
        };
        match self.read().get(name) {
            Some(proc) => PreparedCall::Ready {
                func: proc.func,
                data: proc.data,
            },
            None => PreparedCall::Missing,
        }
    }

    /// True when no procedure has been stored. The shim uses this so a null
    /// name on an empty handler returns false instead of reaching `strcmp`.
    pub(crate) fn is_empty(&self) -> bool {
        self.read().is_empty()
    }

    fn read(&self) -> RwLockReadGuard<'_, NameTable<Proc>> {
        self.procs.read().unwrap_or_else(|err| err.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, NameTable<Proc>> {
        self.procs.write().unwrap_or_else(|err| err.into_inner())
    }
}
