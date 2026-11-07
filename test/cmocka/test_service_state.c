/* C-only characterization of hidden #3840 accessors. The generated include
 * extracts their unchanged bodies from obs-service.c.h before the Rust port. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"
#define get_weak(service) ((obs_weak_service_t *)service->context.control)
#include "service-state-characterization.inc"
#undef get_weak

static void test_state(void **state)
{
	(void)state;
	struct obs_service s = {0};
	struct obs_output o = {0};
	struct obs_service other = {0};
	struct obs_weak_service control = {0};
	assert_false(obs_service_is_destroying(&s));
	assert_false(obs_service_is_active(&s));
	assert_false(obs_service_has_output(&s));
	assert_null(obs_service_get_control(&s));
	s.context.control = (obs_weak_object_t *)&control;
	s.output = &o;
	o.service = &other; /* Detach clears even a different reverse association. */
	obs_service_set_active(&s, true);
	obs_service_set_destroy(&s);
	obs_service_set_destroy(&s);
	assert_true(obs_service_is_destroying(&s));
	assert_true(obs_service_is_active(&s));
	assert_true(obs_service_has_output(&s));
	assert_ptr_equal(obs_service_get_control(&s), &control);
	obs_service_detach_from_output(&s);
	assert_null(o.service);
	assert_ptr_equal(s.output, &o); /* characterized, not endorsed: one-sided */
	obs_service_clear_output(&s);
	obs_service_clear_output(&s);
	assert_false(obs_service_has_output(&s));
	obs_service_set_active(&s, false);
	assert_false(obs_service_is_active(&s));
	assert_true(obs_service_is_destroying(&s));
	assert_ptr_equal(obs_service_get_control(&s), &control);
	assert_int_equal(control.ref.refs, 0);
	assert_int_equal(control.ref.weak_refs, 0);
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(test_state)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
