/* Characterize callback and reference lifetime before the Rust port. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>
#include <util/bmem.h>

struct callback_state {
	obs_scene_t *expected;
	obs_scene_t *received;
	unsigned calls;
	bool release_caller;
	bool alive_after_release;
	obs_weak_source_t *weak;
	unsigned destroy_calls;
	bool expired_at_destroy;
};

static void update_callback(void *data, obs_scene_t *scene)
{
	struct callback_state *s = data;
	s->calls++;
	s->received = scene;
	if (s->release_caller) {
		obs_scene_release(scene);
		s->alive_after_release = !obs_weak_source_expired(s->weak);
	}
}

static void destroy_callback(void *data, calldata_t *params)
{
	struct callback_state *s = data;
	(void)params;
	s->destroy_calls++;
	s->expired_at_destroy = obs_weak_source_expired(s->weak);
	/* The source is expired, but scene_destroy has not freed its scene yet.
	 * This safely reaches the failed-get-ref path without a dangling pointer. */
	obs_scene_atomic_update(s->expected, update_callback, s);
}

static void destruction_barrier(void *data)
{
	(void)data;
}

static void null_scene(void **state)
{
	(void)state;
	struct callback_state s = {0};
	obs_scene_atomic_update(NULL, update_callback, &s);
	assert_int_equal(s.calls, 0);
}

static void run_scene_case(bool removed, bool release_caller)
{
	long before = bnum_allocs();
	assert_true(obs_startup("en-US", NULL, NULL));
	obs_scene_t *scene = obs_scene_create_private("atomic update characterization");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	struct callback_state s = {
		.expected = scene,
		.release_caller = release_caller,
		.weak = obs_source_get_weak_source(source),
	};
	assert_non_null(s.weak);
	signal_handler_connect(obs_source_get_signal_handler(source), "destroy", destroy_callback, &s);
	if (removed) {
		obs_source_remove(source);
		assert_true(obs_source_removed(source));
	}
	/* Characterized, not endorsed: removal alone does not expire a scene,
	 * so a removed scene held by the caller still receives the callback. */
	obs_scene_atomic_update(scene, update_callback, &s);
	assert_int_equal(s.calls, 1);
	assert_ptr_equal(s.received, scene);
	if (release_caller)
		assert_true(s.alive_after_release);
	else
		obs_scene_release(scene);
	/* A leaked temporary reference would keep this false, even if shutdown
	 * later forcibly reclaimed the source. */
	assert_true(obs_weak_source_expired(s.weak));
	obs_queue_task(OBS_TASK_DESTROY, destruction_barrier, NULL, true);
	assert_int_equal(s.destroy_calls, 1);
	assert_true(s.expired_at_destroy);
	assert_int_equal(s.calls, 1);
	obs_weak_source_release(s.weak);
	obs_shutdown();
	assert_int_equal(bnum_allocs(), before);
}

static void live_scene_and_expired_scene(void **state)
{
	(void)state;
	run_scene_case(false, false);
}

static void removed_scene_and_expired_scene(void **state)
{
	(void)state;
	run_scene_case(true, false);
}

static void callback_holds_last_reference(void **state)
{
	(void)state;
	run_scene_case(false, true);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_scene),
		cmocka_unit_test(live_scene_and_expired_scene),
		cmocka_unit_test(removed_scene_and_expired_scene),
		cmocka_unit_test(callback_holds_last_reference),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
