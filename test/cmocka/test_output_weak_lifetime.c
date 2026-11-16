/* Characterization before extraction (Refs #4768). Counts store owners minus
 * one, and identity compares the stored pointer even for an expired handle:
 * characterized, not endorsed. All calls resolve from the selected real libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"

static void null_inputs(void **state)
{
	(void)state;
	obs_weak_output_addref(NULL);
	obs_weak_output_release(NULL);
	assert_null(obs_output_get_weak_output(NULL));
	assert_null(obs_weak_output_get_output(NULL));
	assert_false(obs_weak_output_references_output(NULL, NULL));
}

static void weak_ownership(void **state)
{
	(void)state;
	struct obs_output output = {0};
	const long allocations = bnum_allocs();
	struct obs_weak_output *weak = bzalloc(sizeof(*weak));
	weak->output = &output;
	output.context.control = (struct obs_weak_object *)weak;
	assert_ptr_equal(obs_output_get_weak_output(&output), weak);
	assert_int_equal(weak->ref.weak_refs, 1);
	assert_ptr_equal(obs_output_get_weak_output(&output), weak);
	assert_int_equal(weak->ref.weak_refs, 2);
	obs_weak_output_addref(weak);
	assert_int_equal(weak->ref.weak_refs, 3);
	obs_weak_output_release(weak);
	assert_int_equal(weak->ref.weak_refs, 2);
	obs_weak_output_release(weak);
	obs_weak_output_release(weak);
	assert_int_equal(weak->ref.weak_refs, 0);
	assert_int_equal(weak->ref.refs, 0);
	assert_ptr_equal(weak->output, &output);
	assert_int_equal(bnum_allocs(), allocations + 1);
	/* Expire the synthetic owner, then drop the implicit weak ownership. */
	weak->ref.refs = -1;
	obs_weak_output_release(weak);
	assert_int_equal(bnum_allocs(), allocations);
}

static void promotion_and_identity(void **state)
{
	(void)state;
	struct obs_output output = {0};
	struct obs_output other = {0};
	struct obs_weak_output weak = {.output = &output};
	output.context.control = (struct obs_weak_object *)&weak;
	const long counts[] = {0, 1, 7, 1023};
	for (size_t i = 0; i < OBS_COUNTOF(counts); ++i) {
		weak.ref.refs = counts[i];
		weak.ref.weak_refs = 3;
		assert_ptr_equal(obs_weak_output_get_output(&weak), &output);
		assert_int_equal(weak.ref.refs, counts[i] + 1);
		assert_int_equal(weak.ref.weak_refs, 3);
		obs_output_release(&output);
		assert_int_equal(weak.ref.refs, counts[i]);
		assert_true(obs_weak_output_references_output(&weak, &output));
		assert_false(obs_weak_output_references_output(&weak, &other));
		assert_false(obs_weak_output_references_output(&weak, NULL));
		assert_false(obs_weak_output_references_output(NULL, &output));
	}
	for (long refs = -1; refs >= -3; --refs) {
		weak.ref.refs = refs;
		assert_null(obs_weak_output_get_output(&weak));
		assert_int_equal(weak.ref.refs, refs);
		assert_int_equal(weak.ref.weak_refs, 3);
		/* Identity does not imply that promotion can succeed. The object is
		 * still allocated here, so this never compares a freed C pointer. */
		assert_true(obs_weak_output_references_output(&weak, &output));
		assert_false(obs_weak_output_references_output(&weak, &other));
	}
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

static void real_output_lifetime(void **state)
{
	(void)state;
	/* An unregistered output has a real context/control and destroy path,
	 * without plugin state, encoders, or an active media pipeline. */
	obs_output_t *output = obs_output_create("weak-lifetime-missing", "weak lifetime", NULL, NULL);
	assert_non_null(output);
	obs_weak_output_t *weak = obs_output_get_weak_output(output);
	obs_weak_output_t *second = obs_output_get_weak_output(output);
	assert_ptr_equal(second, weak);
	assert_int_equal(weak->ref.weak_refs, 2);
	obs_output_t *promoted = obs_weak_output_get_output(weak);
	assert_ptr_equal(promoted, output);
	assert_int_equal(weak->ref.refs, 1);
	obs_output_release(output);
	assert_int_equal(weak->ref.refs, 0);
	assert_true(obs_weak_output_references_output(weak, promoted));
	obs_output_t *again = obs_weak_output_get_output(weak);
	assert_ptr_equal(again, promoted);
	obs_output_release(again);
	obs_output_release(promoted);
	assert_int_equal(weak->ref.refs, -1);
	assert_int_equal(weak->ref.weak_refs, 1);
	assert_null(obs_weak_output_get_output(weak));
	obs_weak_output_release(second);
	assert_int_equal(weak->ref.weak_refs, 0);
	assert_null(obs_weak_output_get_output(weak));
	const long allocations = bnum_allocs();
	obs_weak_output_release(weak);
	assert_int_equal(bnum_allocs(), allocations - 1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_inputs),
		cmocka_unit_test(weak_ownership),
		cmocka_unit_test(promotion_and_identity),
		cmocka_unit_test_setup_teardown(real_output_lifetime, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
