//! obs-callback tests, pulled into `obs-tests-core`.

mod calldata;
mod decl;
mod decl_parity;
mod layout;
mod parity;
mod proc;
mod proc_parity;
mod signal;
mod signal_parity;

use std::sync::Mutex;

/// Held by every test here that swaps the process-wide log handler
/// (`base_set_log_handler`), so two swaps never interleave their save and
/// restore.
pub(crate) static LOG_HANDLER: Mutex<()> = Mutex::new(());
