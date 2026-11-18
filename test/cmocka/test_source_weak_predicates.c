/* Characterize the exported weak-source predicates before extraction (Refs #5015).
 * Run unchanged against hybrid libobs and libobs-c-reference. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>

static void null_weak(void **state)
{
	(void)state;
	assert_true(obs_weak_source_expired(NULL));
	assert_false(obs_weak_source_references_source(NULL, NULL));
}

static void live_identity(void **state)
{
	(void)state;
	obs_scene_t *scene = obs_scene_create_private("weak predicate owner");
	obs_scene_t *other_scene = obs_scene_create_private("weak predicate other");
	assert_non_null(scene);
	assert_non_null(other_scene);
	obs_source_t *source = obs_scene_get_source(scene);
	obs_source_t *other = obs_scene_get_source(other_scene);
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	obs_weak_source_t *other_weak = obs_source_get_weak_source(other);
	assert_non_null(weak);
	assert_non_null(other_weak);

	for (unsigned i = 0; i < 3; i++) {
		assert_false(obs_weak_source_expired(weak));
		assert_false(obs_weak_source_expired(other_weak));
		assert_true(obs_weak_source_references_source(weak, source));
		assert_false(obs_weak_source_references_source(weak, other));
		assert_true(obs_weak_source_references_source(other_weak, other));
		assert_false(obs_weak_source_references_source(other_weak, source));
		assert_false(obs_weak_source_references_source(weak, NULL));
		assert_false(obs_weak_source_references_source(NULL, source));
	}

	obs_scene_release(scene);
	obs_scene_release(other_scene);
	while (obs_wait_for_destroy_queue())
		;
	/* Predicate calls must not retain either owner. */
	assert_true(obs_weak_source_expired(weak));
	assert_true(obs_weak_source_expired(other_weak));
	obs_weak_source_release(weak);
	obs_weak_source_release(other_weak);
}

struct expiry_probe {
	obs_source_t *source;
	obs_source_t *other;
	obs_weak_source_t *weak;
	bool expired;
	bool same_source;
	bool different_source;
	bool null_source;
};

static void release_last_strong(void *data)
{
	struct expiry_probe *probe = data;
	obs_source_release(probe->source);
	/* Deferred destruction is queued behind this task. The source is still
	 * allocated here, so identity never compares a freed pointer. Identity
	 * remains true after expiry: characterized, not endorsed. */
	probe->expired = obs_weak_source_expired(probe->weak);
	probe->same_source = obs_weak_source_references_source(probe->weak, probe->source);
	probe->different_source = obs_weak_source_references_source(probe->weak, probe->other);
	probe->null_source = obs_weak_source_references_source(probe->weak, NULL);
}

static void last_strong_release(void **state)
{
	(void)state;
	obs_scene_t *scene = obs_scene_create_private("weak predicate expiry");
	obs_scene_t *other_scene = obs_scene_create_private("weak predicate survivor");
	assert_non_null(scene);
	assert_non_null(other_scene);
	obs_source_t *source = obs_scene_get_source(scene);
	struct expiry_probe probe = {
		.source = obs_source_get_ref(source),
		.other = obs_scene_get_source(other_scene),
		.weak = obs_source_get_weak_source(source),
	};
	assert_non_null(probe.source);
	assert_non_null(probe.weak);
	obs_scene_release(scene);
	while (obs_wait_for_destroy_queue())
		;
	assert_false(obs_weak_source_expired(probe.weak));
	assert_true(obs_weak_source_references_source(probe.weak, probe.source));

	obs_queue_task(OBS_TASK_DESTROY, release_last_strong, &probe, true);
	while (obs_wait_for_destroy_queue())
		;
	assert_true(probe.expired);
	assert_true(probe.same_source);
	assert_false(probe.different_source);
	assert_false(probe.null_source);
	assert_true(obs_weak_source_expired(probe.weak));
	assert_null(obs_weak_source_get_source(probe.weak));
	assert_false(obs_weak_source_references_source(probe.weak, probe.other));
	assert_false(obs_weak_source_references_source(probe.weak, NULL));
	obs_weak_source_release(probe.weak);
	obs_scene_release(other_scene);
	while (obs_wait_for_destroy_queue())
		;
}

int main(void)
{
	null_weak(NULL);
	if (!obs_startup("en-US", NULL, NULL))
		return 1;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_weak),
		cmocka_unit_test(live_identity),
		cmocka_unit_test(last_strong_release),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	null_weak(NULL);
	return result;
}
