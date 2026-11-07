/* Actual-library characterization: locale getters dispatch plugin callbacks. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs-internal.h>
#include <obs-module.h>
#include <util/text-lookup.h>

static lookup_t *lookup;
static unsigned calls;
static const char changed[] = "changed";
static bool module_lookup(const char *key, const char **out)
{
	++calls;
	return text_lookup_getstr(lookup, key, out);
}
static bool false_but_writes(const char *key, const char **out)
{
	(void)key;
	++calls;
	if (out)
		*out = changed;
	return false;
}
static void locale_lookup(void **state)
{
	(void)state;
	struct obs_module module = {0};
	const char *key = "Key";
	const char *missing = "Missing";
	const char *empty = "Empty";
	const char *sentinel = "sentinel";
	const char *out = sentinel;
	assert_false(obs_module_get_locale_string(&module, key, &out));
	assert_ptr_equal(out, sentinel);
	assert_ptr_equal(obs_module_get_locale_text(&module, key), key);
	assert_null(obs_module_get_locale_text(&module, NULL));
	module.get_string = module_lookup;
	lookup = NULL;
	calls = 0;
	assert_false(obs_module_get_locale_string(&module, key, &out));
	assert_ptr_equal(out, sentinel);
	assert_ptr_equal(obs_module_get_locale_text(&module, key), key);
	assert_int_equal(calls, 2);
	assert_false(obs_module_get_locale_string(&module, NULL, &out));
	assert_ptr_equal(out, sentinel);
	assert_null(obs_module_get_locale_text(&module, NULL));
	FILE *file = fopen("test_module_locale_getters.ini", "wb");
	assert_non_null(file);
	assert_true(fputs("Key=Value\nEmpty=\"\"\n", file) >= 0);
	assert_int_equal(fclose(file), 0);
	lookup = text_lookup_create("test_module_locale_getters.ini");
	assert_int_equal(remove("test_module_locale_getters.ini"), 0);
	assert_non_null(lookup);
	assert_false(obs_module_get_locale_string(&module, missing, &out));
	assert_ptr_equal(out, sentinel);
	assert_ptr_equal(obs_module_get_locale_text(&module, missing), missing);
	assert_true(obs_module_get_locale_string(&module, key, &out));
	assert_string_equal(out, "Value");
	assert_ptr_equal(obs_module_get_locale_text(&module, key), out);
	/* Empty translations are ignored by text_lookup, leaving the input key. */
	assert_ptr_equal(obs_module_get_locale_text(&module, empty), empty);
	text_lookup_destroy(lookup);
	lookup = NULL;
	module.get_string = false_but_writes;
	assert_false(obs_module_get_locale_string(&module, NULL, NULL));
	out = sentinel;
	assert_false(obs_module_get_locale_string(&module, key, &out));
	assert_ptr_equal(out, changed);
	/* Characterized, not interpreted: text returns the written value even on false. */
	assert_ptr_equal(obs_module_get_locale_text(&module, key), changed);
#ifdef NDEBUG
	/* Debug C asserts on NULL modules; release C returns false without writing. */
	out = sentinel;
	assert_false(obs_module_get_locale_string(NULL, key, &out));
	assert_ptr_equal(out, sentinel);
	assert_ptr_equal(obs_module_get_locale_text(NULL, key), key);
#endif
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(locale_lookup)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
