#pragma once

/* C ABI of rust/obs-exit-watchdog (src/ffi/mod.rs), linked into the frontend
 * through rust/obs-frontend (obs-rust/obs-studio#4065). */

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Arms the exit watchdog once per process. When the process still runs
 * after the watchdog's timeout, it appends one line to `log_path` (UTF-8,
 * may be NULL) and to stderr and ends the process with exit code 124.
 * Returns false when it was already armed or did not start. */
bool obs_exit_watchdog_arm(const char *log_path);

#ifdef __cplusplus
}
#endif
