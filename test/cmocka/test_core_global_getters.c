/* Actual-library characterization: borrowed global core pointers.
 * An initialized core is required by these C getters. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>

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

static void locale_borrow(void **state)
{
	(void)state;
	const char *locale = obs_get_locale();
	assert_string_equal(locale, "en-US");
	assert_ptr_equal(locale, obs_get_locale());
	obs_set_locale("fr-FR");
	locale = obs_get_locale();
	assert_string_equal(locale, "fr-FR");
	assert_ptr_equal(locale, obs_get_locale());
}

static void signal_borrow(void **state)
{
	(void)state;
	signal_handler_t *handler = obs_get_signal_handler();
	assert_non_null(handler);
	assert_ptr_equal(handler, obs_get_signal_handler());
}

static void proc_borrow(void **state)
{
	(void)state;
	proc_handler_t *handler = obs_get_proc_handler();
	assert_non_null(handler);
	assert_ptr_equal(handler, obs_get_proc_handler());
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(locale_borrow, setup, teardown),
					   cmocka_unit_test_setup_teardown(signal_borrow, setup, teardown),
					   cmocka_unit_test_setup_teardown(proc_borrow, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
