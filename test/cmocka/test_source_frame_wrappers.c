/* Characterize the six async-frame entry points (#5760):
 * obs_source_output_video(), obs_source_preload_video(),
 * obs_source_set_video_frame() and their *2() variants, on a private async
 * source. Their workers are hidden in libobs, so each case checks what the
 * worker left behind: the queued frame (output), the preload frame (preload
 * and set), the source's clocks and flags.
 * - A source being destroyed returns at once, before the NULL check.
 * - A NULL frame reaches the worker as NULL: output clears the queue and the
 *   clocks; preload and set change nothing. (set_video_frame() with a NULL
 *   frame calls the preload worker, which no state here tells apart from the
 *   set worker; Tier 1 and Tier 3 pin it.)
 * - An obs_source_frame is copied whole with full_range replaced by
 *   obs_source_frame_copy_full_range(): a YUV frame keeps it, any other is
 *   full range.
 * - An obs_source_frame2 gives a frame of its planes, size, timestamp,
 *   format, flip, flags, transfer and colour matrix and range, with
 *   full_range from obs_source_frame_copy_full_range2() and max_luminance 0.
 * - The caller's frame is never written. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#define SOURCE_ID "test_source_frame_wrappers"

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
	*state = obs_source_create_private(SOURCE_ID, "frame wrappers", NULL);
	return *state ? 0 : -1;
}

static int teardown(void **state)
{
	obs_source_release(*state);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return 0;
}

/* Plane bytes for every case: BGRA 2x2 uses plane 0 (8 bytes a row), I420
 * 2x2 uses planes 0 (2 a row, 2 rows), 1 and 2 (1 a row, 1 row). */
static uint8_t plane0[16] = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16};
static uint8_t plane1[4] = {21, 22, 23, 24};
static uint8_t plane2[4] = {31, 32, 33, 34};

static const float matrix[16] = {1.5f, 2.5f,  3.5f,  4.5f,  5.5f,  6.5f,  7.5f,  8.5f,
				 9.5f, 10.5f, 11.5f, 12.5f, 13.5f, 14.5f, 15.5f, 16.5f};
static const float range_min[3] = {0.0625f, 0.125f, 0.25f};
static const float range_max[3] = {0.75f, 0.875f, 0.9375f};

static struct obs_source_frame frame_of(enum video_format format, bool full_range, uint64_t timestamp)
{
	struct obs_source_frame frame = {
		.width = 2,
		.height = 2,
		.timestamp = timestamp,
		.format = format,
		.full_range = full_range,
		.max_luminance = 1000,
		.flip = true,
		.flags = OBS_SOURCE_FRAME_LINEAR_ALPHA,
		.trc = VIDEO_TRC_PQ,
		/* used internally by libobs: whatever the caller left */
		.refs = 77,
		.prev_frame = true,
	};
	frame.data[0] = plane0;
	if (format == VIDEO_FORMAT_I420) {
		frame.data[1] = plane1;
		frame.data[2] = plane2;
		frame.linesize[0] = 2;
		frame.linesize[1] = 1;
		frame.linesize[2] = 1;
	} else {
		frame.linesize[0] = 8;
	}
	memcpy(frame.color_matrix, matrix, sizeof(matrix));
	memcpy(frame.color_range_min, range_min, sizeof(range_min));
	memcpy(frame.color_range_max, range_max, sizeof(range_max));
	return frame;
}

static struct obs_source_frame2 frame2_of(enum video_format format, enum video_range_type range, uint64_t timestamp)
{
	const struct obs_source_frame base = frame_of(format, false, timestamp);
	struct obs_source_frame2 frame = {
		.width = base.width,
		.height = base.height,
		.timestamp = base.timestamp,
		.format = format,
		.range = range,
		.flip = base.flip,
		.flags = base.flags,
		.trc = base.trc,
	};
	memcpy(frame.data, base.data, sizeof(frame.data));
	memcpy(frame.linesize, base.linesize, sizeof(frame.linesize));
	memcpy(frame.color_matrix, matrix, sizeof(matrix));
	memcpy(frame.color_range_min, range_min, sizeof(range_min));
	memcpy(frame.color_range_max, range_max, sizeof(range_max));
	return frame;
}

/* The frame a worker produced from `in`, with full_range and max_luminance
 * as given: obs_source_frame_copy() writes the metadata and the planes, and
 * the colour range only when the frame is not full range. */
static void expect_copied(const struct obs_source_frame *out, const struct obs_source_frame *in, bool full_range,
			  uint16_t max_luminance)
{
	assert_non_null(out);
	assert_int_equal(out->format, in->format);
	assert_int_equal(out->width, in->width);
	assert_int_equal(out->height, in->height);
	assert_int_equal(out->timestamp, in->timestamp);
	assert_int_equal(out->full_range, full_range);
	assert_int_equal(out->max_luminance, max_luminance);
	assert_int_equal(out->flip, in->flip);
	assert_int_equal(out->flags, in->flags);
	assert_int_equal(out->trc, in->trc);
	assert_memory_equal(out->color_matrix, matrix, sizeof(matrix));
	if (!full_range) {
		assert_memory_equal(out->color_range_min, range_min, sizeof(range_min));
		assert_memory_equal(out->color_range_max, range_max, sizeof(range_max));
	}
	if (in->format == VIDEO_FORMAT_I420) {
		assert_memory_equal(out->data[0], plane0, 2);
		assert_memory_equal(out->data[0] + out->linesize[0], plane0 + 2, 2);
		assert_int_equal(out->data[1][0], plane1[0]);
		assert_int_equal(out->data[2][0], plane2[0]);
	} else {
		assert_memory_equal(out->data[0], plane0, 8);
		assert_memory_equal(out->data[0] + out->linesize[0], plane0 + 8, 8);
	}
}

/* frame2_of()'s fields as the obs_source_frame they become. */
static struct obs_source_frame as_frame(const struct obs_source_frame2 *frame2)
{
	struct obs_source_frame frame = frame_of(frame2->format, false, frame2->timestamp);
	return frame;
}

/* The frame the last call queued. A new format or range resets the cache
 * and the queue first, so it is the only one then. */
static struct obs_source_frame *queued(obs_source_t *source)
{
	assert_true(source->async_frames.num > 0);
	return source->async_frames.array[source->async_frames.num - 1];
}

static void set_sentinels(obs_source_t *source)
{
	source->last_output_frame_ts = 9;
	source->last_frame_ts = 7;
	os_atomic_set_bool(&source->async_active, true);
	os_atomic_set_bool(&source->async_textures_stale, false);
}

static void expect_sentinels(obs_source_t *source)
{
	assert_int_equal(source->last_output_frame_ts, 9);
	assert_int_equal(source->last_frame_ts, 7);
	assert_true(os_atomic_load_bool(&source->async_active));
	assert_false(os_atomic_load_bool(&source->async_textures_stale));
}

/* A source being destroyed returns before anything, NULL frames included:
 * output with NULL would otherwise clear the clocks. */
static void destroying_returns_first(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame frame = frame_of(VIDEO_FORMAT_BGRA, false, 100);
	struct obs_source_frame2 frame2 = frame2_of(VIDEO_FORMAT_BGRA, VIDEO_RANGE_FULL, 200);
	set_sentinels(source);
	os_atomic_set_long(&source->destroying, 1);
	long allocs = bnum_allocs();

	obs_source_output_video(source, &frame);
	obs_source_output_video(source, NULL);
	obs_source_output_video2(source, &frame2);
	obs_source_output_video2(source, NULL);
	obs_source_preload_video(source, &frame);
	obs_source_preload_video(source, NULL);
	obs_source_preload_video2(source, &frame2);
	obs_source_preload_video2(source, NULL);
	obs_source_set_video_frame(source, &frame);
	obs_source_set_video_frame(source, NULL);
	obs_source_set_video_frame2(source, &frame2);
	obs_source_set_video_frame2(source, NULL);

	os_atomic_set_long(&source->destroying, 0);
	assert_int_equal(bnum_allocs(), allocs);
	assert_int_equal(source->async_frames.num, 0);
	assert_int_equal(source->async_cache.num, 0);
	assert_null(source->async_preload_frame);
	expect_sentinels(source);
}

/* A NULL frame reaches the output worker, which clears the queue, the cache
 * and both clocks and marks the textures stale. */
static void output_null_frame_clears(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame frame = frame_of(VIDEO_FORMAT_BGRA, false, 100);

	obs_source_output_video(source, &frame);
	assert_int_equal(source->async_frames.num, 1);
	set_sentinels(source);
	obs_source_output_video(source, NULL);
	assert_int_equal(source->async_frames.num, 0);
	assert_int_equal(source->async_cache.num, 0);
	assert_int_equal(source->last_output_frame_ts, 0);
	assert_int_equal(source->last_frame_ts, 0);
	assert_false(os_atomic_load_bool(&source->async_active));
	assert_true(os_atomic_load_bool(&source->async_textures_stale));

	obs_source_output_video(source, &frame);
	set_sentinels(source);
	obs_source_output_video2(source, NULL);
	assert_int_equal(source->async_frames.num, 0);
	assert_int_equal(source->async_cache.num, 0);
	assert_int_equal(source->last_output_frame_ts, 0);
	assert_int_equal(source->last_frame_ts, 0);
	assert_false(os_atomic_load_bool(&source->async_active));
	assert_true(os_atomic_load_bool(&source->async_textures_stale));
}

/* A NULL frame changes nothing through preload or set (both reach the
 * preload worker, which returns). */
static void preload_and_set_null_frame_do_nothing(void **state)
{
	obs_source_t *source = *state;
	set_sentinels(source);
	long allocs = bnum_allocs();

	obs_source_preload_video(source, NULL);
	obs_source_preload_video2(source, NULL);
	obs_source_set_video_frame(source, NULL);
	obs_source_set_video_frame2(source, NULL);

	assert_int_equal(bnum_allocs(), allocs);
	assert_null(source->async_preload_frame);
	assert_int_equal(source->async_frames.num, 0);
	expect_sentinels(source);
}

/* obs_source_output_video(): the whole frame, full_range replaced; YUV keeps
 * it, BGRA becomes full range. The caller's frame is not written. */
static void output_video_copies_frame(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame bgra = frame_of(VIDEO_FORMAT_BGRA, false, 100);
	struct obs_source_frame i420 = frame_of(VIDEO_FORMAT_I420, false, 200);
	struct obs_source_frame i420_full = frame_of(VIDEO_FORMAT_I420, true, 300);
	const struct obs_source_frame saved[3] = {bgra, i420, i420_full};

	obs_source_output_video(source, &bgra);
	expect_copied(queued(source), &bgra, true, 1000);
	assert_int_equal(source->last_output_frame_ts, 100);
	assert_true(os_atomic_load_bool(&source->async_active));

	obs_source_output_video(source, &i420);
	expect_copied(queued(source), &i420, false, 1000);
	obs_source_output_video(source, &i420_full);
	expect_copied(queued(source), &i420_full, true, 1000);
	assert_int_equal(source->last_output_frame_ts, 300);

	assert_memory_equal(&bgra, &saved[0], sizeof(bgra));
	assert_memory_equal(&i420, &saved[1], sizeof(i420));
	assert_memory_equal(&i420_full, &saved[2], sizeof(i420_full));
}

/* obs_source_output_video2(): full_range from the range (default resolves
 * to partial for YUV, full otherwise), max_luminance 0. */
static void output_video2_builds_frame(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame2 bgra_partial = frame2_of(VIDEO_FORMAT_BGRA, VIDEO_RANGE_PARTIAL, 100);
	struct obs_source_frame2 bgra_default = frame2_of(VIDEO_FORMAT_BGRA, VIDEO_RANGE_DEFAULT, 200);
	struct obs_source_frame2 i420_default = frame2_of(VIDEO_FORMAT_I420, VIDEO_RANGE_DEFAULT, 300);
	struct obs_source_frame2 i420_full = frame2_of(VIDEO_FORMAT_I420, VIDEO_RANGE_FULL, 400);
	const struct obs_source_frame2 saved = bgra_partial;

	obs_source_output_video2(source, &bgra_partial);
	struct obs_source_frame expected = as_frame(&bgra_partial);
	expect_copied(queued(source), &expected, false, 0);
	assert_int_equal(source->last_output_frame_ts, 100);

	obs_source_output_video2(source, &bgra_default);
	expected = as_frame(&bgra_default);
	expect_copied(queued(source), &expected, true, 0);
	obs_source_output_video2(source, &i420_default);
	expected = as_frame(&i420_default);
	expect_copied(queued(source), &expected, false, 0);
	obs_source_output_video2(source, &i420_full);
	expected = as_frame(&i420_full);
	expect_copied(queued(source), &expected, true, 0);
	assert_int_equal(source->last_output_frame_ts, 400);

	assert_memory_equal(&bgra_partial, &saved, sizeof(saved));
}

/* The preload pair fills the preload frame and the frame clock, and queues
 * nothing. The *2() variant writes max_luminance 0 over the frame the first
 * call left at 1000. */
static void preload_video_copies_frame(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame i420 = frame_of(VIDEO_FORMAT_I420, false, 100);
	struct obs_source_frame2 i420_full = frame2_of(VIDEO_FORMAT_I420, VIDEO_RANGE_FULL, 200);
	const struct obs_source_frame saved = i420;
	const struct obs_source_frame2 saved2 = i420_full;

	obs_source_preload_video(source, &i420);
	expect_copied(source->async_preload_frame, &i420, false, 1000);
	assert_int_equal(source->last_frame_ts, 100);

	struct obs_source_frame *preload = source->async_preload_frame;
	obs_source_preload_video2(source, &i420_full);
	struct obs_source_frame expected = as_frame(&i420_full);
	/* Same shape: the same preload frame, rewritten. */
	assert_ptr_equal(source->async_preload_frame, preload);
	expect_copied(source->async_preload_frame, &expected, true, 0);
	assert_int_equal(source->last_frame_ts, 200);

	struct obs_source_frame bgra = frame_of(VIDEO_FORMAT_BGRA, false, 300);
	obs_source_preload_video(source, &bgra);
	expect_copied(source->async_preload_frame, &bgra, true, 1000);
	assert_int_equal(source->last_frame_ts, 300);

	assert_int_equal(source->async_frames.num, 0);
	assert_memory_equal(&i420, &saved, sizeof(saved));
	assert_memory_equal(&i420_full, &saved2, sizeof(saved2));
}

/* The set pair fills the preload frame, the async texture description and
 * the frame clock right away (no graphics here, so no texture is made). */
static void set_video_frame_copies_frame(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame bgra = frame_of(VIDEO_FORMAT_BGRA, false, 100);
	struct obs_source_frame2 bgra_full = frame2_of(VIDEO_FORMAT_BGRA, VIDEO_RANGE_FULL, 200);
	const struct obs_source_frame saved = bgra;
	const struct obs_source_frame2 saved2 = bgra_full;
	os_atomic_set_bool(&source->async_textures_stale, true);

	obs_source_set_video_frame(source, &bgra);
	expect_copied(source->async_preload_frame, &bgra, true, 1000);
	assert_int_equal(source->last_frame_ts, 100);
	assert_int_equal(source->async_width, 2);
	assert_int_equal(source->async_height, 2);
	assert_int_equal(source->async_format, VIDEO_FORMAT_BGRA);
	assert_true(source->async_full_range);
	assert_int_equal(source->async_trc, VIDEO_TRC_PQ);
	assert_true(source->async_flip);
	assert_false(os_atomic_load_bool(&source->async_textures_stale));

	os_atomic_set_bool(&source->async_textures_stale, true);
	obs_source_set_video_frame2(source, &bgra_full);
	struct obs_source_frame expected = as_frame(&bgra_full);
	expect_copied(source->async_preload_frame, &expected, true, 0);
	assert_int_equal(source->last_frame_ts, 200);
	assert_false(os_atomic_load_bool(&source->async_textures_stale));

	assert_int_equal(source->async_frames.num, 0);
	assert_memory_equal(&bgra, &saved, sizeof(saved));
	assert_memory_equal(&bgra_full, &saved2, sizeof(saved2));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(destroying_returns_first, setup, teardown),
		cmocka_unit_test_setup_teardown(output_null_frame_clears, setup, teardown),
		cmocka_unit_test_setup_teardown(preload_and_set_null_frame_do_nothing, setup, teardown),
		cmocka_unit_test_setup_teardown(output_video_copies_frame, setup, teardown),
		cmocka_unit_test_setup_teardown(output_video2_builds_frame, setup, teardown),
		cmocka_unit_test_setup_teardown(preload_video_copies_frame, setup, teardown),
		cmocka_unit_test_setup_teardown(set_video_frame_copies_frame, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
