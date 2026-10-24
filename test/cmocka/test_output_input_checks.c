/* obs-rust/obs-studio#2127: two unchecked inputs of a raw output.
 *
 * obs_output_set_mixer() stored (size_t)1 << mixer_idx for any index. From 6
 * to 63 that selects a mixer that does not exist, so obs_output_get_mixer()
 * reports 0 instead of what was set; from 64 up the shift is undefined (on
 * x86-64 it wraps, 64 selects mixer 0). An index of MAX_AUDIO_MIXES or more
 * must leave the previous mask, as obs_output_set_preferred_size2() ignores
 * an index out of range for its own index.
 *
 * obs_output_get_video_conversion() read info->format from
 * video_output_get_info(NULL) when the output had a scaled size but no video
 * (obs_output_set_media() given NULL video). With nothing to convert from it
 * must return NULL, as it already does without a scaled size.
 *
 * Every valid call keeps its result: indices 0 to 5, an output with video with
 * and without a scaled size, and a set conversion with and without video.
 *
 * The NULL-video calls run in a forked child, so a crash is reported as a
 * failure of that case instead of aborting the binary. Core video needs a
 * graphics module and libobs needs an X display on Linux, so the test skips
 * without one (run under xvfb-run).
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <cmocka.h>

#include <signal.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#ifdef __linux__
#include <sys/prctl.h>
#endif

#include <obs.h>

#define RAW_OUTPUT_ID "test_output_input_checks_raw"
#define VIDEO_WIDTH 64
#define VIDEO_HEIGHT 48

/* ------------------------------------------------------------------------- */
/* raw audio/video output, never started */

static const char *raw_output_name(void *unused)
{
	(void)unused;
	return "input checks raw output";
}

static void *raw_output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	return output;
}

static void raw_output_destroy(void *data)
{
	(void)data;
}

static bool raw_output_start(void *data)
{
	(void)data;
	return false;
}

static void raw_output_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

static void raw_output_video(void *data, struct video_data *frame)
{
	(void)data;
	(void)frame;
}

static void raw_output_audio(void *data, struct audio_data *frames)
{
	(void)data;
	(void)frames;
}

static const struct obs_output_info raw_output_info = {
	.id = RAW_OUTPUT_ID,
	.flags = OBS_OUTPUT_AV,
	.get_name = raw_output_name,
	.create = raw_output_create,
	.destroy = raw_output_destroy,
	.start = raw_output_start,
	.stop = raw_output_stop,
	.raw_video = raw_output_video,
	.raw_audio = raw_output_audio,
};

static obs_output_t *create_output(void)
{
	obs_output_t *output = obs_output_create(RAW_OUTPUT_ID, "input checks output", NULL, NULL);
	assert_non_null(output);
	assert_ptr_equal(obs_output_video(output), obs_get_video());
	return output;
}

/* ------------------------------------------------------------------------- */
/* obs_output_set_mixer() */

/* Prints and counts a mismatch, so one run lists every index that fails. */
static int check_mixer(obs_output_t *output, size_t mixer, size_t mask, const char *what)
{
	size_t got_mixer = obs_output_get_mixer(output);
	size_t got_mask = obs_output_get_mixers(output);
	if (got_mixer != mixer || got_mask != mask) {
		fprintf(stderr, "%s: mixer %zu, mask 0x%zx; expected mixer %zu, mask 0x%zx\n", what, got_mixer,
			got_mask, mixer, mask);
		return 1;
	}
	return 0;
}

static void set_mixer_valid_indices(void **state)
{
	(void)state;
	int failures = 0;
	obs_output_t *output = create_output();
	failures += check_mixer(output, 0, 0, "new output");

	for (size_t idx = 0; idx < MAX_AUDIO_MIXES; idx++) {
		char what[64];
		snprintf(what, sizeof(what), "obs_output_set_mixer(%zu)", idx);
		obs_output_set_mixer(output, idx);
		failures += check_mixer(output, idx, (size_t)1 << idx, what);
	}

	obs_output_release(output);
	if (failures)
		fail();
}

static void set_mixer_out_of_range_keeps_mask(void **state)
{
	(void)state;
	static const size_t bad[] = {MAX_AUDIO_MIXES, 63, 64, SIZE_MAX};
	int failures = 0;
	obs_output_t *output = create_output();

	for (size_t i = 0; i < sizeof(bad) / sizeof(bad[0]); i++) {
		char what[96];

		/* after a single mixer */
		obs_output_set_mixer(output, 2);
		failures += check_mixer(output, 2, 0x4, "obs_output_set_mixer(2)");
		snprintf(what, sizeof(what), "obs_output_set_mixer(%zu) after mixer 2", bad[i]);
		obs_output_set_mixer(output, bad[i]);
		failures += check_mixer(output, 2, 0x4, what);

		/* after a mask of several mixers */
		obs_output_set_mixers(output, 0x29);
		failures += check_mixer(output, 0, 0x29, "obs_output_set_mixers(0x29)");
		snprintf(what, sizeof(what), "obs_output_set_mixer(%zu) after mask 0x29", bad[i]);
		obs_output_set_mixer(output, bad[i]);
		failures += check_mixer(output, 0, 0x29, what);
	}

	/* a valid index still applies afterwards */
	obs_output_set_mixer(output, 5);
	failures += check_mixer(output, 5, 0x20, "obs_output_set_mixer(5) after the ignored calls");

	obs_output_release(output);
	if (failures)
		fail();
}

/* ------------------------------------------------------------------------- */
/* obs_output_get_video_conversion() */

static void check_conversion(const struct video_scale_info *conv, enum video_format format,
			     enum video_colorspace colorspace, enum video_range_type range, uint32_t width,
			     uint32_t height, const char *what)
{
	if (!conv) {
		fprintf(stderr, "%s: no conversion, expected %ux%u\n", what, width, height);
		fail();
	}
	if (conv->format != format || conv->colorspace != colorspace || conv->range != range || conv->width != width ||
	    conv->height != height) {
		fprintf(stderr,
			"%s: conversion format %d colorspace %d range %d %ux%u; "
			"expected format %d colorspace %d range %d %ux%u\n",
			what, (int)conv->format, (int)conv->colorspace, (int)conv->range, conv->width, conv->height,
			(int)format, (int)colorspace, (int)range, width, height);
		fail();
	}
}

static void check_no_conversion(const struct video_scale_info *conv, const char *what)
{
	if (conv) {
		fprintf(stderr, "%s: conversion %ux%u format %d, expected none\n", what, conv->width, conv->height,
			(int)conv->format);
		fail();
	}
}

static const struct video_scale_info set_i420 = {
	.format = VIDEO_FORMAT_I420,
	.colorspace = VIDEO_CS_601,
	.range = VIDEO_RANGE_FULL,
};

static void conversion_with_video(void **state)
{
	(void)state;
	obs_output_t *output = create_output();
	check_no_conversion(obs_output_get_video_conversion(output), "video, no scaled size");
	obs_output_set_preferred_size(output, VIDEO_WIDTH, VIDEO_HEIGHT);
	check_no_conversion(obs_output_get_video_conversion(output), "video, scaled to the video size");
	obs_output_set_preferred_size(output, 32, 24);
	check_conversion(obs_output_get_video_conversion(output), VIDEO_FORMAT_NV12, VIDEO_CS_DEFAULT,
			 VIDEO_RANGE_DEFAULT, 32, 24, "video, scaled to 32x24");
	obs_output_set_preferred_size(output, VIDEO_WIDTH, 24);
	check_conversion(obs_output_get_video_conversion(output), VIDEO_FORMAT_NV12, VIDEO_CS_DEFAULT,
			 VIDEO_RANGE_DEFAULT, VIDEO_WIDTH, 24, "video, scaled height only");
	obs_output_release(output);

	output = create_output();
	obs_output_set_video_conversion(output, &set_i420);
	check_conversion(obs_output_get_video_conversion(output), VIDEO_FORMAT_I420, VIDEO_CS_601, VIDEO_RANGE_FULL,
			 VIDEO_WIDTH, VIDEO_HEIGHT, "video, set conversion of 0x0");
	obs_output_release(output);

	output = create_output();
	obs_output_set_preferred_size(output, 32, 24);
	obs_output_set_video_conversion(output, &set_i420);
	check_conversion(obs_output_get_video_conversion(output), VIDEO_FORMAT_I420, VIDEO_CS_601, VIDEO_RANGE_FULL, 32,
			 24, "video, scaled, set conversion of 0x0");
	obs_output_release(output);

	output = create_output();
	struct video_scale_info sized = set_i420;
	sized.width = 16;
	sized.height = 8;
	obs_output_set_preferred_size(output, 32, 24);
	obs_output_set_video_conversion(output, &sized);
	check_conversion(obs_output_get_video_conversion(output), VIDEO_FORMAT_I420, VIDEO_CS_601, VIDEO_RANGE_FULL, 16,
			 8, "video, scaled, set conversion of 16x8");
	obs_output_release(output);
}

/* What the child reports for a NULL-video case. */
enum null_video_result {
	NULL_VIDEO_OK = 0,
	NULL_VIDEO_WRONG = 1,
};

static bool same_conversion(const struct video_scale_info *conv, enum video_format format, uint32_t width,
			    uint32_t height)
{
	return conv && conv->format == format && conv->width == width && conv->height == height;
}

enum null_video_case {
	NULL_VIDEO_UNSCALED,
	NULL_VIDEO_SCALED,
	NULL_VIDEO_SET,
	NULL_VIDEO_SCALED_SET,
	NULL_VIDEO_SCALED_SET_SIZED,
};

static const char *null_video_case_name(enum null_video_case c)
{
	switch (c) {
	case NULL_VIDEO_UNSCALED:
		return "no video, no scaled size: expected none";
	case NULL_VIDEO_SCALED:
		return "no video, scaled to 32x24: expected none";
	case NULL_VIDEO_SET:
		return "no video, set conversion of 0x0: expected I420 0x0";
	case NULL_VIDEO_SCALED_SET:
		return "no video, scaled to 32x24, set conversion of 0x0: expected I420 32x24";
	case NULL_VIDEO_SCALED_SET_SIZED:
		return "no video, scaled to 32x24, set conversion of 16x8: expected I420 16x8";
	}
	return "?";
}

static int run_null_video_case(obs_output_t *output, enum null_video_case c)
{
	const struct video_scale_info *conv;
	struct video_scale_info sized = set_i420;

	switch (c) {
	case NULL_VIDEO_UNSCALED:
		conv = obs_output_get_video_conversion(output);
		return conv ? NULL_VIDEO_WRONG : NULL_VIDEO_OK;
	case NULL_VIDEO_SCALED:
		obs_output_set_preferred_size(output, 32, 24);
		conv = obs_output_get_video_conversion(output);
		return conv ? NULL_VIDEO_WRONG : NULL_VIDEO_OK;
	case NULL_VIDEO_SET:
		obs_output_set_video_conversion(output, &set_i420);
		conv = obs_output_get_video_conversion(output);
		return same_conversion(conv, VIDEO_FORMAT_I420, 0, 0) ? NULL_VIDEO_OK : NULL_VIDEO_WRONG;
	case NULL_VIDEO_SCALED_SET:
		obs_output_set_preferred_size(output, 32, 24);
		obs_output_set_video_conversion(output, &set_i420);
		conv = obs_output_get_video_conversion(output);
		return same_conversion(conv, VIDEO_FORMAT_I420, 32, 24) ? NULL_VIDEO_OK : NULL_VIDEO_WRONG;
	case NULL_VIDEO_SCALED_SET_SIZED:
		sized.width = 16;
		sized.height = 8;
		obs_output_set_preferred_size(output, 32, 24);
		obs_output_set_video_conversion(output, &sized);
		conv = obs_output_get_video_conversion(output);
		return same_conversion(conv, VIDEO_FORMAT_I420, 16, 8) ? NULL_VIDEO_OK : NULL_VIDEO_WRONG;
	}
	return NULL_VIDEO_WRONG;
}

static void expect_null_video_case(enum null_video_case c)
{
	const char *what = null_video_case_name(c);
	obs_output_t *output = create_output();
	obs_output_set_media(output, NULL, obs_get_audio());
	assert_null(obs_output_video(output));

	fflush(stdout);
	fflush(stderr);
	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		/* cmocka traps SIGSEGV and would resume the runner in the child;
		 * let a crash kill the child instead. */
		signal(SIGSEGV, SIG_DFL);
		signal(SIGBUS, SIG_DFL);
		signal(SIGABRT, SIG_DFL);
#ifdef __linux__
		/* an expected crash should not leave a core dump behind */
		prctl(PR_SET_DUMPABLE, 0);
#endif
		_exit(run_null_video_case(output, c));
	}

	int status = 0;
	assert_int_equal(waitpid(pid, &status, 0), pid);
	obs_output_release(output);
	if (WIFSIGNALED(status)) {
		fprintf(stderr, "%s: obs_output_get_video_conversion() crashed with signal %d\n", what,
			WTERMSIG(status));
		fail();
	}
	if (!WIFEXITED(status) || WEXITSTATUS(status) != NULL_VIDEO_OK) {
		fprintf(stderr, "%s: obs_output_get_video_conversion() returned something else\n", what);
		fail();
	}
}

static void conversion_no_video_unscaled(void **state)
{
	(void)state;
	expect_null_video_case(NULL_VIDEO_UNSCALED);
}

static void conversion_no_video_scaled(void **state)
{
	(void)state;
	expect_null_video_case(NULL_VIDEO_SCALED);
}

static void conversion_no_video_set(void **state)
{
	(void)state;
	expect_null_video_case(NULL_VIDEO_SET);
	expect_null_video_case(NULL_VIDEO_SCALED_SET);
	expect_null_video_case(NULL_VIDEO_SCALED_SET_SIZED);
}

/* ------------------------------------------------------------------------- */

static int group_setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "obs_startup failed\n");
		return -1;
	}
	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "obs_reset_audio failed\n");
		return -1;
	}
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = VIDEO_WIDTH,
		.base_height = VIDEO_HEIGHT,
		.output_width = VIDEO_WIDTH,
		.output_height = VIDEO_HEIGHT,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: %d\n", ret);
		return -1;
	}
	obs_register_output(&raw_output_info);
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

int main(void)
{
	if (!getenv("DISPLAY"))
		return 77; /* Run with xvfb-run on headless Linux hosts. */

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(set_mixer_valid_indices),    cmocka_unit_test(set_mixer_out_of_range_keeps_mask),
		cmocka_unit_test(conversion_with_video),      cmocka_unit_test(conversion_no_video_unscaled),
		cmocka_unit_test(conversion_no_video_scaled), cmocka_unit_test(conversion_no_video_set),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
