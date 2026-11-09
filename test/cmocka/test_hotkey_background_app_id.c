/* Characterize the actual library's synchronized borrowed app-ID query. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs.h"
#include "obs-hotkey.h"

static void background_app_id(void **state)
{
	(void)state;
	assert_null(obs_hotkeys_get_background_app_id());
	assert_true(obs_startup("en-US", NULL, NULL));
	const char *app_id = obs_hotkeys_get_background_app_id();
	assert_ptr_equal(obs_hotkeys_get_background_app_id(), app_id);
	if (app_id)
		assert_string_equal(obs_hotkeys_get_background_app_id(), app_id);
	obs_shutdown();
	assert_null(obs_hotkeys_get_background_app_id());
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(background_app_id)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
