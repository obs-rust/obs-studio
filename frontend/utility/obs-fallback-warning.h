#pragma once

/* C ABI of rust/obs-fallback-warning (src/ffi/mod.rs), linked into the
 * frontend through rust/obs-frontend when OBS_FRONTEND_RUST is defined
 * (obs-rust/obs-studio#2218). Call it from the UI thread. */

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* obs_fallback_warning_begin results. */
#define OBS_FALLBACK_WARNING_DIALOG 0
#define OBS_FALLBACK_WARNING_ACCEPT_NOW 1

/* How a dialog ended, for obs_fallback_warning_accepted. */
#define OBS_FALLBACK_WARNING_END_COUNTDOWN 0
#define OBS_FALLBACK_WARNING_END_ACCEPT 1
#define OBS_FALLBACK_WARNING_END_CANCEL 2
#define OBS_FALLBACK_WARNING_END_CLOSED 3

/* Whether warning `id` shows a dialog now: the first time per process while
 * the main window is visible. Without a visible window it accepts now and is
 * not remembered. NULL or "" is never remembered. */
int obs_fallback_warning_begin(const char *id, bool window_visible);

/* Whether the caller continues in the fallback mode after the dialog ended
 * with `end` (OBS_FALLBACK_WARNING_END_*). Cancel and Close stop only a
 * caller that can stop. */
bool obs_fallback_warning_accepted(int end, bool can_cancel);

/* The countdown progress after `elapsed_ms`, 0 to 100. */
int obs_fallback_warning_progress(uint64_t elapsed_ms);

/* How long the countdown runs before it accepts by itself. */
uint64_t obs_fallback_warning_countdown_ms(void);

/* The log line for warning `id`: the message, then the future-error
 * sentence. NULL is empty. Free it with obs_fallback_warning_free. */
char *obs_fallback_warning_log_line(const char *id, const char *message);

void obs_fallback_warning_free(char *s);

#ifdef __cplusplus
}
#endif
