/*
 * The deinterlace seam (libobs/obs-source-deinterlace-seam.h,
 * obs-rust/obs-studio#4472), which the port of the rest of
 * libobs/obs-source-deinterlace.c calls in place of the struct reads,
 * writes, locks, container calls and inline it wraps:
 * - the async queue: the count of async_frames, the frame at an index,
 *   da_erase() of the first, and the lock and unlock of async_mutex;
 * - the clock fields: async_unbuffered, last_sys_timestamp, last_frame_ts,
 *   deinterlace_offset, deinterlace_frame_ts and deinterlace_half_duration;
 * - the frame slots: cur_async_frame and prev_async_frame;
 * - the texture fields: async_format, async_trc, async_width, async_height,
 *   async_flip, and the slots of async_textures, async_prev_textures,
 *   async_texrender and async_prev_texrender;
 * - the deinterlace fields: deinterlace_rendered, deinterlace_effect,
 *   deinterlace_mode and deinterlace_top_first;
 * - convert_video_space(), the obs-internal.h inline;
 * - the core video: video_frame_interval_ns, the address of the video stats
 *   and the eight deinterlace effect slots.
 *
 * Each read is checked against the field it wraps, each write is read back
 * from the field, each lock and unlock returns 0 and holds (or lets go of)
 * async_mutex, the queue calls leave the array as da_erase() leaves a copy
 * of it, each slot is the field's own address (a write through it is the
 * field), and the inline wrapper is checked against the inline for every
 * video format and transfer.
 *
 * The accessors are hidden in libobs, so this test compiles the seam into
 * itself and runs it on a stand-in core (`obs`, with only the fields the seam
 * reads) and stand-in sources: zeroed structs with async_mutex initialized.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <string.h>

#include "obs-internal.h"
#include "obs-source-deinterlace-seam.h"

/* libobs has this one in obs.c, hidden; the seam in this test reads this
 * stand-in. */
struct obs_core *obs;

static int tag[MAX_AV_PLANES + 2];
static int other_tag[MAX_AV_PLANES + 2];

static obs_source_t *new_source(void)
{
	obs_source_t *source = bzalloc(sizeof(*source));
	assert_int_equal(pthread_mutex_init(&source->async_mutex, NULL), 0);
	return source;
}

static void free_source(obs_source_t *source)
{
	da_free(source->async_frames);
	pthread_mutex_destroy(&source->async_mutex);
	bfree(source);
}

static int setup(void **state)
{
	obs = bzalloc(sizeof(*obs));
	(void)state;
	return 0;
}

static int teardown(void **state)
{
	bfree(obs);
	obs = NULL;
	(void)state;
	return 0;
}

static void test_async_queue(void **state)
{
	(void)state;
	obs_source_t *source = new_source();
	struct obs_source_frame frames[4];
	DARRAY(struct obs_source_frame *) expected;

	memset(frames, 0, sizeof(frames));
	da_init(expected);
	for (size_t i = 0; i < 4; i++) {
		frames[i].timestamp = 1000 + i;
		da_push_back(source->async_frames, &(struct obs_source_frame *){&frames[i]});
		da_push_back(expected, &(struct obs_source_frame *){&frames[i]});
	}

	assert_int_equal(obs_source_deinterlace_seam_get_async_frames_num(source), 4);
	for (size_t i = 0; i < 4; i++)
		assert_ptr_equal(obs_source_deinterlace_seam_get_async_frame(source, i), &frames[i]);

	/* Each erase of the first leaves the array as da_erase() at 0 leaves
	 * a copy of it. */
	while (expected.num) {
		obs_source_deinterlace_seam_erase_async_frame_front(source);
		da_erase(expected, 0);

		assert_int_equal(source->async_frames.num, expected.num);
		assert_int_equal(obs_source_deinterlace_seam_get_async_frames_num(source), expected.num);
		for (size_t i = 0; i < expected.num; i++) {
			assert_ptr_equal(source->async_frames.array[i], expected.array[i]);
			assert_ptr_equal(obs_source_deinterlace_seam_get_async_frame(source, i), expected.array[i]);
		}
	}
	assert_int_equal(obs_source_deinterlace_seam_get_async_frames_num(source), 0);

	da_free(expected);
	free_source(source);
}

static void test_async_mutex(void **state)
{
	(void)state;
	obs_source_t *source = new_source();

	/* A lock holds the mutex (a trylock from this thread is refused) and
	 * its unlock lets it go (a trylock then succeeds). */
	assert_int_equal(obs_source_deinterlace_seam_async_mutex_lock(source), 0);
	assert_int_not_equal(pthread_mutex_trylock(&source->async_mutex), 0);
	assert_int_equal(obs_source_deinterlace_seam_async_mutex_unlock(source), 0);
	assert_int_equal(pthread_mutex_trylock(&source->async_mutex), 0);
	assert_int_equal(pthread_mutex_unlock(&source->async_mutex), 0);

	free_source(source);
}

static void test_clock_fields(void **state)
{
	(void)state;
	obs_source_t *source = new_source();

	source->async_unbuffered = true;
	source->last_sys_timestamp = 0x8000000000000001ULL;
	source->last_frame_ts = 0x7000000000000002ULL;
	source->deinterlace_offset = 0x6000000000000003ULL;
	source->deinterlace_frame_ts = 0x5000000000000004ULL;
	source->deinterlace_half_duration = 0x80000005U;

	assert_true(obs_source_deinterlace_seam_get_async_unbuffered(source));
	assert_true(obs_source_deinterlace_seam_get_last_sys_timestamp(source) == 0x8000000000000001ULL);
	assert_true(obs_source_deinterlace_seam_get_last_frame_ts(source) == 0x7000000000000002ULL);
	assert_true(obs_source_deinterlace_seam_get_deinterlace_offset(source) == 0x6000000000000003ULL);
	assert_true(obs_source_deinterlace_seam_get_deinterlace_frame_ts(source) == 0x5000000000000004ULL);
	assert_true(obs_source_deinterlace_seam_get_deinterlace_half_duration(source) == 0x80000005U);

	source->async_unbuffered = false;
	assert_false(obs_source_deinterlace_seam_get_async_unbuffered(source));

	/* Each set is read back from the field, and touches no other. */
	obs_source_deinterlace_seam_set_last_frame_ts(source, 0x1000000000000011ULL);
	assert_true(source->last_frame_ts == 0x1000000000000011ULL);
	obs_source_deinterlace_seam_set_deinterlace_offset(source, 0x1000000000000012ULL);
	assert_true(source->deinterlace_offset == 0x1000000000000012ULL);
	obs_source_deinterlace_seam_set_deinterlace_frame_ts(source, 0x1000000000000013ULL);
	assert_true(source->deinterlace_frame_ts == 0x1000000000000013ULL);
	obs_source_deinterlace_seam_set_deinterlace_half_duration(source, 0x10000014U);
	assert_true(source->deinterlace_half_duration == 0x10000014U);

	assert_true(source->last_sys_timestamp == 0x8000000000000001ULL);
	assert_true(source->last_frame_ts == 0x1000000000000011ULL);
	assert_true(source->deinterlace_offset == 0x1000000000000012ULL);
	assert_true(source->deinterlace_frame_ts == 0x1000000000000013ULL);

	obs_source_deinterlace_seam_set_last_frame_ts(source, 0);
	obs_source_deinterlace_seam_set_deinterlace_offset(source, 0);
	obs_source_deinterlace_seam_set_deinterlace_frame_ts(source, 0);
	obs_source_deinterlace_seam_set_deinterlace_half_duration(source, 0);
	assert_true(source->last_frame_ts == 0);
	assert_true(source->deinterlace_offset == 0);
	assert_true(source->deinterlace_frame_ts == 0);
	assert_true(source->deinterlace_half_duration == 0);

	free_source(source);
}

static void test_frame_slots(void **state)
{
	(void)state;
	obs_source_t *source = new_source();
	struct obs_source_frame cur, prev;

	source->cur_async_frame = &cur;
	source->prev_async_frame = &prev;
	assert_ptr_equal(obs_source_deinterlace_seam_get_cur_async_frame(source), &cur);
	assert_ptr_equal(obs_source_deinterlace_seam_get_prev_async_frame(source), &prev);

	/* The take's move of the current frame to the previous slot. */
	obs_source_deinterlace_seam_set_prev_async_frame(source,
							 obs_source_deinterlace_seam_get_cur_async_frame(source));
	assert_ptr_equal(source->prev_async_frame, &cur);
	obs_source_deinterlace_seam_set_cur_async_frame(source, &prev);
	assert_ptr_equal(source->cur_async_frame, &prev);

	obs_source_deinterlace_seam_set_cur_async_frame(source, NULL);
	obs_source_deinterlace_seam_set_prev_async_frame(source, NULL);
	assert_null(source->cur_async_frame);
	assert_null(source->prev_async_frame);
	assert_null(obs_source_deinterlace_seam_get_cur_async_frame(source));
	assert_null(obs_source_deinterlace_seam_get_prev_async_frame(source));

	free_source(source);
}

static void test_texture_fields(void **state)
{
	(void)state;
	obs_source_t *source = new_source();

	source->async_format = VIDEO_FORMAT_P010;
	source->async_trc = VIDEO_TRC_HLG;
	source->async_width = 1920;
	source->async_height = 1081;
	source->async_flip = true;
	assert_int_equal(obs_source_deinterlace_seam_get_async_format(source), VIDEO_FORMAT_P010);
	assert_int_equal(obs_source_deinterlace_seam_get_async_trc(source), VIDEO_TRC_HLG);
	assert_int_equal(obs_source_deinterlace_seam_get_async_width(source), 1920);
	assert_int_equal(obs_source_deinterlace_seam_get_async_height(source), 1081);
	assert_true(obs_source_deinterlace_seam_get_async_flip(source));
	source->async_flip = false;
	assert_false(obs_source_deinterlace_seam_get_async_flip(source));

	/* Each slot is the field's own storage. */
	gs_texture_t **textures = obs_source_deinterlace_seam_get_async_textures(source);
	gs_texture_t **prev_textures = obs_source_deinterlace_seam_get_async_prev_textures(source);
	gs_texrender_t **texrender = obs_source_deinterlace_seam_get_async_texrender_slot(source);
	gs_texrender_t **prev_texrender = obs_source_deinterlace_seam_get_async_prev_texrender_slot(source);
	assert_ptr_equal(textures, &source->async_textures[0]);
	assert_ptr_equal(prev_textures, &source->async_prev_textures[0]);
	assert_ptr_equal(texrender, &source->async_texrender);
	assert_ptr_equal(prev_texrender, &source->async_prev_texrender);

	for (size_t c = 0; c < MAX_AV_PLANES; c++) {
		source->async_textures[c] = (gs_texture_t *)&tag[c];
		source->async_prev_textures[c] = (gs_texture_t *)&other_tag[c];
	}
	source->async_texrender = (gs_texrender_t *)&tag[MAX_AV_PLANES];
	source->async_prev_texrender = (gs_texrender_t *)&other_tag[MAX_AV_PLANES];

	/* deinterlace_update_async_video()'s swap, through the slots. */
	for (size_t c = 0; c < MAX_AV_PLANES; c++) {
		gs_texture_t *prev_tex = prev_textures[c];
		prev_textures[c] = textures[c];
		textures[c] = prev_tex;
	}
	gs_texrender_t *prev = *prev_texrender;
	*prev_texrender = *texrender;
	*texrender = prev;

	for (size_t c = 0; c < MAX_AV_PLANES; c++) {
		assert_ptr_equal(source->async_textures[c], &other_tag[c]);
		assert_ptr_equal(source->async_prev_textures[c], &tag[c]);
	}
	assert_ptr_equal(source->async_texrender, &other_tag[MAX_AV_PLANES]);
	assert_ptr_equal(source->async_prev_texrender, &tag[MAX_AV_PLANES]);

	/* disable_deinterlacing()'s clear, through the slots. */
	for (size_t c = 0; c < 3; c++)
		prev_textures[c] = NULL;
	*prev_texrender = NULL;
	for (size_t c = 0; c < 3; c++)
		assert_null(source->async_prev_textures[c]);
	assert_null(source->async_prev_texrender);
	assert_ptr_equal(source->async_texrender, &other_tag[MAX_AV_PLANES]);

	free_source(source);
}

static void test_deinterlace_fields(void **state)
{
	(void)state;
	obs_source_t *source = new_source();

	source->deinterlace_rendered = true;
	source->deinterlace_effect = (gs_effect_t *)&tag[0];
	assert_true(obs_source_deinterlace_seam_get_deinterlace_rendered(source));
	assert_ptr_equal(obs_source_deinterlace_seam_get_deinterlace_effect(source), &tag[0]);

	obs_source_deinterlace_seam_set_deinterlace_rendered(source, false);
	assert_false(source->deinterlace_rendered);
	assert_false(obs_source_deinterlace_seam_get_deinterlace_rendered(source));
	obs_source_deinterlace_seam_set_deinterlace_rendered(source, true);
	assert_true(source->deinterlace_rendered);

	obs_source_deinterlace_seam_set_deinterlace_effect(source, (gs_effect_t *)&other_tag[0]);
	assert_ptr_equal(source->deinterlace_effect, &other_tag[0]);
	obs_source_deinterlace_seam_set_deinterlace_effect(source, NULL);
	assert_null(source->deinterlace_effect);

	for (int mode = OBS_DEINTERLACE_MODE_DISABLE; mode <= OBS_DEINTERLACE_MODE_YADIF_2X; mode++) {
		obs_source_deinterlace_seam_set_deinterlace_mode(source, (enum obs_deinterlace_mode)mode);
		assert_int_equal((int)source->deinterlace_mode, mode);
	}
	obs_source_deinterlace_seam_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_BLEND);
	assert_int_equal((int)source->deinterlace_mode, (int)OBS_DEINTERLACE_MODE_BLEND);
	obs_source_deinterlace_seam_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_DISABLE);
	assert_int_equal((int)source->deinterlace_mode, (int)OBS_DEINTERLACE_MODE_DISABLE);

	obs_source_deinterlace_seam_set_deinterlace_top_first(source, true);
	assert_true(source->deinterlace_top_first);
	obs_source_deinterlace_seam_set_deinterlace_top_first(source, false);
	assert_false(source->deinterlace_top_first);

	free_source(source);
}

static void test_convert_video_space(void **state)
{
	(void)state;
	obs_source_t *source = new_source();
	size_t not_srgb = 0;

	for (int format = VIDEO_FORMAT_NONE; format <= VIDEO_FORMAT_R10L; format++) {
		for (int trc = VIDEO_TRC_DEFAULT; trc <= VIDEO_TRC_HLG; trc++) {
			const enum gs_color_space want =
				convert_video_space((enum video_format)format, (enum video_trc)trc);
			assert_int_equal(obs_source_deinterlace_seam_convert_video_space((enum video_format)format,
											 (enum video_trc)trc),
					 want);
			if (want != GS_CS_SRGB)
				not_srgb++;

			/* As deinterlace_render() calls it: on the source's
			 * fields, async_trc a uint8_t. */
			source->async_format = (enum video_format)format;
			source->async_trc = (uint8_t)trc;
			assert_int_equal(obs_source_deinterlace_seam_convert_video_space(
						 obs_source_deinterlace_seam_get_async_format(source),
						 obs_source_deinterlace_seam_get_async_trc(source)),
					 convert_video_space(source->async_format, source->async_trc));
		}
	}

	/* The inputs reach every space the inline gives. */
	assert_int_not_equal(not_srgb, 0);
	assert_int_equal(convert_video_space(VIDEO_FORMAT_P010, VIDEO_TRC_PQ), GS_CS_709_EXTENDED);
	assert_int_equal(convert_video_space(VIDEO_FORMAT_P010, VIDEO_TRC_SRGB), GS_CS_SRGB_16F);
	assert_int_equal(convert_video_space(VIDEO_FORMAT_NV12, VIDEO_TRC_DEFAULT), GS_CS_SRGB);

	free_source(source);
}

static void test_core_video(void **state)
{
	(void)state;

	obs->video.video_frame_interval_ns = 0x8000000000000021ULL;
	assert_true(obs_source_deinterlace_seam_get_video_frame_interval_ns() == 0x8000000000000021ULL);
	obs->video.video_frame_interval_ns = 16666667;
	assert_true(obs_source_deinterlace_seam_get_video_frame_interval_ns() == 16666667);

	assert_ptr_equal(obs_source_deinterlace_seam_get_video_stats(), &obs->video.stats);

	/* Each effect slot is the core's own, the one obs_load_effect()
	 * fills. */
	assert_ptr_equal(obs_source_deinterlace_seam_get_discard_effect_slot(), &obs->video.deinterlace_discard_effect);
	assert_ptr_equal(obs_source_deinterlace_seam_get_discard_2x_effect_slot(),
			 &obs->video.deinterlace_discard_2x_effect);
	assert_ptr_equal(obs_source_deinterlace_seam_get_blend_effect_slot(), &obs->video.deinterlace_blend_effect);
	assert_ptr_equal(obs_source_deinterlace_seam_get_blend_2x_effect_slot(),
			 &obs->video.deinterlace_blend_2x_effect);
	assert_ptr_equal(obs_source_deinterlace_seam_get_linear_effect_slot(), &obs->video.deinterlace_linear_effect);
	assert_ptr_equal(obs_source_deinterlace_seam_get_linear_2x_effect_slot(),
			 &obs->video.deinterlace_linear_2x_effect);
	assert_ptr_equal(obs_source_deinterlace_seam_get_yadif_effect_slot(), &obs->video.deinterlace_yadif_effect);
	assert_ptr_equal(obs_source_deinterlace_seam_get_yadif_2x_effect_slot(),
			 &obs->video.deinterlace_yadif_2x_effect);

	*obs_source_deinterlace_seam_get_yadif_2x_effect_slot() = (gs_effect_t *)&tag[0];
	assert_ptr_equal(obs->video.deinterlace_yadif_2x_effect, &tag[0]);
	assert_null(obs->video.deinterlace_yadif_effect);
	*obs_source_deinterlace_seam_get_yadif_2x_effect_slot() = NULL;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(test_async_queue, setup, teardown),
		cmocka_unit_test_setup_teardown(test_async_mutex, setup, teardown),
		cmocka_unit_test_setup_teardown(test_clock_fields, setup, teardown),
		cmocka_unit_test_setup_teardown(test_frame_slots, setup, teardown),
		cmocka_unit_test_setup_teardown(test_texture_fields, setup, teardown),
		cmocka_unit_test_setup_teardown(test_deinterlace_fields, setup, teardown),
		cmocka_unit_test_setup_teardown(test_convert_video_space, setup, teardown),
		cmocka_unit_test_setup_teardown(test_core_video, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
