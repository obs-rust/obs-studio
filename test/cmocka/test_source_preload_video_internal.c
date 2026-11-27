/* Characterize obs_source_preload_video_internal() (#5837), the hidden worker
 * every obs_source_preload_video*() entry ends in, through the public
 * obs_source_preload_video() on a private async source:
 * - A NULL frame changes nothing: no preload frame is made, an existing one
 *   is kept as it is, and the frame clock stays.
 * - The first frame makes the preload frame with obs_source_frame_create()
 *   at the frame's format and size, then copies the frame into it (the
 *   planes into the preload frame's own buffers) and sets last_frame_ts to
 *   the frame's timestamp.
 * - A frame of the same format and size reuses the preload frame: what the
 *   copy does not write (refs here) survives.
 * - A frame of another format, width or height destroys the preload frame
 *   and makes a new one of the new shape (fresh fields, new line sizes).
 * - async_mutex is free afterwards, and a later
 *   obs_source_show_preloaded_video() shows the preload frame with the audio
 *   timing restarted against its timestamp.
 * The worker's own NULL source and destroying checks can't be reached from
 * outside libobs: obs_source_preload_video() reads destroying() first (a
 * NULL source dereferences there, and a destroying source returns before
 * the worker). Tier 1 and Tier 3 cover them. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "util/platform.h"
#include "util/threading.h"

#define SOURCE_ID "test_source_preload_video_internal"

static const char *source_name(void *unused)
{
	(void)unused;
	return SOURCE_ID;
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void source_destroy(void *data)
{
	(void)data;
}

static int setup(void **state)
{
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	struct obs_source_info info = {
		.id = SOURCE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_ASYNC_VIDEO,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
	};
	obs_register_source(&info);
	*state = obs_source_create_private(SOURCE_ID, "preload video internal", NULL);
	return *state ? 0 : -1;
}

static int teardown(void **state)
{
	/* The source's destroy frees the preload frame. */
	obs_source_release(*state);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return 0;
}

/* Plane bytes: BGRA 2x2 uses plane 0 (8 a row), BGRA 4x2 and 2x4 use plane 0
 * (16 or 8 a row), I420 2x2 uses planes 0 (2 a row, 2 rows), 1 and 2 (1 a
 * row, 1 row). */
static uint8_t plane0[32] = {1,  2,  3,  4,  5,  6,  7,  8,  9,  10, 11, 12, 13, 14, 15, 16,
			     17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32};
static uint8_t plane1[4] = {41, 42, 43, 44};
static uint8_t plane2[4] = {51, 52, 53, 54};

static const float matrix[16] = {1.5f, 2.5f,  3.5f,  4.5f,  5.5f,  6.5f,  7.5f,  8.5f,
				 9.5f, 10.5f, 11.5f, 12.5f, 13.5f, 14.5f, 15.5f, 16.5f};

static struct obs_source_frame frame_of(enum video_format format, uint32_t width, uint32_t height, uint64_t timestamp)
{
	struct obs_source_frame frame = {
		.width = width,
		.height = height,
		.timestamp = timestamp,
		.format = format,
		.full_range = true,
		.max_luminance = 1000,
		.flip = true,
		.flags = OBS_SOURCE_FRAME_LINEAR_ALPHA,
		.trc = VIDEO_TRC_PQ,
		/* used internally by libobs: never copied */
		.refs = 77,
		.prev_frame = true,
	};
	frame.data[0] = plane0;
	if (format == VIDEO_FORMAT_I420) {
		frame.data[1] = plane1;
		frame.data[2] = plane2;
		frame.linesize[0] = width;
		frame.linesize[1] = width / 2;
		frame.linesize[2] = width / 2;
	} else {
		frame.linesize[0] = width * 4;
	}
	memcpy(frame.color_matrix, matrix, sizeof(matrix));
	return frame;
}

/* The preload frame holds a copy of `in` in buffers of its own, at the shape
 * obs_source_frame_create() gave it. */
static void expect_preloaded(obs_source_t *source, const struct obs_source_frame *in)
{
	const struct obs_source_frame *out = source->async_preload_frame;
	assert_non_null(out);
	assert_int_equal(out->format, in->format);
	assert_int_equal(out->width, in->width);
	assert_int_equal(out->height, in->height);
	assert_int_equal(out->timestamp, in->timestamp);
	assert_true(out->full_range);
	assert_int_equal(out->max_luminance, in->max_luminance);
	assert_int_equal(out->flip, in->flip);
	assert_int_equal(out->flags, in->flags);
	assert_int_equal(out->trc, in->trc);
	assert_memory_equal(out->color_matrix, matrix, sizeof(matrix));
	assert_false(out->prev_frame);

	assert_non_null(out->data[0]);
	assert_ptr_not_equal(out->data[0], in->data[0]);
	if (in->format == VIDEO_FORMAT_I420) {
		assert_int_equal(out->linesize[0], in->width);
		assert_int_equal(out->linesize[1], in->width / 2);
		assert_int_equal(out->linesize[2], in->width / 2);
		for (uint32_t y = 0; y < in->height; y++)
			assert_memory_equal(out->data[0] + y * out->linesize[0], plane0 + y * in->linesize[0],
					    in->width);
		assert_int_equal(out->data[1][0], plane1[0]);
		assert_int_equal(out->data[2][0], plane2[0]);
	} else {
		assert_int_equal(out->linesize[0], in->width * 4);
		for (uint32_t y = 0; y < in->height; y++)
			assert_memory_equal(out->data[0] + y * out->linesize[0], plane0 + y * in->linesize[0],
					    in->width * 4);
	}

	assert_int_equal(source->last_frame_ts, in->timestamp);
	assert_int_equal(source->async_frames.num, 0);
}

/* async_mutex is recursive, so probe it from another thread. */
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

static void null_frame_without_preload(void **state)
{
	obs_source_t *source = *state;
	source->last_frame_ts = 7;

	obs_source_preload_video(source, NULL);

	assert_null(source->async_preload_frame);
	assert_int_equal(source->last_frame_ts, 7);
}

static void null_frame_keeps_preload(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame bgra = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 100);
	obs_source_preload_video(source, &bgra);
	struct obs_source_frame *preload = source->async_preload_frame;
	preload->refs = 55;

	obs_source_preload_video(source, NULL);

	assert_ptr_equal(source->async_preload_frame, preload);
	assert_int_equal(preload->refs, 55);
	expect_preloaded(source, &bgra);
}

static void first_frame_creates_preload(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame bgra = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 100);
	const struct obs_source_frame saved = bgra;

	obs_source_preload_video(source, &bgra);

	expect_preloaded(source, &bgra);
	/* A fresh frame: obs_source_frame_create() zeroes what the copy skips. */
	assert_int_equal(source->async_preload_frame->refs, 0);
	assert_memory_equal(&bgra, &saved, sizeof(saved));
	expect_unlocked(&source->async_mutex);
}

static void same_shape_reuses_preload(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame first = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 100);
	obs_source_preload_video(source, &first);
	struct obs_source_frame *preload = source->async_preload_frame;
	uint8_t *data = preload->data[0];
	preload->refs = 55;

	struct obs_source_frame second = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 200);
	second.flip = false;
	second.flags = 0;
	second.max_luminance = 300;
	obs_source_preload_video(source, &second);

	assert_ptr_equal(source->async_preload_frame, preload);
	assert_ptr_equal(preload->data[0], data);
	assert_int_equal(preload->refs, 55);
	expect_preloaded(source, &second);
	expect_unlocked(&source->async_mutex);
}

/* Each changed shape gets a frame obs_source_frame_create() made at the new
 * shape: refs (set on the old one) is zero again. */
static void expect_recreated(obs_source_t *source, const struct obs_source_frame *in)
{
	expect_preloaded(source, in);
	assert_int_equal(source->async_preload_frame->refs, 0);
	source->async_preload_frame->refs = 55;
}

static void changed_shape_recreates_preload(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame bgra = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 100);
	obs_source_preload_video(source, &bgra);
	source->async_preload_frame->refs = 55;

	struct obs_source_frame i420 = frame_of(VIDEO_FORMAT_I420, 2, 2, 200);
	obs_source_preload_video(source, &i420);
	expect_recreated(source, &i420);

	obs_source_preload_video(source, &bgra);
	expect_recreated(source, &bgra);

	/* Only the width changes. */
	struct obs_source_frame wide = frame_of(VIDEO_FORMAT_BGRA, 4, 2, 300);
	obs_source_preload_video(source, &wide);
	expect_recreated(source, &wide);

	obs_source_preload_video(source, &bgra);
	expect_recreated(source, &bgra);

	/* Only the height changes. */
	struct obs_source_frame tall = frame_of(VIDEO_FORMAT_BGRA, 2, 4, 400);
	obs_source_preload_video(source, &tall);
	expect_recreated(source, &tall);
	expect_unlocked(&source->async_mutex);
}

/* The preload frame is what obs_source_show_preloaded_video() puts up,
 * with the audio timing restarted against the preloaded timestamp. */
static void show_preloaded_shows_it(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame bgra = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 5000000);
	obs_source_preload_video(source, &bgra);
	os_atomic_set_bool(&source->async_active, false);
	os_atomic_set_bool(&source->async_textures_stale, true);

	const uint64_t before = os_gettime_ns();
	obs_source_show_preloaded_video(source);
	const uint64_t after = os_gettime_ns();

	assert_true(os_atomic_load_bool(&source->async_active));
	assert_false(os_atomic_load_bool(&source->async_textures_stale));
	assert_true(source->async_flip);
	assert_true(source->async_linear_alpha);
	assert_int_equal(source->async_width, 2);
	assert_int_equal(source->async_height, 2);
	assert_int_equal(source->async_format, VIDEO_FORMAT_BGRA);
	assert_true(source->timing_set);
	const uint64_t sys_ts = source->timing_adjust + 5000000;
	assert_true(sys_ts >= before);
	assert_true(sys_ts <= after);
	expect_unlocked(&source->async_mutex);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(null_frame_without_preload, setup, teardown),
		cmocka_unit_test_setup_teardown(null_frame_keeps_preload, setup, teardown),
		cmocka_unit_test_setup_teardown(first_frame_creates_preload, setup, teardown),
		cmocka_unit_test_setup_teardown(same_shape_reuses_preload, setup, teardown),
		cmocka_unit_test_setup_teardown(changed_shape_recreates_preload, setup, teardown),
		cmocka_unit_test_setup_teardown(show_preloaded_shows_it, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
