/* Direct characterization before extraction (Refs #5695). Counts store owners
 * minus one; negative counts refuse promotion: characterized, not endorsed.
 * Every API call resolves from the selected real libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"

static void null_inputs(void **state)
{
	(void)state;
	obs_output_release(NULL);
	assert_null(obs_output_get_ref(NULL));
}

static void promotion_and_release(void **state)
{
	(void)state;
	struct obs_output output = {0};
	struct obs_weak_output control = {.output = &output};
	output.context.control = (struct obs_weak_object *)&control;
	const long counts[] = {0, 1, 7, 1023};
	for (size_t i = 0; i < OBS_COUNTOF(counts); ++i) {
		control.ref.refs = counts[i];
		control.ref.weak_refs = 3;
		obs_output_t *retained = obs_output_get_ref(&output);
		assert_ptr_equal(retained, &output);
		assert_int_equal(control.ref.refs, counts[i] + 1);
		assert_int_equal(control.ref.weak_refs, 3);
		obs_output_release(retained);
		assert_int_equal(control.ref.refs, counts[i]);
		assert_int_equal(control.ref.weak_refs, 3);
	}
	/* The synthetic output stays allocated: no dereference of a freed object. */
	for (long refs = -1; refs >= -3; --refs) {
		control.ref.refs = refs;
		assert_null(obs_output_get_ref(&output));
		assert_int_equal(control.ref.refs, refs);
		assert_int_equal(control.ref.weak_refs, 3);
	}
}

static unsigned destroy_calls;
static long expected_weak_refs;

static const char *output_name(void *type_data)
{
	(void)type_data;
	return "strong ref characterization";
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	return output;
}

static void output_destroy(void *data)
{
	obs_output_t *output = data;
	obs_weak_output_t *control = (obs_weak_output_t *)output->context.control;
	++destroy_calls;
	/* Removal precedes the callback, and the implicit weak owner survives it. */
	assert_null(obs_get_output_by_name(output->context.name));
	assert_int_equal(control->ref.refs, -1);
	assert_int_equal(control->ref.weak_refs, expected_weak_refs);
	assert_null(obs_output_get_ref(output));
	assert_int_equal(control->ref.refs, -1);
}

static bool output_start(void *data)
{
	(void)data;
	return false;
}

static void output_stop(void *data, uint64_t timestamp)
{
	(void)data;
	(void)timestamp;
}

static int setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	struct obs_output_info info = {
		.id = "strong-ref-characterization",
		.get_name = output_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = output_start,
		.stop = output_stop,
	};
	obs_register_output(&info);
	destroy_calls = 0;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static void final_release_with_weak_owner(void **state)
{
	(void)state;
	obs_output_t *output = obs_output_create("strong-ref-characterization", "strong refs", NULL, NULL);
	assert_non_null(output);
	obs_weak_output_t *control = obs_output_get_weak_output(output);
	expected_weak_refs = 1;
	assert_int_equal(control->ref.refs, 0);
	obs_output_t *retained = obs_output_get_ref(output);
	assert_ptr_equal(retained, output);
	assert_int_equal(control->ref.refs, 1);
	obs_output_release(output);
	assert_int_equal(destroy_calls, 0);
	assert_int_equal(control->ref.refs, 0);
	assert_string_equal(obs_output_get_name(retained), "strong refs");
	obs_output_release(retained);
	assert_int_equal(destroy_calls, 1);
	assert_int_equal(control->ref.refs, -1);
	assert_int_equal(control->ref.weak_refs, 0);
	assert_null(obs_weak_output_get_output(control));
	const long allocations = bnum_allocs();
	obs_weak_output_release(control);
	assert_int_equal(bnum_allocs(), allocations - 1);
}

static void final_release_without_weak_owner(void **state)
{
	(void)state;
	const long allocations = bnum_allocs();
	obs_output_t *output = obs_output_create("strong-ref-characterization", "sole strong ref", NULL, NULL);
	assert_non_null(output);
	expected_weak_refs = 0;
	obs_output_release(output);
	assert_int_equal(destroy_calls, 1);
	assert_int_equal(bnum_allocs(), allocations);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_inputs),
		cmocka_unit_test(promotion_and_release),
		cmocka_unit_test_setup_teardown(final_release_with_weak_owner, setup, teardown),
		cmocka_unit_test_setup_teardown(final_release_without_weak_owner, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
