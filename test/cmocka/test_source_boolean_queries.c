/* Actual-library characterization; NULL is outside these getters' C contract. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

/* Exercise the deprecated public hidden-state API as part of ABI parity. */
PRAGMA_WARN_PUSH
PRAGMA_DISABLE_DEPRECATION
static void independent_fields(void **state)
{
	(void)state;
	struct obs_source source = {0};
	unsigned char before[sizeof(source)];
	for (unsigned centered = 0; centered < 2; centered++) {
		for (unsigned hidden = 0; hidden < 2; hidden++) {
			source.texcoords_centered = centered != 0;
			obs_source_set_hidden(&source, hidden != 0);
			source.enabled = true;
			source.flags = 0xa5a55a5a;
			memcpy(before, &source, sizeof(source));
			assert_int_equal(obs_source_get_texcoords_centered(&source), centered);
			assert_memory_equal(before, &source, sizeof(source));
			assert_int_equal(obs_source_is_hidden(&source), hidden);
			assert_memory_equal(before, &source, sizeof(source));
			assert_int_equal(obs_source_get_texcoords_centered(&source), centered);
			assert_int_equal(obs_source_is_hidden(&source), hidden);
			assert_memory_equal(before, &source, sizeof(source));
		}
	}
}

PRAGMA_WARN_POP

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(independent_fields)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
