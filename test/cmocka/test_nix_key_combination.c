/* Public Linux/FreeBSD formatter characterization before its Rust port.
 * Requires the X11 backend (xvfb-run); no video or audio reset is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>

#include <obs.h>
#include <util/dstr.h>

static int setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static void names(const char *control, const char *meta, const char *alt, const char *shift, const char *insert)
{
	struct obs_hotkeys_translations translations = {0};
	translations.control = control;
	translations.meta = meta;
	translations.alt = alt;
	translations.shift = shift;
	translations.insert = insert;
	obs_hotkeys_set_translations_s(&translations, sizeof(translations));
}

static void expect(uint32_t modifiers, obs_key_t key, const char *prefix, const char *expected)
{
	struct dstr text = {0};
	dstr_copy(&text, prefix);
	obs_key_combination_t combination = {modifiers, key};
	obs_key_combination_to_str(combination, &text);
	assert_string_equal(text.array ? text.array : "", expected);
	assert_int_equal(text.len, strlen(expected));
	dstr_free(&text);
}

static void every_modifier_subset(void **state)
{
	(void)state;
	names("C", "M", "A", "S", "K");
	const uint32_t c = INTERACT_CONTROL_KEY, m = INTERACT_COMMAND_KEY;
	const uint32_t a = INTERACT_ALT_KEY, s = INTERACT_SHIFT_KEY;
	expect(0, OBS_KEY_INSERT, "", "K");
	expect(c, OBS_KEY_INSERT, "", "C + K");
	expect(m, OBS_KEY_INSERT, "", "M + K");
	expect(a, OBS_KEY_INSERT, "", "A + K");
	expect(s, OBS_KEY_INSERT, "", "S + K");
	expect(c | m, OBS_KEY_INSERT, "", "C + M + K");
	expect(c | a, OBS_KEY_INSERT, "", "C + A + K");
	expect(c | s, OBS_KEY_INSERT, "", "C + S + K");
	expect(m | a, OBS_KEY_INSERT, "", "M + A + K");
	expect(m | s, OBS_KEY_INSERT, "", "M + S + K");
	expect(a | s, OBS_KEY_INSERT, "", "A + S + K");
	expect(c | m | a, OBS_KEY_INSERT, "", "C + M + A + K");
	expect(c | m | s, OBS_KEY_INSERT, "", "C + M + S + K");
	expect(c | a | s, OBS_KEY_INSERT, "", "C + A + S + K");
	expect(m | a | s, OBS_KEY_INSERT, "", "M + A + S + K");
	expect(c | m | a | s, OBS_KEY_INSERT, "", "C + M + A + S + K");
}

static void appends_and_keeps_duplicate_modifiers(void **state)
{
	(void)state;
	names("Ctl", "Meta", "Alt", "Shift", "Ins");
	expect(0, OBS_KEY_NONE, "existing", "existing");
	expect(0, OBS_KEY_NONE, "", "");
	expect(0, OBS_KEY_INSERT, "existing", "existing + Ins");
	expect(INTERACT_CONTROL_KEY, OBS_KEY_NONE, "existing + ", "existing +  + Ctl");
	/* Characterized, not endorsed: modifier keys are appended twice. */
	expect(INTERACT_CONTROL_KEY, OBS_KEY_CONTROL, "", "Ctl + Ctl");
	expect(INTERACT_CONTROL_KEY | INTERACT_COMMAND_KEY | INTERACT_ALT_KEY | INTERACT_SHIFT_KEY, OBS_KEY_NONE,
	       "existing", "existing + Ctl + Meta + Alt + Shift");
	const uint32_t known = INTERACT_CONTROL_KEY | INTERACT_COMMAND_KEY | INTERACT_ALT_KEY | INTERACT_SHIFT_KEY;
	expect(UINT32_MAX & ~known, OBS_KEY_INSERT, "", "Ins");
	expect(UINT32_MAX, OBS_KEY_INSERT, "", "Ctl + Meta + Alt + Shift + Ins");
}

static void empty_names_do_not_add_separators(void **state)
{
	(void)state;
	const uint32_t all = INTERACT_CONTROL_KEY | INTERACT_COMMAND_KEY | INTERACT_ALT_KEY | INTERACT_SHIFT_KEY;
	names("", "Meta", "", "Shift", "");
	expect(all, OBS_KEY_INSERT, "", "Meta + Shift");
	expect(all, OBS_KEY_INSERT, "prefix", "prefix + Meta + Shift");
	names("", "", "", "", "");
	expect(all, OBS_KEY_INSERT, "prefix", "prefix");
	expect(all, OBS_KEY_INSERT, "", "");
	/* Whitespace and UTF-8 names are opaque bytes, not trimmed or normalized. */
	names(" ", "\xc3\xa9", "", "", "K");
	expect(all, OBS_KEY_INSERT, "", "  + \xc3\xa9 + K");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(every_modifier_subset),
		cmocka_unit_test(appends_and_keeps_duplicate_modifiers),
		cmocka_unit_test(empty_names_do_not_add_separators),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
