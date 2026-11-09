/* Direct characterization of registered transitions before the Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>
#include "util/threading.h"
#include "obs-internal.h"
#include "obs-scene.h"

static unsigned created, destroyed;
static const char *fixture_name(void *data)
{
	(void)data;
	return "serialization fixture";
}
static void *fixture_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	created++;
	return source;
}
static void fixture_destroy(void *data)
{
	(void)data;
	destroyed++;
}
static void fixture_render(void *data, gs_effect_t *effect)
{
	(void)data;
	(void)effect;
}
static bool fixture_audio(void *data, uint64_t *ts, struct obs_source_audio_mix *audio, uint32_t mixers,
			  size_t channels, size_t rate)
{
	(void)data;
	(void)ts;
	(void)audio;
	(void)mixers;
	(void)channels;
	(void)rate;
	return false;
}
static int setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
	obs_add_data_path(TEST_LIBOBS_DATA_PATH);
#pragma GCC diagnostic pop
	struct obs_video_info video = {.graphics_module = GRAPHICS_MODULE_PATH,
				       .fps_num = 30,
				       .fps_den = 1,
				       .base_width = 64,
				       .base_height = 64,
				       .output_width = 64,
				       .output_height = 64,
				       .output_format = VIDEO_FORMAT_NV12,
				       .colorspace = VIDEO_CS_709,
				       .range = VIDEO_RANGE_PARTIAL,
				       .scale_type = OBS_SCALE_BICUBIC};
	if (obs_reset_video(&video) != OBS_VIDEO_SUCCESS) {
		obs_shutdown();
		return -1;
	}
	struct obs_source_info info = {.id = "serialization_fixture",
				       .type = OBS_SOURCE_TYPE_TRANSITION,
				       .output_flags = OBS_SOURCE_VIDEO,
				       .get_name = fixture_name,
				       .create = fixture_create,
				       .destroy = fixture_destroy,
				       .video_render = fixture_render,
				       .audio_render = fixture_audio};
	obs_register_source(&info);
	return 0;
}
static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}
static void roundtrip_replace_clear(void **state)
{
	(void)state;
	struct obs_scene_item item = {0};
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "marker", "kept");
	for (unsigned show = 0; show < 2; show++) {
		unsigned start = created;
		obs_source_t *source = obs_source_create_private("serialization_fixture", "Original", settings);
		assert_non_null(source);
		assert_non_null(source->context.data);
		obs_sceneitem_set_transition(&item, show != 0, source);
		obs_sceneitem_set_transition_duration(&item, show != 0, 777);
		obs_source_release(source);
		obs_data_t *saved = obs_sceneitem_transition_save(&item, show != 0);
		assert_string_equal(obs_data_get_string(saved, "id"), "serialization_fixture");
		assert_string_equal(obs_data_get_string(saved, "name"), "Original");
		/* Load must use id, not versioned_id. */
		obs_data_set_string(saved, "versioned_id", "unused_wrong_id");
		obs_weak_source_t *old = obs_source_get_weak_source(obs_sceneitem_get_transition(&item, show != 0));
		obs_sceneitem_transition_load(&item, saved, show != 0);
		obs_wait_for_destroy_queue();
		assert_true(obs_weak_source_expired(old));
		obs_weak_source_release(old);
		obs_source_t *loaded = obs_sceneitem_get_transition(&item, show != 0);
		assert_non_null(loaded);
		assert_non_null(loaded->context.data);
		assert_string_equal(obs_source_get_id(loaded), "serialization_fixture");
		assert_string_equal(obs_source_get_name(loaded), "Original");
		obs_data_t *got = obs_source_get_settings(loaded);
		assert_string_equal(obs_data_get_string(got, "marker"), "kept");
		obs_data_release(got);
		assert_int_equal(obs_sceneitem_get_transition_duration(&item, show != 0), 777);
		assert_null(obs_sceneitem_get_transition(&item, show == 0));
		obs_weak_source_t *weak = obs_source_get_weak_source(loaded);
		obs_data_t *empty = obs_data_create();
		obs_sceneitem_transition_load(&item, empty, show != 0);
		obs_wait_for_destroy_queue();
		assert_null(obs_sceneitem_get_transition(&item, show != 0));
		assert_true(obs_weak_source_expired(weak));
		assert_int_equal(obs_sceneitem_get_transition_duration(&item, show != 0), 0);
		obs_weak_source_release(weak);
		obs_data_release(empty);
		obs_data_release(saved);
		assert_int_equal(created, start + 2);
		assert_int_equal(destroyed, created);
	}
	assert_string_equal(obs_data_get_string(settings, "marker"), "kept");
	obs_data_release(settings);
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(roundtrip_replace_clear)};
	return cmocka_run_group_tests(tests, setup, teardown);
}
