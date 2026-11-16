#pragma once

/* C ABI of rust/obs-exit-watchdog (src/ffi/mod.rs), linked into the frontend
 * through rust/obs-frontend (obs-rust/obs-studio#4065). */

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Starts the exit watchdog's helper process (obs-rust/obs-studio#4589):
 * on Windows only, from the file next to the OBS executable, with the
 * watchdog's timeout and `log_path` (UTF-8, may be NULL). Once it runs,
 * obs_exit_watchdog_arm() arms it instead of a thread, so a plugin DLL
 * that blocks its detach inside ExitProcess is still ended. Call it early,
 * long before quit. Returns false when no helper started (another host, no
 * helper file, or already started). */
bool obs_exit_watchdog_start(const char *log_path);

/* Arms the exit watchdog once per process, for the calling thread. When
 * the process still runs after the watchdog's timeout, it appends one line
 * to `log_path` (UTF-8, may be NULL) and to stderr, naming the library the
 * calling thread waits in when it can, and ends the process with exit
 * code 124.
 * Returns false when it was already armed or did not start. */
bool obs_exit_watchdog_arm(const char *log_path);

/* Which module brought in which library (obs-rust/obs-studio#4525), so the
 * watchdog line can name the library the main thread waits in and its
 * module. The frontend notes each loaded module's binary path and calls
 * note_loaded after loading the modules and note_post_load after their
 * post-load. */
void obs_exit_watchdog_note_module(const char *bin_path);
void obs_exit_watchdog_note_loaded(void);
void obs_exit_watchdog_note_post_load(void);

#ifdef __cplusplus
}
#endif
