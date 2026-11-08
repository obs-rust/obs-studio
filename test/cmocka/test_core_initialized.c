/* Characterize the public initialization query against the actual libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>

static void initialized_lifecycle(void **state)
{
	(void)state;
	assert_false(obs_initialized());
	assert_false(obs_initialized());
	assert_true(obs_startup("en-US", NULL, NULL));
	assert_true(obs_initialized());
	assert_true(obs_initialized());
	obs_shutdown();
	assert_false(obs_initialized());
	assert_false(obs_initialized());
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(initialized_lifecycle)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
