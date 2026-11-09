/* Characterize the live shutdown path, not a local copy of its helpers.
 * The host leaks a scene deliberately; shutdown owns reclaiming it. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs-internal.h>
#include <util/bmem.h>

#define INPUT_ID "shutdown_detach_input"
#define SOURCE_COUNT 6
#define ITEM_COUNT 7

struct detach_state {
	obs_weak_source_t *weak[SOURCE_COUNT];
	pthread_mutex_t *table_mutex;
	unsigned removed;
	unsigned destroyed;
	unsigned tasks;
	bool unlocked;
	bool retained;
};

static struct detach_state teardown;

static const char *input_name(void *unused)
{
	(void)unused;
	return "Shutdown detach input";
}

static void *input_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return &teardown;
}

static void second_task(void *data)
{
	struct detach_state *s = data;
	s->tasks++;
}

static void first_task(void *data)
{
	struct detach_state *s = data;
	s->tasks++;
	obs_queue_task(OBS_TASK_DESTROY, second_task, data, false);
}

static void input_destroy(void *data)
{
	struct detach_state *s = data;
	s->destroyed++;
	obs_queue_task(OBS_TASK_DESTROY, first_task, data, false);
}

/* A same-thread trylock would accept the recursive source-table mutex even
 * if shutdown held it. Probe from another thread, without a blocking lock. */
static void *probe_table_lock(void *data)
{
	struct detach_state *s = data;
	if (pthread_mutex_trylock(s->table_mutex) == 0) {
		pthread_mutex_unlock(s->table_mutex);
	} else {
		s->unlocked = false;
	}
	return NULL;
}

static void item_removed(void *data, calldata_t *cd)
{
	(void)cd;
	struct detach_state *s = data;
	pthread_t probe;
	assert_int_equal(pthread_create(&probe, NULL, probe_table_lock, s), 0);
	assert_int_equal(pthread_join(probe, NULL), 0);
	s->removed++;

	/* Force queued destruction to finish before checking the snapshot's
	 * lifetimes. No scheduler timing or sleep is part of this assertion. */
	while (obs_wait_for_destroy_queue())
		;
	for (size_t i = 0; i < SOURCE_COUNT; i++) {
		obs_source_t *source = obs_weak_source_get_source(s->weak[i]);
		if (!source)
			s->retained = false;
		obs_source_release(source);
	}
	assert_int_equal(s->destroyed, 0);
}

static void watch_scene(obs_scene_t *scene)
{
	signal_handler_connect(obs_source_get_signal_handler(obs_scene_get_source(scene)), "item_remove", item_removed,
			       &teardown);
}

static void test_shutdown_detaches_snapshot(void **unused)
{
	(void)unused;
	long before = bnum_allocs();
	assert_true(obs_startup("en-US", NULL, NULL));
	teardown = (struct detach_state){.unlocked = true, .retained = true};
	struct obs_source_info info = {
		.id = INPUT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = input_name,
		.create = input_create,
		.destroy = input_destroy,
	};
	obs_register_source(&info);

	obs_scene_t *outer = obs_scene_create_private("outer leaked by host");
	obs_scene_t *inner = obs_scene_create_private("scene owned by outer");
	assert_non_null(outer);
	assert_non_null(inner);
	obs_sceneitem_t *group_item = obs_scene_add_group(inner, "group");
	assert_non_null(group_item);
	obs_scene_t *group = obs_sceneitem_group_get_scene(group_item);
	assert_non_null(group);
	obs_sceneitem_t *nested_item = obs_scene_add_group(group, "nested group");
	assert_non_null(nested_item);
	obs_scene_t *nested = obs_sceneitem_group_get_scene(nested_item);
	assert_non_null(nested);
	obs_source_t *first = obs_source_create_private(INPUT_ID, "shared private input", NULL);
	obs_source_t *second = obs_source_create(INPUT_ID, "public input", NULL, NULL);
	assert_non_null(first);
	assert_non_null(second);
	assert_non_null(obs_scene_add(outer, obs_scene_get_source(inner)));
	assert_non_null(obs_scene_add(outer, first));
	assert_non_null(obs_scene_add(inner, second));
	assert_non_null(obs_scene_add(nested, first));
	assert_non_null(obs_scene_add(nested, second));

	obs_source_t *sources[SOURCE_COUNT] = {obs_scene_get_source(outer),
					       obs_scene_get_source(inner),
					       obs_scene_get_source(group),
					       obs_scene_get_source(nested),
					       first,
					       second};
	for (size_t i = 0; i < SOURCE_COUNT; i++) {
		teardown.weak[i] = obs_source_get_weak_source(sources[i]);
		assert_non_null(teardown.weak[i]);
	}
	/* The UUID-table registration records the real sources_mutex here.
	 * Borrow it only while shutdown is removing items, before core teardown. */
	teardown.table_mutex = first->context.mutex;
	assert_non_null(teardown.table_mutex);
	watch_scene(outer);
	watch_scene(inner);
	watch_scene(group);
	watch_scene(nested);
	obs_source_release(first);
	obs_source_release(second);
	obs_scene_release(inner);

	obs_shutdown();
	assert_int_equal(teardown.removed, ITEM_COUNT);
	assert_true(teardown.unlocked);
	assert_true(teardown.retained);
	assert_int_equal(teardown.destroyed, 2);
	assert_int_equal(teardown.tasks, 4);
	/* Characterized, not endorsed: forced reclamation of the host-leaked
	 * outer scene frees its object without expiring its strong counter.
	 * Never promote that dangling weak target after shutdown. */
	assert_false(obs_weak_source_expired(teardown.weak[0]));
	obs_weak_source_release(teardown.weak[0]);
	for (size_t i = 1; i < SOURCE_COUNT; i++) {
		assert_true(obs_weak_source_expired(teardown.weak[i]));
		assert_null(obs_weak_source_get_source(teardown.weak[i]));
		obs_weak_source_release(teardown.weak[i]);
	}
	assert_int_equal(bnum_allocs(), before);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(test_shutdown_detaches_snapshot)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
