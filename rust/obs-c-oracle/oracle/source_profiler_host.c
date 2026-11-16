/* Test-only stand-ins for the libobs core calls the Rust source-profiler
 * shim (rust/obs-core/src/ffi/source_profiler.rs) makes. libobs supplies the
 * real ones; cargo test links no libobs core, so these let the obs-core shim
 * link and run with no graphics: no context, NULL timers and ranges, no
 * async flag. The graphics task queue runs the task at once. The timer and
 * timer-range entry points the shim also calls (gs_timer_destroy, _begin,
 * _end, _get_data and their gs_timer_range_* twins) are Rust in
 * obs-graphics, which every binary that links this shim links too; with no
 * graphics context they do nothing and return false, so they are not
 * defined here (a second definition breaks the port_comparison link). The C
 * oracle of source-profiler.c (oracle/source_profiler.c) uses its own
 * renamed stand-ins, not these. */

#include "obs-internal.h"

void obs_enter_graphics(void) {}

void obs_leave_graphics(void) {}

void obs_queue_task(enum obs_task_type type, obs_task_t task, void *param, bool wait)
{
	UNUSED_PARAMETER(type);
	UNUSED_PARAMETER(wait);
	task(param);
}

uint32_t obs_source_get_output_flags(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0;
}

uint64_t obs_source_get_last_async_ts(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0;
}

gs_timer_t *gs_timer_create(void)
{
	return NULL;
}

gs_timer_range_t *gs_timer_range_create(void)
{
	return NULL;
}
