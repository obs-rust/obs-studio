/* Public-API characterization before the Rust swap (Refs #4667).
 * The APIs use the common context/control block and stored destroy callback,
 * without dispatching on the type tag. These private-context fixtures avoid
 * unrelated source/output/encoder/service/canvas initialization and destruction.
 * The counter representation (owners minus one) and stale stored identity after
 * final release are characterized, not endorsed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct fixture {
	struct obs_context_data context;
	unsigned destroys;
	void *destroyed_object;
	long refs_at_destroy;
	long weak_refs_at_destroy;
};

static const enum obs_obj_type types[] = {
	OBS_OBJ_TYPE_INVALID, OBS_OBJ_TYPE_SOURCE,  OBS_OBJ_TYPE_OUTPUT,
	OBS_OBJ_TYPE_ENCODER, OBS_OBJ_TYPE_SERVICE, OBS_OBJ_TYPE_CANVAS,
};

static void record_destroy(void *object)
{
	struct fixture *fixture = object;
	fixture->destroys++;
	fixture->destroyed_object = object;
	fixture->refs_at_destroy = os_atomic_load_long(&fixture->context.control->ref.refs);
	fixture->weak_refs_at_destroy = os_atomic_load_long(&fixture->context.control->ref.weak_refs);
}

static void init_fixture(struct fixture *fixture, struct obs_weak_object *control, enum obs_obj_type type, long refs,
			 long weak_refs)
{
	memset(fixture, 0, sizeof(*fixture));
	memset(control, 0, sizeof(*control));
	fixture->context.type = type;
	fixture->context.control = control;
	fixture->context.destroy = record_destroy;
	control->object = &fixture->context;
	control->ref.refs = refs;
	control->ref.weak_refs = weak_refs;
}

struct warnings {
	unsigned count;
	bool exact;
};

static void capture_warning(int level, const char *message, va_list args, void *param)
{
	(void)args;
	struct warnings *warnings = param;
	warnings->count++;
	warnings->exact = warnings->exact && level == LOG_WARNING &&
			  strcmp(message, "Tried to release an object when the OBS core is shut down!") == 0;
}

static void without_core(void **state)
{
	(void)state;
	struct fixture fixture;
	struct obs_weak_object control;
	init_fixture(&fixture, &control, OBS_OBJ_TYPE_SOURCE, 0, 0);
	assert_false(obs_initialized());
	assert_null(obs_object_get_ref(NULL));
	/* Acquisition does not require an initialized core. */
	assert_ptr_equal(obs_object_get_ref(&fixture.context), &fixture.context);
	assert_int_equal(control.ref.refs, 1);

	log_handler_t previous;
	void *previous_param;
	struct warnings warnings = {0, true};
	base_get_log_handler(&previous, &previous_param);
	base_set_log_handler(capture_warning, &warnings);
	obs_object_release(NULL);
	obs_object_release(&fixture.context);
	base_set_log_handler(previous, previous_param);
	/* The shutdown guard precedes even the NULL check, and leaks the ref. */
	assert_int_equal(warnings.count, 2);
	assert_true(warnings.exact);
	assert_int_equal(control.ref.refs, 1);
	assert_int_equal(control.ref.weak_refs, 0);
	assert_int_equal(fixture.destroys, 0);
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

static void null_and_expired(void **state)
{
	(void)state;
	assert_null(obs_object_get_ref(NULL));
	struct warnings warnings = {0, true};
	log_handler_t previous;
	void *previous_param;
	base_get_log_handler(&previous, &previous_param);
	base_set_log_handler(capture_warning, &warnings);
	obs_object_release(NULL);
	base_set_log_handler(previous, previous_param);
	assert_int_equal(warnings.count, 0);

	for (size_t i = 0; i < OBS_COUNTOF(types); ++i) {
		struct fixture fixture;
		struct obs_weak_object control;
		init_fixture(&fixture, &control, types[i], 0, 3);
		fixture.context.control = NULL;
		assert_null(obs_object_get_ref(&fixture.context));
		fixture.context.control = &control;
		control.object = NULL;
		/* A live block with no stored identity still acquires a strong ref. */
		assert_null(obs_object_get_ref(&fixture.context));
		assert_int_equal(control.ref.refs, 1);
		obs_object_release(&fixture.context);
		assert_int_equal(control.ref.refs, 0);
		control.object = &fixture.context;
		for (long refs = -1; refs >= -3; --refs) {
			control.ref.refs = refs;
			assert_null(obs_object_get_ref(&fixture.context));
			assert_int_equal(control.ref.refs, refs);
			assert_int_equal(control.ref.weak_refs, 3);
			assert_int_equal(fixture.destroys, 0);
		}
	}
	/* Releasing a context with no control, or releasing an already expired
	 * object, is outside the valid ownership domain; do not exercise C UB. */
}

static void strong_counts_and_stored_identity(void **state)
{
	(void)state;
	static const long counts[] = {0, 1, 7, 1023};
	for (size_t i = 0; i < OBS_COUNTOF(types); ++i) {
		for (size_t j = 0; j < OBS_COUNTOF(counts); ++j) {
			struct fixture fixture;
			struct obs_weak_object control;
			init_fixture(&fixture, &control, types[i], counts[j], 4);
			assert_ptr_equal(obs_object_get_ref(&fixture.context), &fixture.context);
			assert_ptr_equal(obs_object_get_ref(&fixture.context), &fixture.context);
			assert_int_equal(control.ref.refs, counts[j] + 2);
			obs_object_release(&fixture.context);
			assert_int_equal(control.ref.refs, counts[j] + 1);
			obs_object_release(&fixture.context);
			assert_int_equal(control.ref.refs, counts[j]);
			assert_int_equal(control.ref.weak_refs, 4);
			assert_int_equal(fixture.destroys, 0);
			assert_ptr_equal(control.object, &fixture.context);
			assert_int_equal(fixture.context.type, types[i]);
		}
		struct fixture fixture;
		struct obs_weak_object control;
		init_fixture(&fixture, &control, types[i], 0, 0);
		struct obs_context_data alias = {.control = &control};
		/* It returns the control block's stored identity, not its argument. */
		assert_ptr_equal(obs_object_get_ref(&alias), &fixture.context);
		assert_int_equal(control.ref.refs, 1);
		obs_object_release(&fixture.context);
		assert_int_equal(control.ref.refs, 0);
		assert_int_equal(fixture.destroys, 0);
	}
}

static void last_release_and_weak_ownership(void **state)
{
	(void)state;
	for (size_t i = 0; i < OBS_COUNTOF(types); ++i) {
		struct fixture fixture;
		struct obs_weak_object *control = bzalloc(sizeof(*control));
		/* One external weak owner keeps the control alive for inspection. */
		init_fixture(&fixture, control, types[i], 0, 1);
		obs_object_release(&fixture.context);
		assert_int_equal(fixture.destroys, 1);
		assert_ptr_equal(fixture.destroyed_object, &fixture.context);
		assert_int_equal(fixture.refs_at_destroy, -1);
		assert_int_equal(fixture.weak_refs_at_destroy, 1);
		assert_int_equal(control->ref.refs, -1);
		assert_int_equal(control->ref.weak_refs, 0);
		assert_ptr_equal(control->object, &fixture.context);
		assert_null(obs_object_get_ref(&fixture.context));
		assert_null(obs_weak_object_get_object(control));
		assert_int_equal(control->ref.refs, -1);
		assert_int_equal(fixture.destroys, 1);
		obs_weak_object_release(control);

		/* With no external weak owner, the final release must free its control. */
		long allocations = bnum_allocs();
		control = bzalloc(sizeof(*control));
		init_fixture(&fixture, control, types[i], 0, 0);
		obs_object_release(&fixture.context);
		assert_int_equal(fixture.destroys, 1);
		assert_int_equal(fixture.refs_at_destroy, -1);
		assert_int_equal(fixture.weak_refs_at_destroy, 0);
		assert_int_equal(bnum_allocs(), allocations);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(without_core),
		cmocka_unit_test_setup_teardown(null_and_expired, setup, teardown),
		cmocka_unit_test_setup_teardown(strong_counts_and_stored_identity, setup, teardown),
		cmocka_unit_test_setup_teardown(last_release_and_weak_ownership, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
