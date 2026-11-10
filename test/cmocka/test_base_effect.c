/* Actual-library characterization of the borrowed core effect query.
 * Known enums require a live core (characterized, not endorsed). The
 * OpenGL effect cache lets the public API identify each video-owned pointer
 * by its original filename without exporting private core storage. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <limits.h>
#include <cmocka.h>
#include <obs.h>
#include <util/bmem.h>

static const struct {
	enum obs_base_effect id;
	const char *file;
} effects[] = {
	{OBS_EFFECT_DEFAULT, "default.effect"},
	{OBS_EFFECT_DEFAULT_RECT, "default_rect.effect"},
	{OBS_EFFECT_OPAQUE, "opaque.effect"},
	{OBS_EFFECT_SOLID, "solid.effect"},
	{OBS_EFFECT_REPEAT, "repeat.effect"},
	{OBS_EFFECT_BICUBIC, "bicubic_scale.effect"},
	{OBS_EFFECT_LANCZOS, "lanczos_scale.effect"},
	{OBS_EFFECT_AREA, "area.effect"},
	{OBS_EFFECT_BILINEAR_LOWRES, "bilinear_lowres_scale.effect"},
	{OBS_EFFECT_PREMULTIPLIED_ALPHA, "premultiplied_alpha.effect"},
};

static void unknown_enums(void **state)
{
	(void)state;
	const int unknown[] = {-1, INT_MIN, 10, 11, INT_MAX};
	for (size_t i = 0; i < sizeof(unknown) / sizeof(unknown[0]); i++)
		assert_null(obs_get_base_effect((enum obs_base_effect)unknown[i]));
}

static int startup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int shutdown_core(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static void unloaded_effects(void **state)
{
	(void)state;
	for (size_t i = 0; i < sizeof(effects) / sizeof(effects[0]); i++)
		assert_null(obs_get_base_effect(effects[i].id));
	unknown_enums(state);
}

static void loaded_mappings_and_identity(void **state)
{
	(void)state;
	struct obs_video_info video = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	assert_int_equal(obs_reset_video(&video), OBS_VIDEO_SUCCESS);
	gs_effect_t *borrowed[10];
	obs_enter_graphics();
	for (size_t i = 0; i < sizeof(effects) / sizeof(effects[0]); i++) {
		borrowed[i] = obs_get_base_effect(effects[i].id);
		assert_non_null(borrowed[i]);
		char *path = obs_find_data_file(effects[i].file);
		assert_non_null(path);
		/* All ten files were loaded by obs_init_graphics. A cache hit
		 * returns the SAME effect, without transferring ownership. */
		assert_ptr_equal(gs_effect_create_from_file(path, NULL), borrowed[i]);
		bfree(path);
		for (size_t j = 0; j < i; j++)
			assert_ptr_not_equal(borrowed[i], borrowed[j]);
	}
	for (unsigned pass = 0; pass < 3; pass++) {
		unknown_enums(state);
		for (size_t i = 0; i < sizeof(effects) / sizeof(effects[0]); i++)
			assert_ptr_equal(obs_get_base_effect(effects[i].id), borrowed[i]);
	}
	obs_leave_graphics();
	/* Borrowed effects are destroyed by obs_shutdown, never by this test. */
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(unknown_enums),
		cmocka_unit_test_setup_teardown(unloaded_effects, startup, shutdown_core),
		cmocka_unit_test_setup_teardown(loaded_mappings_and_identity, startup, shutdown_core),
		cmocka_unit_test(unknown_enums),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
