/* Characterize the public getter through the actual libobs lifecycle. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>

static void frame_interval_values(void **state)
{
	(void)state;
	for (size_t i = 0; i < 2; ++i) {
		assert_true(obs_startup("en-US", NULL, NULL));
		/* No video clock has been configured in a newly initialized core. */
		assert_int_equal(obs_get_frame_interval_ns(), 0);
		assert_int_equal(obs_get_frame_interval_ns(), 0);
		obs_shutdown();
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(frame_interval_values)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
