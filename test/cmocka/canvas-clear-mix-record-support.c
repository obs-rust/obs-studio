/* test_canvas_clear_mix_rust: the real private clear-mix adapters, bound to
 * the unchanged characterization's own core and recording teardown. One
 * unity group includes this after the test, so current_core, record_lock,
 * record_unlock and record_free are the test's own, exactly as its
 * non-external branch binds the original. */
#define obs current_core
#define obs_free_video_mix record_free
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock record_lock
#define pthread_mutex_unlock record_unlock
#include "libobs/obs-canvas-clear-mix-fields.c.h"
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")
#undef obs_free_video_mix
#undef obs

#ifdef CANVAS_CLEAR_MIX_NULL_PROBE
/* The C fixture's Linux NULL-canvas fault probe, linked here as part of the
 * same group so the unchanged null_canvas case runs on the Rust shim too. */
#include "test/cmocka/canvas-clear-mix-null-host-linux.c"
#endif
