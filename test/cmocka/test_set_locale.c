/* Actual-library characterization before the Rust swap (Refs #5041).
 * No startup, plugin loading, graphics or threads are needed. The fixture restores
 * the library's hidden core even after a failed assertion. A NULL core and
 * an input aliasing the old owned locale are outside the C function's domain. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "core_set_locale_fixture.h"

static struct obs_core core;
static struct obs_core *previous_core;
static struct obs_module modules[7];
static long baseline;
static const char *expected_argument;
static const char *expected_text;
static unsigned visits[12];
static size_t visit_count;

static int setup(void **state)
{
	(void)state;
	memset(&core, 0, sizeof(core));
	memset(modules, 0, sizeof(modules));
	memset(visits, 0, sizeof(visits));
	visit_count = 0;
	expected_argument = NULL;
	expected_text = NULL;
	baseline = bnum_allocs();
	previous_core = obs_test_set_locale_swap_core(&core);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	/* obs_shutdown frees the owned locale; synthetic records remain fixture-owned. */
	bfree(core.locale);
	core.locale = NULL;
	obs_test_set_locale_swap_core(previous_core);
	assert_int_equal(bnum_allocs(), baseline);
	return 0;
}

static void record(unsigned id, const char *locale)
{
	/* Callbacks receive the caller's pointer, after owned storage is replaced. */
	assert_ptr_equal(locale, expected_argument);
	if (expected_text) {
		assert_non_null(core.locale);
		assert_ptr_not_equal(core.locale, locale);
		assert_string_equal(core.locale, expected_text);
	} else {
		assert_null(core.locale);
	}
	assert_int_equal(bnum_allocs(), baseline + (expected_text ? 1 : 0));
	assert_true(visit_count < sizeof(visits) / sizeof(visits[0]));
	visits[visit_count++] = id;
}

static void first(const char *locale)
{
	record(1, locale);
}

static void second(const char *locale)
{
	assert_true(visit_count > 0);
	assert_int_equal(visits[visit_count - 1], 1);
	record(2, locale);
}

static void third(const char *locale)
{
	record(3, locale);
}

static void excluded(const char *locale)
{
	(void)locale;
	fail();
}

static void replacement_owns_one_copy(void **state)
{
	(void)state;
	char initial[] = "en-US";
	obs_set_locale(initial);
	assert_string_equal(core.locale, "en-US");
	assert_ptr_not_equal(core.locale, initial);
	assert_int_equal(bnum_allocs(), baseline + 1);
	initial[0] = 'x';
	assert_string_equal(core.locale, "en-US");

	char replacement[] = "a-much-longer-replacement-locale";
	obs_set_locale(replacement);
	assert_string_equal(core.locale, replacement);
	assert_ptr_not_equal(core.locale, replacement);
	/* One owned allocation remains: replacement freed the previous string. */
	assert_int_equal(bnum_allocs(), baseline + 1);
	replacement[0] = 'x';
	assert_string_equal(core.locale, "a-much-longer-replacement-locale");

	char empty[] = "";
	obs_set_locale(empty);
	assert_non_null(core.locale);
	assert_ptr_not_equal(core.locale, empty);
	assert_string_equal(core.locale, "");
	assert_int_equal(bnum_allocs(), baseline + 1);
	/* Characterized, not endorsed: NULL clears storage through bstrdup(NULL). */
	obs_set_locale(NULL);
	assert_null(core.locale);
	assert_int_equal(bnum_allocs(), baseline);
	obs_set_locale(NULL);
	assert_null(core.locale);
	assert_int_equal(bnum_allocs(), baseline);
}

static void ordered_callbacks_skip_missing(void **state)
{
	(void)state;
	/* Non-array traversal, with absent callbacks at the head, middle and tail. */
	core.first_module = &modules[5];
	modules[5].next = &modules[2];
	modules[2].set_locale = third;
	modules[2].next = &modules[3];
	modules[3].next = &modules[0];
	modules[0].set_locale = first;
	modules[0].next = &modules[1];
	modules[1].set_locale = second;
	modules[1].next = &modules[4];
	/* Characterized: list membership alone controls dispatch, not loaded state. */
	modules[0].loaded = false;
	modules[0].load_state = OBS_MODULE_DISABLED;
	core.first_disabled_module = &modules[6];
	modules[6].set_locale = excluded;
	struct obs_module before[7];
	memcpy(before, modules, sizeof(before));

	char locale[] = "fr-FR";
	expected_argument = locale;
	expected_text = "fr-FR";
	for (size_t i = 0; i < 2; ++i) {
		obs_set_locale(locale);
		assert_int_equal(visit_count, (i + 1) * 3);
		assert_int_equal(visits[i * 3], 3);
		assert_int_equal(visits[i * 3 + 1], 1);
		assert_int_equal(visits[i * 3 + 2], 2);
	}
	expected_argument = NULL;
	expected_text = NULL;
	obs_set_locale(NULL);
	assert_int_equal(visit_count, 9);
	assert_int_equal(visits[6], 3);
	assert_int_equal(visits[7], 1);
	assert_int_equal(visits[8], 2);
	assert_memory_equal(modules, before, sizeof(before));
	assert_ptr_equal(core.first_module, &modules[5]);
	assert_ptr_equal(core.first_disabled_module, &modules[6]);
}

static void all_callbacks_missing(void **state)
{
	(void)state;
	core.first_module = &modules[0];
	modules[0].next = &modules[1];
	modules[1].next = &modules[2];
	obs_set_locale("de-DE");
	assert_string_equal(core.locale, "de-DE");
	assert_int_equal(visit_count, 0);
	assert_int_equal(bnum_allocs(), baseline + 1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(replacement_owns_one_copy, setup, teardown),
		cmocka_unit_test_setup_teardown(ordered_callbacks_skip_missing, setup, teardown),
		cmocka_unit_test_setup_teardown(all_callbacks_missing, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
