/* Characterize the real exported scene lifetime wrappers before the Rust swap.
 * Run unchanged against hybrid libobs and libobs-c-reference. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs.h>

struct destroy_probe {
	obs_scene_t *scene;
	obs_source_t *source;
	obs_weak_source_t *weak;
	unsigned destroyed;
	bool identity_matches;
	bool expired_after_release;
	obs_scene_t *expired_promotion;
};

static void destroyed(void *data, calldata_t *params)
{
	struct destroy_probe *probe = data;
	probe->destroyed++;
	probe->identity_matches = calldata_ptr(params, "source") == probe->source;
}

static void release_last_on_destroy_queue(void *data)
{
	struct destroy_probe *probe = data;
	obs_scene_release(probe->scene);
	probe->expired_after_release = obs_weak_source_expired(probe->weak);
	/* The deferred destroy is queued behind this task on the same thread.
	 * Both scene and source are still allocated, while the held weak control
	 * is expired. This pins failed promotion without a scheduling race. */
	probe->expired_promotion = obs_scene_get_ref(probe->scene);
}

static void null_scene(void **state)
{
	(void)state;
	assert_null(obs_scene_get_ref(NULL));
	obs_scene_release(NULL);
}

static void exercise_refs(unsigned acquisitions)
{
	obs_scene_t *scene = obs_scene_create_private("scene ref characterization");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	assert_non_null(source);
	struct destroy_probe probe = {.scene = scene, .source = source, .weak = obs_source_get_weak_source(source)};
	assert_non_null(probe.weak);
	signal_handler_connect(obs_source_get_signal_handler(source), "destroy", destroyed, &probe);

	for (unsigned i = 0; i < acquisitions; i++) {
		assert_ptr_equal(obs_scene_get_ref(scene), scene);
		assert_ptr_equal(obs_scene_get_source(scene), source);
	}

	/* A private scene has no canvas-owned strong reference. Each acquisition
	 * must keep precisely one source reference after the original is gone. */
	for (unsigned i = 0; i < acquisitions; i++) {
		obs_scene_release(scene);
		while (obs_wait_for_destroy_queue())
			;
		assert_false(obs_weak_source_expired(probe.weak));
		assert_int_equal(probe.destroyed, 0);
		obs_source_t *retained = obs_weak_source_get_source(probe.weak);
		assert_ptr_equal(retained, source);
		assert_ptr_equal(obs_scene_from_source(retained), scene);
		obs_source_release(retained);
	}

	/* Synchronous queue execution orders the failed promotion before the
	 * deferred free; draining afterwards observes destruction itself. */
	obs_queue_task(OBS_TASK_DESTROY, release_last_on_destroy_queue, &probe, true);
	while (obs_wait_for_destroy_queue())
		;
	assert_true(probe.expired_after_release);
	assert_null(probe.expired_promotion);
	assert_true(obs_weak_source_expired(probe.weak));
	assert_null(obs_weak_source_get_source(probe.weak));
	assert_int_equal(probe.destroyed, 1);
	assert_true(probe.identity_matches);
	obs_weak_source_release(probe.weak);
}

static void last_reference(void **state)
{
	(void)state;
	exercise_refs(0);
}

static void retained_reference(void **state)
{
	(void)state;
	exercise_refs(1);
}

static void repeated_references(void **state)
{
	(void)state;
	exercise_refs(2);
	exercise_refs(7);
	exercise_refs(32);
}

int main(void)
{
	/* NULL is safe even without an initialized core. */
	assert_null(obs_scene_get_ref(NULL));
	obs_scene_release(NULL);
	if (!obs_startup("en-US", NULL, NULL))
		return 1;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_scene),
		cmocka_unit_test(last_reference),
		cmocka_unit_test(retained_reference),
		cmocka_unit_test(repeated_references),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	assert_null(obs_scene_get_ref(NULL));
	obs_scene_release(NULL);
	return result;
}
