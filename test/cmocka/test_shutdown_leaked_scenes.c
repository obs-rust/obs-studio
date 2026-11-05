/* A scene the host leaves alive must release its items before shutdown
 * destroys their sources (#3458). Public API only, for both libobs builds. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdatomic.h>
#include <obs.h>
#include <util/bmem.h>

#define INPUT_ID "shutdown_leaked_scene_input"

struct teardown_state {
	atomic_uint destroyed;
	atomic_uint removed;
	atomic_bool destroyed_before_remove;
};

static struct teardown_state teardown;

static const char *input_name(void *data)
{
	(void)data;
	return "Leaked scene shutdown input";
}

static void *input_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return &teardown;
}

static void input_destroy(void *data)
{
	struct teardown_state *s = data;
	atomic_fetch_add(&s->destroyed, 1);
}

static void item_removed(void *data, calldata_t *cd)
{
	(void)cd;
	struct teardown_state *s = data;
	if (atomic_load(&s->destroyed))
		atomic_store(&s->destroyed_before_remove, true);
	atomic_fetch_add(&s->removed, 1);
}

static void watch_items(obs_scene_t *scene)
{
	signal_handler_connect(obs_source_get_signal_handler(obs_scene_get_source(scene)), "item_remove", item_removed,
			       &teardown);
}

static void run_shutdown_case(bool private_input, bool release_input, bool nested)
{
	long before = bnum_allocs();
	if (!obs_startup("en-US", NULL, NULL))
		skip();
	atomic_store(&teardown.destroyed, 0);
	atomic_store(&teardown.removed, 0);
	atomic_store(&teardown.destroyed_before_remove, false);
	struct obs_source_info info = {
		.id = INPUT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = input_name,
		.create = input_create,
		.destroy = input_destroy,
	};
	obs_register_source(&info);

	/* Insert the input before the scene in the UUID table too. The old
	 * public-name walk always destroyed a public input before any scene. */
	obs_source_t *input = private_input ? obs_source_create_private(INPUT_ID, "input", NULL)
					    : obs_source_create(INPUT_ID, "input", NULL, NULL);
	obs_scene_t *scene = obs_scene_create("host keeps this scene");
	assert_non_null(input);
	assert_non_null(scene);
	obs_scene_t *owner = scene;
	if (nested) {
		obs_sceneitem_t *group_item = obs_scene_add_group(scene, "nested group");
		assert_non_null(group_item);
		owner = obs_sceneitem_group_get_scene(group_item);
		assert_non_null(owner);
	}
	watch_items(owner);
	assert_non_null(obs_scene_add(owner, input));
	if (release_input)
		obs_source_release(input);

	/* Deliberately retain the host's scene reference. The shutdown cleanup
	 * owns reclaiming it and must not free the input out from under it. */
	obs_shutdown();
	assert_int_equal(atomic_load(&teardown.removed), 1);
	assert_int_equal(atomic_load(&teardown.destroyed), 1);
	assert_false(atomic_load(&teardown.destroyed_before_remove));
	assert_int_equal(bnum_allocs(), before);
}

static void public_input_held_by_host(void **state)
{
	(void)state;
	run_shutdown_case(false, false, false);
}

static void private_input_held_by_host(void **state)
{
	(void)state;
	run_shutdown_case(true, false, false);
}

static void input_owned_only_by_scene(void **state)
{
	(void)state;
	run_shutdown_case(false, true, false);
}

static void input_owned_only_by_nested_group(void **state)
{
	(void)state;
	run_shutdown_case(true, true, true);
}

static void sentinel_destroy(void *data)
{
	(void)data;
}

static void finish_scene_destroy(void *data, calldata_t *call)
{
	(void)call;
	unsigned *callbacks = data;
	++*callbacks;
	/* filter_remove runs on the shutdown thread. Finish the earlier scene's
	 * deferred destroy before the hash walk continues to its cached next
	 * source, which the scene alone owned. */
	while (obs_wait_for_destroy_queue())
		;
	assert_int_equal(atomic_load(&teardown.destroyed), 1);
}

static void scene_destroy_mutates_next_hash_entry(void **state)
{
	(void)state;
	long before = bnum_allocs();
	if (!obs_startup("en-US", NULL, NULL))
		skip();
	atomic_store(&teardown.destroyed, 0);
	atomic_store(&teardown.removed, 0);
	atomic_store(&teardown.destroyed_before_remove, false);
	struct obs_source_info input_info = {
		.id = INPUT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = input_name,
		.create = input_create,
		.destroy = input_destroy,
	};
	struct obs_source_info sentinel_info = {
		.id = "shutdown_hash_sentinel",
		.destroy = sentinel_destroy,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = input_name,
		.create = input_create,
	};
	struct obs_source_info filter_info = {
		.id = "shutdown_hash_filter",
		.destroy = sentinel_destroy,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = input_name,
		.create = input_create,
	};
	obs_register_source(&input_info);
	obs_register_source(&sentinel_info);
	obs_register_source(&filter_info);
	/* Private sources bypass the public-name walk. UUID insertion order:
	 * scene -> sentinel -> child -> filter. */
	obs_scene_t *scene = obs_scene_create_private("scene before child");
	obs_source_t *sentinel = obs_source_create_private(sentinel_info.id, "sentinel", NULL);
	obs_source_t *child = obs_source_create_private(INPUT_ID, "child", NULL);
	obs_source_t *filter = obs_source_create_private(filter_info.id, "filter", NULL);
	assert_non_null(scene);
	assert_non_null(sentinel);
	assert_non_null(child);
	assert_non_null(filter);
	watch_items(scene);
	assert_non_null(obs_scene_add(scene, child));
	obs_source_release(child);
	obs_source_filter_add(sentinel, filter);
	assert_int_equal(obs_source_filter_count(sentinel), 1);
	unsigned callbacks = 0;
	signal_handler_connect(obs_source_get_signal_handler(sentinel), "filter_remove", finish_scene_destroy,
			       &callbacks);
	obs_shutdown();
	assert_int_equal(callbacks, 1);
	assert_int_equal(atomic_load(&teardown.removed), 1);
	assert_int_equal(atomic_load(&teardown.destroyed), 1);
	assert_false(atomic_load(&teardown.destroyed_before_remove));
	assert_int_equal(bnum_allocs(), before);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(public_input_held_by_host),
		cmocka_unit_test(private_input_held_by_host),
		cmocka_unit_test(input_owned_only_by_scene),
		cmocka_unit_test(input_owned_only_by_nested_group),
		cmocka_unit_test(scene_destroy_mutates_next_hash_entry),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
