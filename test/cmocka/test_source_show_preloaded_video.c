/* Characterize obs_source_show_preloaded_video() (#5762) on stack source
 * fixtures under a started core without video (obs_enter_graphics() and
 * obs_leave_graphics() then do nothing): the early returns (a NULL source, a
 * source being destroyed, no preload frame) touch nothing; otherwise the
 * preload frame's flip and alpha mode reach the async upload state, the
 * textures are marked fresh and the source active, and the audio timing
 * restarts at the current time (time zero for a monitor-only source) against
 * the source's last frame timestamp, with the audio buffer emptied. Both
 * mutexes are free afterwards. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "util/platform.h"
#include "util/threading.h"

#define LAST_FRAME_TS 5000000ULL
#define SYNC_OFFSET 1000
#define RESAMPLE_OFFSET 200

struct fixture {
	struct obs_source source;
	struct obs_source_frame frame;
	uint8_t plane[16];
};

static int group_setup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int group_teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static int setup(void **state)
{
	struct fixture *f = bzalloc(sizeof(*f));
	struct obs_source *source = &f->source;
	/* As obs_source_init(). */
	assert_int_equal(pthread_mutex_init_recursive(&source->async_mutex), 0);
	assert_int_equal(pthread_mutex_init_recursive(&source->audio_buf_mutex), 0);

	/* NV12 without GPU conversion: the texture size already matches, so
	 * set_async_texture_size() keeps the textures, and update_async_textures()
	 * makes no graphics call. */
	f->frame.format = VIDEO_FORMAT_NV12;
	f->frame.width = 4;
	f->frame.height = 4;
	f->frame.data[0] = f->plane;
	f->frame.linesize[0] = 4;
	f->frame.flip = true;
	f->frame.flags = OBS_SOURCE_FRAME_LINEAR_ALPHA;

	source->async_format = VIDEO_FORMAT_NV12;
	source->async_width = 4;
	source->async_height = 4;
	source->async_textures_stale = true;
	source->last_frame_ts = LAST_FRAME_TS;
	source->sync_offset = SYNC_OFFSET;
	source->resample_offset = RESAMPLE_OFFSET;
	source->last_audio_input_buf_size = 7;
	source->next_audio_sys_ts_min = 1;
	source->audio_ts = 1;
	uint8_t samples[32] = {0};
	deque_push_back(&source->audio_input_buf[0], samples, sizeof(samples));

	*state = f;
	return 0;
}

static int teardown(void **state)
{
	struct fixture *f = *state;
	deque_free(&f->source.audio_input_buf[0]);
	assert_int_equal(pthread_mutex_destroy(&f->source.async_mutex), 0);
	assert_int_equal(pthread_mutex_destroy(&f->source.audio_buf_mutex), 0);
	bfree(f);
	return 0;
}

/* Both mutexes are recursive, so probe them from another thread. */
struct probe {
	pthread_mutex_t *mutex;
	int result;
};

static void *trylock(void *data)
{
	struct probe *p = data;
	p->result = pthread_mutex_trylock(p->mutex);
	if (p->result == 0)
		pthread_mutex_unlock(p->mutex);
	return NULL;
}

static void expect_unlocked(pthread_mutex_t *mutex)
{
	struct probe p = {mutex, -1};
	pthread_t thread;
	assert_int_equal(pthread_create(&thread, NULL, trylock, &p), 0);
	pthread_join(thread, NULL);
	assert_int_equal(p.result, 0);
}

static void expect_untouched(const struct obs_source *source)
{
	assert_false(source->async_active);
	assert_true(source->async_textures_stale);
	assert_false(source->async_flip);
	assert_false(source->async_linear_alpha);
	assert_false(source->timing_set);
	assert_int_equal(source->timing_adjust, 0);
	assert_int_equal(source->audio_ts, 1);
	assert_int_equal(source->next_audio_sys_ts_min, 1);
	assert_int_equal(source->last_audio_input_buf_size, 7);
	assert_int_equal(source->audio_input_buf[0].size, 32);
}

static void null_source(void **state)
{
	(void)state;
	obs_source_show_preloaded_video(NULL);
}

static void destroying_source(void **state)
{
	struct fixture *f = *state;
	f->source.async_preload_frame = &f->frame;
	f->source.destroying = true;

	obs_source_show_preloaded_video(&f->source);

	expect_untouched(&f->source);
	assert_ptr_equal(f->source.async_preload_frame, &f->frame);
}

static void without_preload_frame(void **state)
{
	struct fixture *f = *state;

	obs_source_show_preloaded_video(&f->source);

	expect_untouched(&f->source);
	assert_null(f->source.async_preload_frame);
}

static void expect_shown(const struct fixture *f, uint64_t sys_ts)
{
	const struct obs_source *source = &f->source;
	/* The preload frame stays where it is; only its state was used. */
	assert_ptr_equal(source->async_preload_frame, &f->frame);
	assert_true(source->async_flip);
	assert_true(source->async_linear_alpha);
	assert_false(source->async_textures_stale);
	assert_true(source->async_active);
	assert_int_equal(source->async_width, 4);
	assert_int_equal(source->async_height, 4);
	assert_int_equal(source->async_format, VIDEO_FORMAT_NV12);

	assert_true(source->timing_set);
	assert_int_equal(source->timing_adjust, sys_ts - LAST_FRAME_TS);
	assert_int_equal(source->audio_input_buf[0].size, 0);
	assert_int_equal(source->last_audio_input_buf_size, 0);
	assert_int_equal(source->next_audio_sys_ts_min, sys_ts);
	assert_int_equal(source->audio_ts, sys_ts ? sys_ts + SYNC_OFFSET - RESAMPLE_OFFSET : 0);

	expect_unlocked((pthread_mutex_t *)&source->async_mutex);
	expect_unlocked((pthread_mutex_t *)&source->audio_buf_mutex);
}

static void shows_at_current_time(void **state)
{
	struct fixture *f = *state;
	f->source.async_preload_frame = &f->frame;
	f->source.monitoring_type = OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT;

	const uint64_t before = os_gettime_ns();
	obs_source_show_preloaded_video(&f->source);
	const uint64_t after = os_gettime_ns();

	/* The restart time is the one clock read, between before and after. */
	const uint64_t sys_ts = f->source.next_audio_sys_ts_min;
	assert_true(sys_ts >= before);
	assert_true(sys_ts <= after);
	expect_shown(f, sys_ts);
}

static void shows_unmonitored_at_current_time(void **state)
{
	struct fixture *f = *state;
	f->source.async_preload_frame = &f->frame;
	f->source.monitoring_type = OBS_MONITORING_TYPE_NONE;

	const uint64_t before = os_gettime_ns();
	obs_source_show_preloaded_video(&f->source);
	const uint64_t after = os_gettime_ns();

	const uint64_t sys_ts = f->source.next_audio_sys_ts_min;
	assert_true(sys_ts >= before);
	assert_true(sys_ts <= after);
	expect_shown(f, sys_ts);
}

static void monitor_only_shows_at_time_zero(void **state)
{
	struct fixture *f = *state;
	f->source.async_preload_frame = &f->frame;
	f->source.monitoring_type = OBS_MONITORING_TYPE_MONITOR_ONLY;

	obs_source_show_preloaded_video(&f->source);

	expect_shown(f, 0);
}

/* A frame without the flip and alpha flags clears them: the upload state is
 * copied from the frame, not or-ed in. */
static void copies_upload_state_from_frame(void **state)
{
	struct fixture *f = *state;
	f->frame.flip = false;
	f->frame.flags = 0;
	f->source.async_flip = true;
	f->source.async_linear_alpha = true;
	f->source.async_preload_frame = &f->frame;

	obs_source_show_preloaded_video(&f->source);

	assert_false(f->source.async_flip);
	assert_false(f->source.async_linear_alpha);
	assert_true(f->source.async_active);
	assert_false(f->source.async_textures_stale);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source),
		cmocka_unit_test_setup_teardown(destroying_source, setup, teardown),
		cmocka_unit_test_setup_teardown(without_preload_frame, setup, teardown),
		cmocka_unit_test_setup_teardown(shows_at_current_time, setup, teardown),
		cmocka_unit_test_setup_teardown(shows_unmonitored_at_current_time, setup, teardown),
		cmocka_unit_test_setup_teardown(monitor_only_shows_at_time_zero, setup, teardown),
		cmocka_unit_test_setup_teardown(copies_upload_state_from_frame, setup, teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
