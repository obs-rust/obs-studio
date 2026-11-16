/* Characterize the actual library, without compiling a getter into this test. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs.h"
#include "obs-hotkey.h"

static void background_error_lifecycle(void **state)
{
	(void)state;
	assert_null(obs_hotkeys_get_background_error());
	assert_true(obs_startup("en-US", NULL, NULL));
	assert_null(obs_hotkeys_get_background_error());
	obs_shutdown();
	assert_null(obs_hotkeys_get_background_error());
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(background_error_lifecycle)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
