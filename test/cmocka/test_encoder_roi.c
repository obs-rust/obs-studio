/* Characterization of the encoder region of interest (ROI) API, through the
 * exported functions: obs_encoder_add_roi(), obs_encoder_clear_roi(),
 * obs_encoder_has_roi(), obs_encoder_enum_roi() and
 * obs_encoder_get_roi_increment(). Their checks and scaling are in
 * libobs/obs-encoder-roi.c.h (obs-rust/obs-studio#1959).
 *
 * Two test video encoders are registered, one with OBS_ENCODER_CAP_ROI and
 * one without. They are never started. The scaled cases give the encoder a
 * video output of its own (video_output_open()), so no graphics are needed.
 * obs_startup() needs a display on Linux (xvfb-run); without one the cases
 * skip. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>

#include <obs.h>
#include <media-io/video-io.h>

#define ROI_ENC_ID "test_encoder_roi_cap"
#define NO_ROI_ENC_ID "test_encoder_roi_nocap"

static const char *enc_name(void *type_data)
{
	(void)type_data;
	return "roi encoder";
}

static void *enc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	return bzalloc(1);
}

static void enc_destroy(void *data)
{
	bfree(data);
}

static bool enc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received = false;
	return true;
}

static bool obs_up;

static int roi_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_encoder_info info = {
		.id = ROI_ENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = enc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = enc_encode,
		.caps = OBS_ENCODER_CAP_ROI,
	};
	obs_register_encoder(&info);

	info.id = NO_ROI_ENC_ID;
	info.caps = 0;
	obs_register_encoder(&info);
	return 0;
}

static int roi_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

static obs_encoder_t *roi_encoder(const char *id)
{
	if (!obs_up)
		skip();
	obs_encoder_t *enc = obs_video_encoder_create(id, "roi", NULL, NULL);
	assert_non_null(enc);
	return enc;
}

static struct obs_encoder_roi rect(uint32_t top, uint32_t bottom, uint32_t left, uint32_t right, float priority)
{
	struct obs_encoder_roi roi = {
		.top = top,
		.bottom = bottom,
		.left = left,
		.right = right,
		.priority = priority,
	};
	return roi;
}

/* Adds `roi` and checks the result, and that roi_increment moved only when
 * it was stored. */
static void expect_add(obs_encoder_t *enc, struct obs_encoder_roi roi, bool stored)
{
	uint32_t before = obs_encoder_get_roi_increment(enc);
	assert_int_equal(obs_encoder_add_roi(enc, &roi), stored);
	assert_int_equal(obs_encoder_get_roi_increment(enc), before + (stored ? 1 : 0));
}

#define MAX_SEEN 8

struct seen {
	size_t count;
	struct obs_encoder_roi rois[MAX_SEEN];
};

static void collect(void *param, struct obs_encoder_roi *roi)
{
	struct seen *seen = param;
	assert_true(seen->count < MAX_SEEN);
	seen->rois[seen->count++] = *roi;
}

static void assert_roi(const struct obs_encoder_roi *roi, uint32_t top, uint32_t bottom, uint32_t left, uint32_t right,
		       float priority)
{
	assert_int_equal(roi->top, top);
	assert_int_equal(roi->bottom, bottom);
	assert_int_equal(roi->left, left);
	assert_int_equal(roi->right, right);
	assert_true(roi->priority == priority);
}

/* A video output of its own, so obs_encoder_enum_roi() has a size to scale
 * from. */
static video_t *open_video(uint32_t width, uint32_t height)
{
	struct video_output_info voi = {
		.name = "roi video",
		.format = VIDEO_FORMAT_NV12,
		.fps_num = 30,
		.fps_den = 1,
		.width = width,
		.height = height,
		.cache_size = 4,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
	};
	video_t *video = NULL;
	assert_int_equal(video_output_open(&video, &voi), VIDEO_OUTPUT_SUCCESS);
	assert_non_null(video);
	return video;
}

static void test_null_roi_is_refused(void **state)
{
	(void)state;
	obs_encoder_t *enc = roi_encoder(ROI_ENC_ID);

	assert_false(obs_encoder_add_roi(enc, NULL));
	assert_int_equal(obs_encoder_get_roi_increment(enc), 0);
	assert_false(obs_encoder_has_roi(enc));

	obs_encoder_release(enc);
}

static void test_encoder_without_cap_refuses(void **state)
{
	(void)state;
	obs_encoder_t *enc = roi_encoder(NO_ROI_ENC_ID);

	expect_add(enc, rect(0, 64, 0, 64, 0.5f), false);
	assert_false(obs_encoder_has_roi(enc));

	obs_encoder_release(enc);
}

/* The smallest ROI is one 16 x 16 block, in each direction on its own. */
static void test_minimum_size(void **state)
{
	(void)state;
	obs_encoder_t *enc = roi_encoder(ROI_ENC_ID);

	expect_add(enc, rect(100, 115, 200, 300, 0.5f), false);
	expect_add(enc, rect(100, 116, 200, 300, 0.5f), true);
	expect_add(enc, rect(100, 117, 200, 300, 0.5f), true);

	expect_add(enc, rect(100, 200, 200, 215, 0.5f), false);
	expect_add(enc, rect(100, 200, 200, 216, 0.5f), true);
	expect_add(enc, rect(100, 200, 200, 217, 0.5f), true);

	expect_add(enc, rect(100, 115, 200, 215, 0.5f), false);
	expect_add(enc, rect(0, 16, 0, 16, 0.5f), true);
	expect_add(enc, rect(100, 100, 200, 300, 0.5f), false);
	expect_add(enc, rect(100, 200, 200, 200, 0.5f), false);
	expect_add(enc, rect(UINT32_MAX - 16, UINT32_MAX, UINT32_MAX - 16, UINT32_MAX, 0.5f), true);
	expect_add(enc, rect(UINT32_MAX - 15, UINT32_MAX, 0, 16, 0.5f), false);

	obs_encoder_release(enc);
}

static void test_priority_range(void **state)
{
	(void)state;
	obs_encoder_t *enc = roi_encoder(ROI_ENC_ID);

	expect_add(enc, rect(0, 16, 0, 16, -1.0f), true);
	expect_add(enc, rect(0, 16, 0, 16, 1.0f), true);
	expect_add(enc, rect(0, 16, 0, 16, 0.0f), true);
	expect_add(enc, rect(0, 16, 0, 16, -0.0f), true);
	/* The floats just outside [-1, 1]. */
	expect_add(enc, rect(0, 16, 0, 16, -0x1.000002p+0f), false);
	expect_add(enc, rect(0, 16, 0, 16, 0x1.000002p+0f), false);
	expect_add(enc, rect(0, 16, 0, 16, -INFINITY), false);
	expect_add(enc, rect(0, 16, 0, 16, INFINITY), false);

	obs_encoder_release(enc);
}

/* A NaN priority is not in [-1, 1]. NaN < -1 and NaN > 1 are both false,
 * so the range check alone stored it (#1959). */
static void test_nan_priority(void **state)
{
	(void)state;
	obs_encoder_t *enc = roi_encoder(ROI_ENC_ID);

	expect_add(enc, rect(0, 16, 0, 16, NAN), false);
	expect_add(enc, rect(0, 16, 0, 16, -NAN), false);
	assert_false(obs_encoder_has_roi(enc));

	obs_encoder_release(enc);
}

/* An upside-down rectangle (bottom above top, or right left of left) is
 * refused. The size check subtracts in uint32_t, so on its own it wrapped
 * to a large size and stored the rectangle (#1959). */
static void test_upside_down_rect(void **state)
{
	(void)state;
	obs_encoder_t *enc = roi_encoder(ROI_ENC_ID);

	expect_add(enc, rect(200, 100, 0, 64, 0.5f), false);
	expect_add(enc, rect(0, 64, 200, 100, 0.5f), false);
	expect_add(enc, rect(200, 100, 200, 100, 0.5f), false);
	expect_add(enc, rect(101, 100, 0, 64, 0.5f), false);
	expect_add(enc, rect(0, 64, 101, 100, 0.5f), false);
	expect_add(enc, rect(UINT32_MAX, 0, 0, 64, 0.5f), false);
	assert_false(obs_encoder_has_roi(enc));

	obs_encoder_release(enc);
}

static void test_increment_after_add_and_clear(void **state)
{
	(void)state;
	obs_encoder_t *enc = roi_encoder(ROI_ENC_ID);

	assert_int_equal(obs_encoder_get_roi_increment(enc), 0);
	assert_false(obs_encoder_has_roi(enc));

	/* Clearing an empty list changes nothing. */
	obs_encoder_clear_roi(enc);
	assert_int_equal(obs_encoder_get_roi_increment(enc), 0);

	expect_add(enc, rect(0, 16, 0, 16, 0.5f), true);
	expect_add(enc, rect(16, 32, 16, 32, -0.5f), true);
	assert_int_equal(obs_encoder_get_roi_increment(enc), 2);
	assert_true(obs_encoder_has_roi(enc));

	obs_encoder_clear_roi(enc);
	assert_int_equal(obs_encoder_get_roi_increment(enc), 3);
	assert_false(obs_encoder_has_roi(enc));

	obs_encoder_clear_roi(enc);
	assert_int_equal(obs_encoder_get_roi_increment(enc), 3);

	expect_add(enc, rect(0, 16, 0, 16, 0.5f), true);
	assert_int_equal(obs_encoder_get_roi_increment(enc), 4);

	obs_encoder_release(enc);
}

/* Without a scaled size the ROIs are passed on as stored, last added
 * first. */
static void test_enum_order_unscaled(void **state)
{
	(void)state;
	obs_encoder_t *enc = roi_encoder(ROI_ENC_ID);
	struct seen seen = {0};

	obs_encoder_enum_roi(enc, collect, &seen);
	assert_int_equal(seen.count, 0);

	expect_add(enc, rect(0, 16, 0, 16, 1.0f), true);
	expect_add(enc, rect(33, 101, 7, 1001, -1.0f), true);
	expect_add(enc, rect(1000, 2000, 3000, 4000, 0.25f), true);

	obs_encoder_enum_roi(enc, collect, &seen);
	assert_int_equal(seen.count, 3);
	assert_roi(&seen.rois[0], 1000, 2000, 3000, 4000, 0.25f);
	assert_roi(&seen.rois[1], 33, 101, 7, 1001, -1.0f);
	assert_roi(&seen.rois[2], 0, 16, 0, 16, 1.0f);

	obs_encoder_release(enc);
}

/* With a scaled size each edge is multiplied in float by scaled / video
 * size and truncated toward 0, still last added first. */
static void test_enum_order_scaled(void **state)
{
	(void)state;
	obs_encoder_t *enc = roi_encoder(ROI_ENC_ID);
	video_t *video = open_video(1920, 1080);
	struct seen seen = {0};

	obs_encoder_set_video(enc, video);
	obs_encoder_set_scaled_size(enc, 1280, 720);

	expect_add(enc, rect(0, 1080, 0, 1920, 1.0f), true);
	expect_add(enc, rect(33, 101, 7, 1001, -1.0f), true);
	expect_add(enc, rect(100, 200, 300, 400, 0.25f), true);

	obs_encoder_enum_roi(enc, collect, &seen);
	assert_int_equal(seen.count, 3);
	/* x 2/3 (0.6666667f): 66.666664 -> 66, 133.33333 -> 133. */
	assert_roi(&seen.rois[0], 66, 133, 200, 266, 0.25f);
	assert_roi(&seen.rois[1], 22, 67, 4, 667, -1.0f);
	assert_roi(&seen.rois[2], 0, 720, 0, 1280, 1.0f);

	obs_encoder_release(enc);

	enc = roi_encoder(ROI_ENC_ID);
	obs_encoder_set_video(enc, video);
	obs_encoder_set_scaled_size(enc, 3840, 540);
	expect_add(enc, rect(33, 101, 7, 1001, 0.5f), true);

	seen.count = 0;
	obs_encoder_enum_roi(enc, collect, &seen);
	assert_int_equal(seen.count, 1);
	assert_roi(&seen.rois[0], 16, 50, 14, 2002, 0.5f);

	obs_encoder_release(enc);
	video_output_close(video);
}

/* A scaled edge at 2^32 or past it is UINT32_MAX, the nearest value in
 * range. The (uint32_t) cast of such a float is undefined in C (#1959). */
static void test_enum_scaled_saturates(void **state)
{
	(void)state;
	obs_encoder_t *enc = roi_encoder(ROI_ENC_ID);
	video_t *video = open_video(16, 16);
	struct seen seen = {0};

	/* x 256 */
	obs_encoder_set_video(enc, video);
	obs_encoder_set_scaled_size(enc, 4096, 4096);

	/* 2^24 - 1 x 256 is 2^32 - 256, in range; 2^24 x 256 is 2^32. */
	expect_add(enc, rect(0, 0xFFFFFF, 16, 0x1000000, 0.5f), true);
	expect_add(enc, rect(0x1000000, UINT32_MAX, 0xFFFFFF, UINT32_MAX, -0.5f), true);

	obs_encoder_enum_roi(enc, collect, &seen);
	assert_int_equal(seen.count, 2);
	assert_roi(&seen.rois[0], UINT32_MAX, UINT32_MAX, 0xFFFFFF00u, UINT32_MAX, -0.5f);
	assert_roi(&seen.rois[1], 0, 0xFFFFFF00u, 4096, UINT32_MAX, 0.5f);

	obs_encoder_release(enc);
	video_output_close(video);
}

/* The ROI functions check the encoder, as the other obs_encoder_*
 * functions do (obs_encoder_valid()), instead of dereferencing NULL. */
static void test_null_encoder(void **state)
{
	(void)state;
	struct obs_encoder_roi roi = rect(0, 16, 0, 16, 0.5f);
	struct seen seen = {0};

	assert_false(obs_encoder_add_roi(NULL, &roi));
	assert_false(obs_encoder_has_roi(NULL));
	obs_encoder_clear_roi(NULL);
	obs_encoder_enum_roi(NULL, collect, &seen);
	assert_int_equal(seen.count, 0);
	assert_int_equal(obs_encoder_get_roi_increment(NULL), 0);
}

/* A scaled size with no video to scale from: nothing is passed on. */
static void test_enum_without_video_size(void **state)
{
	(void)state;
	obs_encoder_t *enc = roi_encoder(ROI_ENC_ID);
	struct seen seen = {0};

	obs_encoder_set_scaled_size(enc, 1280, 720);
	expect_add(enc, rect(0, 16, 0, 16, 0.5f), true);

	obs_encoder_enum_roi(enc, collect, &seen);
	assert_int_equal(seen.count, 0);

	obs_encoder_release(enc);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_null_roi_is_refused),
		cmocka_unit_test(test_encoder_without_cap_refuses),
		cmocka_unit_test(test_minimum_size),
		cmocka_unit_test(test_priority_range),
		cmocka_unit_test(test_nan_priority),
		cmocka_unit_test(test_upside_down_rect),
		cmocka_unit_test(test_increment_after_add_and_clear),
		cmocka_unit_test(test_enum_order_unscaled),
		cmocka_unit_test(test_enum_order_scaled),
		cmocka_unit_test(test_enum_without_video_size),
		cmocka_unit_test(test_enum_scaled_saturates),
		cmocka_unit_test(test_null_encoder),
	};
	return cmocka_run_group_tests(tests, roi_setup, roi_teardown);
}
