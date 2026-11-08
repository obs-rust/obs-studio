/*
 * The transition seam's callback, signal and video-time accessors
 * (libobs/obs-source-transition-seam.h, obs-rust/obs-studio#3835), which the
 * ports of the transition start and set and of the clock read and stop call
 * in place of the struct reads they wrap:
 * - obs_source_transition_seam_call_start() and _call_stop(): the type's
 *   transition_start (or transition_stop) callback on the transition's
 *   context data when it has one, and whether it had one;
 * - obs_source_transition_seam_dosignal(): obs_source_dosignal(), the
 *   source in the calldata, signal_obs on the core's handler unless it is
 *   NULL or the source is private, signal_source on the source's own
 *   handler unless it is NULL;
 * - obs_source_transition_seam_get_video_time(): the latest video frame's
 *   time, read under the video stats mutex.
 *
 * The accessors are hidden in libobs, so this test compiles the seam (and
 * the load checks its transition_valid() asks) into itself, and runs them
 * on a stand-in core (`obs`, with only its signal handler and video stats)
 * and stand-in transitions: zeroed struct obs_source with a type, callbacks,
 * context data, their own signal handler and the private flag. The signal
 * handlers and calldata are libobs's.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <string.h>

#include "obs-internal.h"
#include "obs-source-transition-seam.h"

/* libobs has this one in obs.c, hidden; the seam in this test reads this
 * stand-in. */
struct obs_core *obs;

static int data_tag;
static unsigned start_calls;
static unsigned stop_calls;
static void *last_data;

static void count_start(void *data)
{
	start_calls++;
	last_data = data;
}

static void count_stop(void *data)
{
	stop_calls++;
	last_data = data;
}

/* What each handler heard: the count and the last calldata's source. */
struct heard {
	unsigned count;
	void *source;
};

static struct heard global_start, global_stop, global_other;
static struct heard own_start, own_stop, own_other;

static void hear(void *param, calldata_t *cd)
{
	struct heard *h = param;
	h->count++;
	h->source = calldata_ptr(cd, "source");
}

static obs_source_t *new_transition(bool with_callbacks, bool private_source)
{
	obs_source_t *tr = bzalloc(sizeof(*tr));
	tr->info.type = OBS_SOURCE_TYPE_TRANSITION;
	if (with_callbacks) {
		tr->info.transition_start = count_start;
		tr->info.transition_stop = count_stop;
	}
	tr->context.data = &data_tag;
	tr->context.private = private_source;
	tr->context.signals = signal_handler_create();
	assert_non_null(tr->context.signals);
	signal_handler_add(tr->context.signals, "void transition_start(ptr source)");
	signal_handler_add(tr->context.signals, "void transition_stop(ptr source)");
	signal_handler_add(tr->context.signals, "void transition_other(ptr source)");
	signal_handler_connect(tr->context.signals, "transition_start", hear, &own_start);
	signal_handler_connect(tr->context.signals, "transition_stop", hear, &own_stop);
	signal_handler_connect(tr->context.signals, "transition_other", hear, &own_other);
	return tr;
}

static void free_transition(obs_source_t *tr)
{
	signal_handler_destroy(tr->context.signals);
	bfree(tr);
}

static int case_setup(void **state)
{
	UNUSED_PARAMETER(state);
	start_calls = 0;
	stop_calls = 0;
	last_data = NULL;
	memset(&global_start, 0, sizeof(global_start));
	memset(&global_stop, 0, sizeof(global_stop));
	memset(&global_other, 0, sizeof(global_other));
	memset(&own_start, 0, sizeof(own_start));
	memset(&own_stop, 0, sizeof(own_stop));
	memset(&own_other, 0, sizeof(own_other));
	return 0;
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	obs = bzalloc(sizeof(*obs));
	assert_int_equal(obs_video_stats_init(&obs->video.stats), 0);
	obs->signals = signal_handler_create();
	assert_non_null(obs->signals);
	signal_handler_add(obs->signals, "void source_transition_start(ptr source)");
	signal_handler_add(obs->signals, "void source_transition_stop(ptr source)");
	signal_handler_add(obs->signals, "void source_transition_other(ptr source)");
	signal_handler_connect(obs->signals, "source_transition_start", hear, &global_start);
	signal_handler_connect(obs->signals, "source_transition_stop", hear, &global_stop);
	signal_handler_connect(obs->signals, "source_transition_other", hear, &global_other);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	signal_handler_destroy(obs->signals);
	obs_video_stats_destroy(&obs->video.stats);
	bfree(obs);
	obs = NULL;
	return 0;
}

/* Each call runs its own callback once on the context data and says so;
 * neither runs the other's, and neither signals. */
static void calls_with_callbacks(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *tr = new_transition(true, false);

	assert_true(obs_source_transition_seam_call_start(tr));
	assert_int_equal(start_calls, 1);
	assert_int_equal(stop_calls, 0);
	assert_ptr_equal(last_data, &data_tag);

	last_data = NULL;
	assert_true(obs_source_transition_seam_call_stop(tr));
	assert_true(obs_source_transition_seam_call_stop(tr));
	assert_int_equal(start_calls, 1);
	assert_int_equal(stop_calls, 2);
	assert_ptr_equal(last_data, &data_tag);

	assert_int_equal(own_start.count + own_stop.count + global_start.count + global_stop.count, 0);
	free_transition(tr);
}

/* Without the callbacks, each call does nothing and returns false. */
static void calls_without_callbacks(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *tr = new_transition(false, false);

	assert_false(obs_source_transition_seam_call_start(tr));
	assert_false(obs_source_transition_seam_call_stop(tr));
	assert_int_equal(start_calls + stop_calls, 0);

	/* Only the one that is set. */
	tr->info.transition_stop = count_stop;
	assert_false(obs_source_transition_seam_call_start(tr));
	assert_true(obs_source_transition_seam_call_stop(tr));
	assert_int_equal(start_calls, 0);
	assert_int_equal(stop_calls, 1);
	free_transition(tr);
}

/* Both names: one signal on the core's handler, one on the transition's,
 * each with the transition as its source, and nothing else. */
static void dosignal_emits_both(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *tr = new_transition(true, false);

	obs_source_transition_seam_dosignal(tr, "source_transition_start", "transition_start");
	assert_int_equal(global_start.count, 1);
	assert_ptr_equal(global_start.source, tr);
	assert_int_equal(own_start.count, 1);
	assert_ptr_equal(own_start.source, tr);
	assert_int_equal(global_stop.count + own_stop.count, 0);

	obs_source_transition_seam_dosignal(tr, "source_transition_stop", "transition_stop");
	obs_source_transition_seam_dosignal(tr, "source_transition_stop", "transition_stop");
	assert_int_equal(global_stop.count, 2);
	assert_ptr_equal(global_stop.source, tr);
	assert_int_equal(own_stop.count, 2);
	assert_ptr_equal(own_stop.source, tr);
	assert_int_equal(global_start.count, 1);
	assert_int_equal(own_start.count, 1);

	/* The names pass through: neither is tied to the other. */
	obs_source_transition_seam_dosignal(tr, "source_transition_other", "transition_start");
	assert_int_equal(global_other.count, 1);
	assert_int_equal(own_start.count, 2);
	assert_int_equal(global_start.count, 1);

	/* The callbacks are not called. */
	assert_int_equal(start_calls + stop_calls, 0);
	free_transition(tr);
}

/* A NULL name skips that handler; both NULL, nothing. */
static void dosignal_skips_null_names(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *tr = new_transition(true, false);

	obs_source_transition_seam_dosignal(tr, NULL, "transition_stop");
	assert_int_equal(global_stop.count, 0);
	assert_int_equal(own_stop.count, 1);

	obs_source_transition_seam_dosignal(tr, "source_transition_stop", NULL);
	assert_int_equal(global_stop.count, 1);
	assert_int_equal(own_stop.count, 1);

	obs_source_transition_seam_dosignal(tr, NULL, NULL);
	assert_int_equal(global_stop.count, 1);
	assert_int_equal(own_stop.count, 1);
	free_transition(tr);
}

/* A private transition: only its own handler. */
static void dosignal_private_source(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *tr = new_transition(true, true);

	obs_source_transition_seam_dosignal(tr, "source_transition_stop", "transition_stop");
	assert_int_equal(global_stop.count, 0);
	assert_int_equal(own_stop.count, 1);
	assert_ptr_equal(own_stop.source, tr);
	free_transition(tr);
}

/* The latest frame time, as set; it reads the video stats under their
 * mutex, so the mutex is free again afterwards. */
static void video_time_is_the_frame_time(void **state)
{
	UNUSED_PARAMETER(state);
	obs_video_stats_set_frame_time(&obs->video.stats, 0);
	assert_int_equal(obs_source_transition_seam_get_video_time(), 0);

	obs_video_stats_set_frame_time(&obs->video.stats, 123456789012345ULL);
	assert_true(obs_source_transition_seam_get_video_time() == 123456789012345ULL);

	obs_video_stats_set_frame_time(&obs->video.stats, UINT64_MAX);
	assert_true(obs_source_transition_seam_get_video_time() == UINT64_MAX);

	assert_int_equal(pthread_mutex_trylock(&obs->video.stats.mutex), 0);
	pthread_mutex_unlock(&obs->video.stats.mutex);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(calls_with_callbacks, case_setup),
		cmocka_unit_test_setup(calls_without_callbacks, case_setup),
		cmocka_unit_test_setup(dosignal_emits_both, case_setup),
		cmocka_unit_test_setup(dosignal_skips_null_names, case_setup),
		cmocka_unit_test_setup(dosignal_private_source, case_setup),
		cmocka_unit_test_setup(video_time_is_the_frame_time, case_setup),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
