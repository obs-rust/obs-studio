#pragma once

/* C ABI of rust/obs-camera-busy (src/ffi/mod.rs), linked into the frontend
 * through rust/obs-frontend (obs-rust/obs-studio#3724). Every string a
 * function returns as `char *` is freed with obs_camera_busy_free. */

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* obs_camera_busy_begin decisions. */
#define OBS_CAMERA_BUSY_DIALOG 0
#define OBS_CAMERA_BUSY_ALLOWED 1
#define OBS_CAMERA_BUSY_DUPLICATE 2
#define OBS_CAMERA_BUSY_RATE_LIMITED 3
#define OBS_CAMERA_BUSY_NO_HOLDER 4

/* What noticed it. */
#define OBS_CAMERA_BUSY_CAUSE_BUSY 0
#define OBS_CAMERA_BUSY_CAUSE_OPENED 1

/* The user's answer; anything else is Ignore. */
#define OBS_CAMERA_BUSY_CLOSE_IT 0
#define OBS_CAMERA_BUSY_IGNORE 1
#define OBS_CAMERA_BUSY_ALWAYS_ALLOW 2

/* How obs_camera_busy_close ended. */
#define OBS_CAMERA_BUSY_EXITED 0
#define OBS_CAMERA_BUSY_KILLED 1
#define OBS_CAMERA_BUSY_FAILED 2

typedef struct Holders obs_camera_busy_holders_t;
typedef struct Watcher obs_camera_busy_watch_t;

/* The processes other than OBS that hold `device` (symlinks resolved), under
 * /proc. Free with obs_camera_busy_holders_free. */
obs_camera_busy_holders_t *obs_camera_busy_scan(const char *device);
/* The same under `proc_root`, with `device` as given (tests). */
obs_camera_busy_holders_t *obs_camera_busy_scan_root(const char *proc_root, const char *device, uint32_t self_pid,
						     uint32_t self_uid);
size_t obs_camera_busy_holders_count(const obs_camera_busy_holders_t *h);
uint32_t obs_camera_busy_holder_pid(const obs_camera_busy_holders_t *h, size_t i);
/* Owned by the list. */
const char *obs_camera_busy_holder_name(const obs_camera_busy_holders_t *h, size_t i);
bool obs_camera_busy_holder_can_close(const obs_camera_busy_holders_t *h, size_t i);
void obs_camera_busy_holders_free(obs_camera_busy_holders_t *h);

/* The "Always allow" list as a config string ("zoom;cheese"). */
void obs_camera_busy_set_allowlist(const char *config);
char *obs_camera_busy_allowlist(void);

/* Whether a warning shows a dialog (*show, OBS_CAMERA_BUSY_*), and the holders
 * it names, which go to obs_camera_busy_end when the dialog ends. */
obs_camera_busy_holders_t *obs_camera_busy_begin(int cause, const char *device,
						 const obs_camera_busy_holders_t *holders, uint64_t now_ms, int *show);
/* The dialog ended with `answer`: returns whether the allowlist changed. */
bool obs_camera_busy_end(const char *device, const obs_camera_busy_holders_t *holders, int answer, uint64_t now_ms);

/* "zoom (PID 12), cheese (PID 40)" */
char *obs_camera_busy_holder_list(const obs_camera_busy_holders_t *holders);
char *obs_camera_busy_log_line(int cause, const char *device, const obs_camera_busy_holders_t *holders, int show);
char *obs_camera_busy_answer_log_line(const char *device, const obs_camera_busy_holders_t *holders, int answer);

/* Close it: SIGTERM, SIGKILL after a grace. Blocks seconds: never on the UI
 * thread. */
int obs_camera_busy_close(uint32_t pid);
char *obs_camera_busy_close_log_line(const char *name, uint32_t pid, int closed);

/* Called on the watcher thread a moment after another process opened the
 * device, with who holds it then (often nobody); the callee frees `holders`. */
typedef void (*obs_camera_busy_watch_cb)(void *param, obs_camera_busy_holders_t *holders);
/* NULL when the watch cannot start. */
obs_camera_busy_watch_t *obs_camera_busy_watch(const char *device, obs_camera_busy_watch_cb cb, void *param);
/* Waits for a callback in progress; never call it from the callback. */
void obs_camera_busy_unwatch(obs_camera_busy_watch_t *w);

void obs_camera_busy_free(char *s);

#ifdef __cplusplus
}
#endif
