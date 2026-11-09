/* Characterization of the actual libobs generic weak-object reference API. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>
#include "obs-internal.h"

static void null_handles(void **state)
{
	(void)state;
	obs_weak_object_addref(NULL);
	obs_weak_object_release(NULL);
	assert_null(obs_object_get_weak_object(NULL));
	assert_null(obs_weak_object_get_object(NULL));
}

static void weak_ownership(void **state)
{
	(void)state;
	struct obs_context_data object = {0};
	struct obs_weak_object weak = {.ref = {.refs = 4, .weak_refs = 0}, .object = &object};
	object.control = &weak;
	obs_weak_object_addref(&weak);
	assert_int_equal(weak.ref.weak_refs, 1);
	assert_int_equal(weak.ref.refs, 4);
	assert_ptr_equal(weak.object, &object);
	assert_ptr_equal(obs_object_get_weak_object(&object), &weak);
	assert_int_equal(weak.ref.weak_refs, 2);
	assert_int_equal(weak.ref.refs, 4);
	obs_weak_object_release(&weak);
	assert_int_equal(weak.ref.weak_refs, 1);
	obs_weak_object_release(&weak);
	assert_int_equal(weak.ref.weak_refs, 0);
	assert_int_equal(weak.ref.refs, 4);
	assert_ptr_equal(weak.object, &object);

	/* Characterized, not endorsed: acquiring a weak handle does not check expiry. */
	weak.ref.refs = -1;
	assert_ptr_equal(obs_object_get_weak_object(&object), &weak);
	assert_int_equal(weak.ref.refs, -1);
	assert_int_equal(weak.ref.weak_refs, 1);
	obs_weak_object_release(&weak);
}

static void promotion(void **state)
{
	(void)state;
	struct obs_context_data object = {0};
	struct obs_weak_object weak = {.ref = {.weak_refs = 3}, .object = &object};
	const long counts[] = {-2, -1, 0, 1, 7};
	for (size_t i = 0; i < sizeof(counts) / sizeof(counts[0]); i++) {
		weak.ref.refs = counts[i];
		if (counts[i] < 0) {
			assert_null(obs_weak_object_get_object(&weak));
			assert_int_equal(weak.ref.refs, counts[i]);
		} else {
			assert_ptr_equal(obs_weak_object_get_object(&weak), &object);
			assert_int_equal(weak.ref.refs, counts[i] + 1);
		}
		assert_int_equal(weak.ref.weak_refs, 3);
		assert_ptr_equal(weak.object, &object);
	}
}

static void final_release(void **state)
{
	(void)state;
	const long before = bnum_allocs();
	struct obs_weak_object *weak = bzalloc(sizeof(*weak));
	weak->ref.refs = -1;
	assert_int_equal(bnum_allocs(), before + 1);
	obs_weak_object_addref(weak);
	assert_int_equal(weak->ref.weak_refs, 1);
	obs_weak_object_release(weak);
	assert_int_equal(weak->ref.weak_refs, 0);
	assert_int_equal(bnum_allocs(), before + 1);
	obs_weak_object_release(weak);
	assert_int_equal(bnum_allocs(), before);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_handles), cmocka_unit_test(weak_ownership),
					   cmocka_unit_test(promotion), cmocka_unit_test(final_release)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
