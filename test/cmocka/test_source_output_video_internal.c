/* Characterize obs_source_output_video_internal() (#5841), the hidden worker
 * every obs_source_output_video*() entry ends in, through the public
 * obs_source_output_video() and obs_source_output_video2() on a private
 * async source under a started core without video (nothing ticks, so what
 * the worker queues stays queued and can be seen):
 * - last_output_frame_ts is the frame's timestamp, or 0 for a NULL frame;
 *   last_frame_ts is left alone by a frame.
 * - A frame goes through the async frame cache (cache_video()): a cache
 *   entry, used, holds a copy of it (the planes in buffers of its own, the
 *   metadata, prev_frame cleared); the copy is pushed onto async_frames with
 *   the cache's reference only (refs 1) and async_active is set.
 * - A second frame of the same shape takes a new entry while the first is
 *   still used; an entry the render gave back is reused, the same frame.
 * - A frame of another convert type or size resets the cache: the queued
 *   frames and the old entries are dropped, and the new frame is queued.
 * - A NULL frame clears async_active, marks the textures stale, zeroes
 *   last_frame_ts and drops the queue and the cache.
 * - With 30 frames queued (MAX_ASYNC_FRAMES), the next frame drops the queue
 *   and the cache and queues nothing; async_active stays set and
 *   last_frame_ts stays.
 * - async_mutex and audio_buf_mutex are free afterwards.
 * The worker's own NULL source check can't be reached from outside libobs
 * (obs_source_output_video() reads destroying() first, so a NULL source
 * dereferences there), nor can its refs-reach-0 branch (it needs the cache
 * freed between cache_video() and the queueing); Tier 1 and Tier 3 cover
 * them. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "util/platform.h"
#include "util/threading.h"

#define SOURCE_ID "test_source_output_video_internal"

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
	obs_source_t *source = obs_source_create_private(SOURCE_ID, "output video internal", NULL);
	if (!source)
		return -1;
	*state = source;
	return 0;
}

static int teardown(void **state)
{
	/* The source's destroy frees the cache and the queue. */
	obs_source_release(*state);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return 0;
}

/* Plane bytes: BGRA 2x2 uses plane 0 (8 a row), BGRA 4x2 plane 0 (16 a
 * row), I420 2x2 planes 0 (2 a row, 2 rows), 1 and 2 (1 a row, 1 row). */
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

/* The mutexes are recursive, so probe them from another thread. */
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

static void expect_free(obs_source_t *source)
{
	expect_unlocked(&source->async_mutex);
	expect_unlocked(&source->audio_buf_mutex);
}

/* `out` holds a copy of `in` in buffers of its own. */
static void expect_copy(const struct obs_source_frame *out, const struct obs_source_frame *in)
{
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
		for (uint32_t y = 0; y < in->height; y++)
			assert_memory_equal(out->data[0] + y * out->linesize[0], plane0 + y * in->linesize[0],
					    in->width);
		assert_int_equal(out->data[1][0], plane1[0]);
		assert_int_equal(out->data[2][0], plane2[0]);
	} else {
		for (uint32_t y = 0; y < in->height; y++)
			assert_memory_equal(out->data[0] + y * out->linesize[0], plane0 + y * in->linesize[0],
					    in->width * 4);
	}
}

/* The queue's newest frame is the copy of `in`, held by a used cache entry
 * with the cache's reference only. */
static struct obs_source_frame *expect_queued(obs_source_t *source, const struct obs_source_frame *in, size_t queued,
					      size_t cached)
{
	assert_int_equal(source->async_frames.num, queued);
	assert_int_equal(source->async_cache.num, cached);
	struct obs_source_frame *out = source->async_frames.array[queued - 1];
	expect_copy(out, in);
	assert_int_equal(out->refs, 1);

	bool held = false;
	for (size_t i = 0; i < source->async_cache.num; i++) {
		if (source->async_cache.array[i].frame == out) {
			assert_true(source->async_cache.array[i].used);
			held = true;
		}
	}
	assert_true(held);

	assert_true(os_atomic_load_bool(&source->async_active));
	assert_int_equal(source->last_output_frame_ts, in->timestamp);
	assert_int_equal(source->async_cache_width, in->width);
	assert_int_equal(source->async_cache_height, in->height);
	assert_int_equal(source->async_cache_format, in->format);
	expect_free(source);
	return out;
}

static void frame_is_queued(void **state)
{
	obs_source_t *source = *state;
	source->last_frame_ts = 7;
	os_atomic_set_bool(&source->async_textures_stale, false);
	struct obs_source_frame bgra = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 100);
	const struct obs_source_frame saved = bgra;

	obs_source_output_video(source, &bgra);

	expect_queued(source, &bgra, 1, 1);
	/* The frame touches neither last_frame_ts nor the stale flag. */
	assert_int_equal(source->last_frame_ts, 7);
	assert_false(os_atomic_load_bool(&source->async_textures_stale));
	assert_memory_equal(&bgra, &saved, sizeof(saved));
}

static void same_shape_takes_a_new_entry(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame first = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 100);
	obs_source_output_video(source, &first);
	struct obs_source_frame *queued = source->async_frames.array[0];

	struct obs_source_frame second = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 200);
	second.flip = false;
	second.flags = 0;
	obs_source_output_video(source, &second);

	struct obs_source_frame *out = expect_queued(source, &second, 2, 2);
	assert_ptr_not_equal(out, queued);
	assert_ptr_equal(source->async_frames.array[0], queued);
	assert_int_equal(queued->timestamp, 100);
}

static void given_back_entry_is_reused(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame first = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 100);
	obs_source_output_video(source, &first);
	struct obs_source_frame *queued = source->async_frames.array[0];
	uint8_t *data = queued->data[0];

	/* What remove_async_frame() does once the render is done with it. */
	pthread_mutex_lock(&source->async_mutex);
	da_erase(source->async_frames, 0);
	source->async_cache.array[0].used = false;
	queued->prev_frame = true;
	pthread_mutex_unlock(&source->async_mutex);

	struct obs_source_frame second = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 200);
	obs_source_output_video(source, &second);

	struct obs_source_frame *out = expect_queued(source, &second, 1, 1);
	assert_ptr_equal(out, queued);
	assert_ptr_equal(out->data[0], data);
	assert_int_equal(source->async_cache.array[0].unused_count, 0);
}

static void changed_shape_resets_the_cache(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame bgra = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 100);
	obs_source_output_video(source, &bgra);
	obs_source_output_video(source, &bgra);
	assert_int_equal(source->async_frames.num, 2);

	/* Another convert type. */
	struct obs_source_frame i420 = frame_of(VIDEO_FORMAT_I420, 2, 2, 200);
	obs_source_output_video(source, &i420);
	expect_queued(source, &i420, 1, 1);

	/* Another width. */
	struct obs_source_frame wide = frame_of(VIDEO_FORMAT_BGRA, 4, 2, 300);
	obs_source_output_video(source, &wide);
	expect_queued(source, &wide, 1, 1);

	/* Another height. */
	struct obs_source_frame tall = frame_of(VIDEO_FORMAT_BGRA, 4, 1, 400);
	obs_source_output_video(source, &tall);
	expect_queued(source, &tall, 1, 1);
}

static void null_frame_clears_the_queue(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame bgra = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 100);
	obs_source_output_video(source, &bgra);
	obs_source_output_video(source, &bgra);
	source->last_frame_ts = 7;
	source->cur_async_frame = source->async_frames.array[0];
	source->prev_async_frame = source->async_frames.array[1];
	os_atomic_set_bool(&source->async_textures_stale, false);

	obs_source_output_video(source, NULL);

	assert_false(os_atomic_load_bool(&source->async_active));
	assert_true(os_atomic_load_bool(&source->async_textures_stale));
	assert_int_equal(source->last_frame_ts, 0);
	assert_int_equal(source->last_output_frame_ts, 0);
	assert_int_equal(source->async_frames.num, 0);
	assert_int_equal(source->async_cache.num, 0);
	assert_null(source->cur_async_frame);
	assert_null(source->prev_async_frame);
	expect_free(source);

	/* A frame after that is queued again. */
	obs_source_output_video(source, &bgra);
	expect_queued(source, &bgra, 1, 1);
}

static void null_frame_on_an_idle_source(void **state)
{
	obs_source_t *source = *state;
	source->last_output_frame_ts = 9;
	source->last_frame_ts = 7;

	obs_source_output_video2(source, NULL);

	assert_false(os_atomic_load_bool(&source->async_active));
	assert_true(os_atomic_load_bool(&source->async_textures_stale));
	assert_int_equal(source->last_frame_ts, 0);
	assert_int_equal(source->last_output_frame_ts, 0);
	assert_int_equal(source->async_frames.num, 0);
	assert_int_equal(source->async_cache.num, 0);
	expect_free(source);
}

static void overflow_drops_the_queue(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame bgra = frame_of(VIDEO_FORMAT_BGRA, 2, 2, 0);
	for (uint64_t i = 1; i <= 30; i++) {
		bgra.timestamp = i * 10;
		obs_source_output_video(source, &bgra);
	}
	expect_queued(source, &bgra, 30, 30);
	source->last_frame_ts = 7;
	os_atomic_set_bool(&source->async_textures_stale, false);

	bgra.timestamp = 310;
	obs_source_output_video(source, &bgra);

	assert_int_equal(source->async_frames.num, 0);
	assert_int_equal(source->async_cache.num, 0);
	assert_int_equal(source->last_output_frame_ts, 310);
	/* Nothing more: the running clock and the flags stay. */
	assert_true(os_atomic_load_bool(&source->async_active));
	assert_int_equal(source->last_frame_ts, 7);
	assert_false(os_atomic_load_bool(&source->async_textures_stale));
	expect_free(source);

	/* The next frame is queued again. */
	bgra.timestamp = 320;
	obs_source_output_video(source, &bgra);
	expect_queued(source, &bgra, 1, 1);
}

/* obs_source_output_video2() ends in the same worker. */
static void frame2_ends_in_the_worker(void **state)
{
	obs_source_t *source = *state;
	struct obs_source_frame2 frame = {
		.width = 2,
		.height = 2,
		.timestamp = 500,
		.format = VIDEO_FORMAT_BGRA,
		.range = VIDEO_RANGE_DEFAULT,
		.flip = true,
		.flags = OBS_SOURCE_FRAME_LINEAR_ALPHA,
		.trc = VIDEO_TRC_PQ,
	};
	frame.data[0] = plane0;
	frame.linesize[0] = 8;
	memcpy(frame.color_matrix, matrix, sizeof(matrix));

	obs_source_output_video2(source, &frame);

	assert_int_equal(source->async_frames.num, 1);
	const struct obs_source_frame *out = source->async_frames.array[0];
	assert_int_equal(out->format, VIDEO_FORMAT_BGRA);
	assert_int_equal(out->timestamp, 500);
	assert_int_equal(out->refs, 1);
	assert_memory_equal(out->data[0], plane0, 16);
	assert_true(os_atomic_load_bool(&source->async_active));
	assert_int_equal(source->last_output_frame_ts, 500);
	expect_free(source);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(frame_is_queued, setup, teardown),
		cmocka_unit_test_setup_teardown(same_shape_takes_a_new_entry, setup, teardown),
		cmocka_unit_test_setup_teardown(given_back_entry_is_reused, setup, teardown),
		cmocka_unit_test_setup_teardown(changed_shape_resets_the_cache, setup, teardown),
		cmocka_unit_test_setup_teardown(null_frame_clears_the_queue, setup, teardown),
		cmocka_unit_test_setup_teardown(null_frame_on_an_idle_source, setup, teardown),
		cmocka_unit_test_setup_teardown(overflow_drops_the_queue, setup, teardown),
		cmocka_unit_test_setup_teardown(frame2_ends_in_the_worker, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
