/* Characterization of the source strong-reference entries (#5818):
 * obs_source_release(), obs_source_get_ref() and
 * obs_weak_source_get_source(). The fourth, obs_source_addref(), is declared
 * in obs-internal.h without EXPORT and its one caller (a group's reorder in
 * obs_scene_reorder_items2()) releases right after, so its count can't be
 * seen from outside libobs: Tier 1 and Tier 3 cover it.
 *
 * What it pins, on private sources of a started core, reading the control's
 * counts (the strong count is biased by one: 0 is the last owner):
 *
 *   - NULL: release does nothing, get_ref and the weak promotion return
 *     NULL, and nothing is logged.
 *   - get_ref adds 1 to refs and release takes 1 off, leaving weak_refs
 *     alone; while refs stays at 0 or above, release never destroys.
 *   - The release that takes refs from 0 to -1 destroys the source once
 *     (its destroy callback runs on the destruction thread) and then drops
 *     the source's own weak reference: a weak handle held across it sees
 *     refs at -1 and its weak count back where it was before it was taken.
 *   - get_ref on a live source returns the same pointer and adds 1 to refs;
 *     the weak promotion of its control does the same.
 *   - get_ref and the weak promotion refuse a source whose refs is -1 (being
 *     destroyed): NULL, and refs stays -1. Any count above -1 is promoted.
 *   - With the core shut down, release of a non-NULL source logs
 *     "Tried to release a source when the OBS core is shut down!" at
 *     LOG_WARNING once and touches nothing; release(NULL) logs nothing.
 *
 * obs_startup() needs a display on Linux, so the test runs under xvfb-run
 * when it is installed. */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#define NODE_ID "test_source_refs_node"

/* ------------------------------------------------------------------------- */
/* log capture */

static unsigned log_count;
static int log_level;
static char log_line[256];

#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_line, sizeof(log_line), format, args);
}

static log_handler_t saved_handler;
static void *saved_param;

static void capture_start(void)
{
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	log_level = -1;
	log_line[0] = '\0';
}

static void capture_stop(void)
{
	base_set_log_handler(saved_handler, saved_param);
}

/* ------------------------------------------------------------------------- */
/* the node source type */

static volatile long destroyed;

static const char *node_name(void *type_data)
{
	(void)type_data;
	return "refs node";
}

static void *node_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return bzalloc(1);
}

static void node_destroy(void *data)
{
	os_atomic_inc_long(&destroyed);
	bfree(data);
}

static uint32_t node_size(void *data)
{
	(void)data;
	return 16;
}

static void register_node(void)
{
	struct obs_source_info info = {
		.id = NODE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = node_name,
		.create = node_create,
		.destroy = node_destroy,
		.get_width = node_size,
		.get_height = node_size,
	};
	obs_register_source(&info);
}

/* ------------------------------------------------------------------------- */
/* helpers */

static obs_source_t *source;

static obs_weak_source_t *control_of(obs_source_t *s)
{
	return (obs_weak_source_t *)s->context.control;
}

static long refs_of(obs_weak_source_t *weak)
{
	return os_atomic_load_long(&weak->ref.refs);
}

static long weak_refs_of(obs_weak_source_t *weak)
{
	return os_atomic_load_long(&weak->ref.weak_refs);
}

static int fixture_setup(void **state)
{
	(void)state;
	/* A teardown's destroy from the previous case has finished. */
	obs_wait_for_destroy_queue();
	os_atomic_set_long(&destroyed, 0);
	source = obs_source_create_private(NODE_ID, "refs", NULL);
	assert_non_null(source);
	assert_int_equal(refs_of(control_of(source)), 0);
	return 0;
}

static int fixture_teardown(void **state)
{
	(void)state;
	if (source)
		obs_source_release(source);
	source = NULL;
	/* Sources are destroyed on the destruction thread, which logs. Let it
	 * finish here, so those lines never land in the next test's capture. */
	obs_wait_for_destroy_queue();
	return 0;
}

/* ------------------------------------------------------------------------- */
/* cases */

static void null_inputs_do_nothing(void **state)
{
	(void)state;
	capture_start();
	obs_source_release(NULL);
	obs_source_t *ref = obs_source_get_ref(NULL);
	obs_source_t *promoted = obs_weak_source_get_source(NULL);
	capture_stop();

	assert_null(ref);
	assert_null(promoted);
	assert_int_equal(log_count, 0);
	assert_int_equal(refs_of(control_of(source)), 0);
}

static void get_ref_and_release_move_only_refs(void **state)
{
	(void)state;
	obs_weak_source_t *weak = control_of(source);
	long weak_before = weak_refs_of(weak);

	capture_start();
	assert_ptr_equal(obs_source_get_ref(source), source);
	assert_int_equal(refs_of(weak), 1);
	assert_ptr_equal(obs_source_get_ref(source), source);
	assert_int_equal(refs_of(weak), 2);
	obs_source_release(source);
	assert_int_equal(refs_of(weak), 1);
	obs_source_release(source);
	assert_int_equal(refs_of(weak), 0);
	capture_stop();

	assert_int_equal(weak_refs_of(weak), weak_before);
	assert_int_equal(log_count, 0);
	obs_wait_for_destroy_queue();
	assert_int_equal(os_atomic_load_long(&destroyed), 0);
}

static void final_release_destroys_then_drops_its_weak_ref(void **state)
{
	(void)state;
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	assert_ptr_equal(weak, control_of(source));
	long weak_held = weak_refs_of(weak);

	obs_source_release(source);
	source = NULL;
	obs_wait_for_destroy_queue();

	assert_int_equal(os_atomic_load_long(&destroyed), 1);
	assert_int_equal(refs_of(weak), -1);
	/* The source's own weak reference is gone; ours is the last. */
	assert_int_equal(weak_refs_of(weak), weak_held - 1);
	assert_int_equal(weak_refs_of(weak), 0);

	/* A source being destroyed is not promoted, and refs stays -1. */
	assert_null(obs_weak_source_get_source(weak));
	assert_int_equal(refs_of(weak), -1);
	obs_weak_source_release(weak);
}

static void get_ref_of_a_live_source_adds_one(void **state)
{
	(void)state;
	obs_weak_source_t *weak = control_of(source);

	capture_start();
	obs_source_t *ref = obs_source_get_ref(source);
	capture_stop();

	assert_ptr_equal(ref, source);
	assert_int_equal(refs_of(weak), 1);
	assert_int_equal(log_count, 0);
	obs_source_release(ref);
	assert_int_equal(refs_of(weak), 0);
}

static void weak_promotion_of_a_live_source_adds_one(void **state)
{
	(void)state;
	obs_weak_source_t *weak = control_of(source);

	obs_source_t *ref = obs_weak_source_get_source(weak);
	assert_ptr_equal(ref, source);
	assert_int_equal(refs_of(weak), 1);
	obs_source_release(ref);
	assert_int_equal(refs_of(weak), 0);
}

static void any_count_above_minus_one_is_promoted(void **state)
{
	(void)state;
	obs_weak_source_t *weak = control_of(source);

	os_atomic_set_long(&weak->ref.refs, 41);
	assert_ptr_equal(obs_source_get_ref(source), source);
	assert_int_equal(refs_of(weak), 42);
	os_atomic_set_long(&weak->ref.refs, 0);
}

static void a_dying_source_is_refused(void **state)
{
	(void)state;
	obs_weak_source_t *weak = control_of(source);

	/* What a source being destroyed looks like to a late caller. */
	os_atomic_set_long(&weak->ref.refs, -1);
	capture_start();
	obs_source_t *ref = obs_source_get_ref(source);
	obs_source_t *promoted = obs_weak_source_get_source(weak);
	capture_stop();
	long refs = refs_of(weak);
	os_atomic_set_long(&weak->ref.refs, 0);

	assert_null(ref);
	assert_null(promoted);
	assert_int_equal(refs, -1);
	assert_int_equal(log_count, 0);
	assert_int_equal(os_atomic_load_long(&destroyed), 0);
}

/* Run with the core shut down. */
static void release_after_shutdown_warns_and_touches_nothing(void **state)
{
	(void)state;
	struct obs_source dummy;
	memset(&dummy, 0x5a, sizeof(dummy));
	struct obs_source before;
	memcpy(&before, &dummy, sizeof(dummy));

	assert_false(obs_initialized());
	capture_start();
	obs_source_release(&dummy);
	unsigned count = log_count;
	int level = log_level;
	char line[sizeof(log_line)];
	memcpy(line, log_line, sizeof(line));
	obs_source_release(NULL);
	unsigned count_after_null = log_count;
	capture_stop();

	assert_int_equal(count, 1);
	assert_int_equal(level, LOG_WARNING);
	assert_string_equal(line, "Tried to release a source when the OBS core is shut down!");
	assert_int_equal(count_after_null, 1);
	assert_memory_equal(&dummy, &before, sizeof(dummy));
}

/* ------------------------------------------------------------------------- */

static int group_setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	register_node();
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

#define CASE(f) cmocka_unit_test_setup_teardown(f, fixture_setup, fixture_teardown)

int main(void)
{
	const struct CMUnitTest tests[] = {
		CASE(null_inputs_do_nothing),
		CASE(get_ref_and_release_move_only_refs),
		CASE(final_release_destroys_then_drops_its_weak_ref),
		CASE(get_ref_of_a_live_source_adds_one),
		CASE(weak_promotion_of_a_live_source_adds_one),
		CASE(any_count_above_minus_one_is_promoted),
		CASE(a_dying_source_is_refused),
	};
	const struct CMUnitTest shut_down[] = {
		cmocka_unit_test(release_after_shutdown_warns_and_touches_nothing),
	};
	int failed = cmocka_run_group_tests_name("source_refs", tests, group_setup, group_teardown);
	failed += cmocka_run_group_tests_name("source_refs_shut_down", shut_down, NULL, NULL);
	return failed;
}
