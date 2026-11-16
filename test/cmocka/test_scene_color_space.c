/* Characterize scene_video_get_color_space through the public source API.
 * Preferred spaces are ignored: characterized, not endorsed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs.h>

static void check_preferences(enum gs_color_space expected)
{
	obs_scene_t *scene = obs_scene_create_private("color-space characterization");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	const enum gs_color_space srgb[] = {GS_CS_SRGB};
	const enum gs_color_space extended[] = {GS_CS_709_EXTENDED};
	const enum gs_color_space other[] = {GS_CS_SRGB_16F, GS_CS_709_SCRGB};
	const enum gs_color_space forward[] = {GS_CS_SRGB, GS_CS_709_EXTENDED};
	const enum gs_color_space reverse[] = {GS_CS_709_EXTENDED, GS_CS_SRGB};
	assert_int_equal(obs_source_get_color_space(source, 0, NULL), expected);
	assert_int_equal(obs_source_get_color_space(source, 1, srgb), expected);
	assert_int_equal(obs_source_get_color_space(source, 1, extended), expected);
	assert_int_equal(obs_source_get_color_space(source, 2, other), expected);
	assert_int_equal(obs_source_get_color_space(source, 2, forward), expected);
	assert_int_equal(obs_source_get_color_space(source, 2, reverse), expected);
	obs_scene_release(scene);
}

static void no_video_defaults_to_srgb(void **state)
{
	(void)state;
	struct obs_video_info info;
	assert_false(obs_get_video_info(&info));
	check_preferences(GS_CS_SRGB);
}

static void check_video(enum video_colorspace colorspace, enum gs_color_space expected)
{
	struct obs_video_info info = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 32,
		.base_height = 32,
		.output_width = 32,
		.output_height = 32,
		.output_format = VIDEO_FORMAT_P010,
		.colorspace = colorspace,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	assert_int_equal(obs_reset_video(&info), OBS_VIDEO_SUCCESS);
	struct obs_video_info actual;
	assert_true(obs_get_video_info(&actual));
	assert_int_equal(actual.colorspace, colorspace);
	check_preferences(expected);
}

static void srgb_ignores_hdr_preference(void **state)
{
	(void)state;
	check_video(VIDEO_CS_SRGB, GS_CS_SRGB);
}

static void pq_ignores_srgb_preference(void **state)
{
	(void)state;
	check_video(VIDEO_CS_2100_PQ, GS_CS_709_EXTENDED);
}

static void hlg_ignores_srgb_preference(void **state)
{
	(void)state;
	check_video(VIDEO_CS_2100_HLG, GS_CS_709_EXTENDED);
}

static int setup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(no_video_defaults_to_srgb),
		cmocka_unit_test(srgb_ignores_hdr_preference),
		cmocka_unit_test(pq_ignores_srgb_preference),
		cmocka_unit_test(hlg_ignores_srgb_preference),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
