#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>

#include "vmic-install.h"

static void first_start_installs_the_bundled_driver(void **state)
{
	(void)state;
	assert_int_equal(vmic_driver_action(NULL, NULL, "32.0.0", "1"), VMIC_DRIVER_INSTALL);
}

static void same_version_and_build_is_ok(void **state)
{
	(void)state;
	assert_int_equal(vmic_driver_action("32.0.0", "1", "32.0.0", "1"), VMIC_DRIVER_OK);
}

static void another_version_or_build_is_updated(void **state)
{
	(void)state;
	assert_int_equal(vmic_driver_action("31.1.0", "1", "32.0.0", "1"), VMIC_DRIVER_UPDATE);
	assert_int_equal(vmic_driver_action("32.0.0", "1", "32.0.0", "2"), VMIC_DRIVER_UPDATE);
	assert_int_equal(vmic_driver_action("32.0.0", NULL, "32.0.0", "2"), VMIC_DRIVER_UPDATE);
}

static void without_a_bundled_copy_the_installed_one_stays(void **state)
{
	(void)state;
	assert_int_equal(vmic_driver_action("31.1.0", "1", NULL, NULL), VMIC_DRIVER_OK);
	assert_int_equal(vmic_driver_action(NULL, NULL, NULL, NULL), VMIC_DRIVER_UNAVAILABLE);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(first_start_installs_the_bundled_driver),
		cmocka_unit_test(same_version_and_build_is_ok),
		cmocka_unit_test(another_version_or_build_is_updated),
		cmocka_unit_test(without_a_bundled_copy_the_installed_one_stays),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
