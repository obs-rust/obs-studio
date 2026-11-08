/* Characterize the public source-type queries against actual libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>

static void source_type_queries(void **state)
{
	(void)state;
	assert_false(obs_source_type_is_group(NULL));
	assert_false(obs_source_type_is_scene(NULL));
	const char *ids[] = {"",       "group",  "scene",  "Group",  "Scene", "grou",           "scen",
			     "groups", "scenes", " scene", "group ", "\xff",  "group\0ignored", "scene\0ignored"};
	for (size_t i = 0; i < sizeof(ids) / sizeof(ids[0]); i++) {
		assert_int_equal(obs_source_type_is_group(ids[i]), i == 1 || i == 12);
		assert_int_equal(obs_source_type_is_scene(ids[i]), i == 2 || i == 13);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(source_type_queries)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
