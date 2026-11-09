/* Characterize immediate reconciliation through public APIs, observing private
 * state only to distinguish queued audio changes from immediate ones. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "util/threading.h"
#include "obs-scene.h"

static const char *input_name(void *unused)
{
	(void)unused;
	return "visibility characterization";
}

static void *input_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void input_destroy(void *data)
{
	(void)data;
}

static uint32_t input_dimension(void *data)
{
	(void)data;
	return 1;
}

static void count_child(obs_source_t *parent, obs_source_t *child, void *data)
{
	(void)parent;
	assert_non_null(child);
	(*(unsigned *)data)++;
}

static void check_item(obs_sceneitem_t *item, bool visible, long refs, size_t queued)
{
	assert_int_equal(obs_sceneitem_visible(item), visible);
	assert_int_equal(item->visible, visible);
	assert_int_equal(item->user_visible, visible);
	assert_int_equal(item->active_refs, refs);
	assert_int_equal(item->audio_actions.num, queued);
}

static void immediate_video(void **state)
{
	(void)state;
	obs_scene_t *scene = obs_scene_create_private("video scene");
	obs_source_t *source = obs_source_create_private("visibility_video", "video", NULL);
	assert_non_null(scene);
	assert_non_null(source);
	obs_source_inc_active(obs_scene_get_source(scene));
	obs_sceneitem_t *item = obs_scene_add(scene, source);
	assert_non_null(item);
	check_item(item, true, 1, 0);
	assert_true(obs_source_active(source));
	for (unsigned i = 0; i < 3; i++) {
		assert_true(obs_sceneitem_set_visible(item, false));
		check_item(item, false, 0, 0);
		assert_false(obs_source_active(source));
		assert_false(obs_sceneitem_set_visible(item, false));
		assert_true(obs_sceneitem_set_visible(item, true));
		check_item(item, true, 1, 0);
		assert_true(obs_source_active(source));
		assert_false(obs_sceneitem_set_visible(item, true));
	}
	obs_source_dec_active(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_source_release(source);
}

static void remove_pending_audio(void **state)
{
	(void)state;
	obs_scene_t *scene = obs_scene_create_private("audio scene");
	obs_source_t *source = obs_source_create_private("visibility_audio", "audio", NULL);
	assert_non_null(scene);
	assert_non_null(source);
	obs_source_inc_active(obs_scene_get_source(scene));
	obs_sceneitem_t *item = obs_scene_add(scene, source);
	assert_non_null(item);
	assert_int_equal(item->audio_actions.num, 1);
	assert_true(obs_sceneitem_set_visible(item, false));
	assert_true(obs_sceneitem_set_visible(item, true));
	assert_true(obs_sceneitem_set_visible(item, false));
	/* Characterized, not endorsed: hide is queued, so refs can exceed one. */
	assert_int_equal(item->active_refs, 2);
	assert_int_equal(item->audio_actions.num, 4);
	assert_true(obs_source_active(source));
	unsigned children = 0;
	obs_source_enum_active_sources(obs_scene_get_source(scene), count_child, &children);
	assert_int_equal(children, 1);
	obs_sceneitem_addref(item);
	obs_sceneitem_remove(item);
	check_item(item, false, 0, 0);
	assert_false(obs_source_active(source));
	assert_null(obs_sceneitem_get_scene(item));
	children = 0;
	obs_source_enum_active_sources(obs_scene_get_source(scene), count_child, &children);
	assert_int_equal(children, 0);
	obs_sceneitem_release(item);
	obs_source_dec_active(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_source_release(source);
}

static void loaded_and_duplicated_audio(void **state)
{
	(void)state;
	obs_source_t *audio = obs_source_create("visibility_audio", "saved audio", NULL, NULL);
	assert_non_null(audio);
	for (unsigned visible = 0; visible < 2; visible++) {
		obs_data_t *data = obs_data_create_from_json(
			"{\"id\":\"scene\",\"name\":\"loaded scene\",\"settings\":{\"items\":[{\"name\":\"saved audio\"}]}}");
		obs_data_t *settings = obs_data_get_obj(data, "settings");
		obs_data_array_t *items = obs_data_get_array(settings, "items");
		obs_data_t *entry = obs_data_array_item(items, 0);
		obs_data_set_bool(entry, "visible", visible != 0);
		obs_source_t *loaded = obs_load_source(data);
		assert_non_null(loaded);
		obs_source_load2(loaded);
		obs_scene_t *scene = obs_scene_from_source(loaded);
		obs_sceneitem_t *item = obs_scene_find_source(scene, "saved audio");
		assert_non_null(item);
		check_item(item, visible != 0, visible ? 1 : 0, 0);
		unsigned children = 0;
		obs_source_enum_active_sources(loaded, count_child, &children);
		assert_int_equal(children, visible);
		if (!visible) {
			obs_scene_t *copy = obs_scene_duplicate(scene, "copy", OBS_SCENE_DUP_PRIVATE_REFS);
			assert_non_null(copy);
			obs_sceneitem_t *copied = obs_scene_find_source(copy, "saved audio");
			assert_non_null(copied);
			check_item(copied, false, 0, 0);
			children = 0;
			obs_source_enum_active_sources(obs_scene_get_source(copy), count_child, &children);
			assert_int_equal(children, 0);
			obs_scene_release(copy);
		}
		obs_source_remove(loaded);
		obs_source_release(loaded);
		while (obs_wait_for_destroy_queue())
			;
		obs_data_release(entry);
		obs_data_array_release(items);
		obs_data_release(settings);
		obs_data_release(data);
	}
	obs_source_release(audio);
}

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	/* Source destruction releases graphics resources even without rendering. */
	struct obs_video_info video_info = {.graphics_module = GRAPHICS_MODULE_PATH,
					    .fps_num = 30,
					    .fps_den = 1,
					    .base_width = 320,
					    .base_height = 180,
					    .output_width = 320,
					    .output_height = 180,
					    .output_format = VIDEO_FORMAT_RGBA,
					    .colorspace = VIDEO_CS_709,
					    .range = VIDEO_RANGE_FULL};
	assert_int_equal(obs_reset_video(&video_info), OBS_VIDEO_SUCCESS);
	struct obs_source_info video = {.id = "visibility_video",
					.type = OBS_SOURCE_TYPE_INPUT,
					.output_flags = OBS_SOURCE_VIDEO,
					.get_name = input_name,
					.create = input_create,
					.destroy = input_destroy,
					.get_width = input_dimension,
					.get_height = input_dimension};
	obs_register_source(&video);
	video.id = "visibility_audio";
	video.output_flags = OBS_SOURCE_AUDIO;
	obs_register_source(&video);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	while (obs_wait_for_destroy_queue())
		;
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(immediate_video), cmocka_unit_test(remove_pending_audio),
					   cmocka_unit_test(loaded_and_duplicated_audio)};
	return cmocka_run_group_tests(tests, setup, teardown);
}
