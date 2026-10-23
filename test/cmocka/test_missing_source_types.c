#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include "missing-source-types.h"

/* obs-rust/obs-studio#1865: a scene collection moved from upstream OBS to the
 * AppImage has browser sources, but the AppImage has no obs-browser. libobs
 * keeps those sources and renders nothing, so OBS tells the user which
 * source types are missing and why. */

static void test_lists_each_missing_type_once(void **state)
{
	(void)state;
	struct missing_source_types list = {0};

	missing_source_types_add(&list, "browser_source", false);
	missing_source_types_add(&list, "ffmpeg_source", true);
	missing_source_types_add(&list, "browser_source", false);
	missing_source_types_add(&list, "vlc_source", false);
	missing_source_types_add(&list, "scene", true);
	missing_source_types_add(&list, "browser_source", false);

	assert_int_equal(list.count, 2);
	assert_string_equal(list.ids[0], "browser_source");
	assert_int_equal(list.sources[0], 3);
	assert_string_equal(list.ids[1], "vlc_source");
	assert_int_equal(list.sources[1], 1);
}

static void test_nothing_missing(void **state)
{
	(void)state;
	struct missing_source_types list = {0};

	missing_source_types_add(&list, "ffmpeg_source", true);
	missing_source_types_add(&list, NULL, false);
	missing_source_types_add(&list, "", false);
	assert_int_equal(list.count, 0);
}

/* More missing types than the list holds: the first ones stay listed, the
 * rest are dropped, nothing overflows. An over-long id is cut, not spilled. */
static void test_list_is_bounded(void **state)
{
	(void)state;
	struct missing_source_types list = {0};
	char id[32];
	char long_id[200];

	for (int i = 0; i < MISSING_SOURCE_TYPES_MAX + 4; i++) {
		snprintf(id, sizeof(id), "plugin_source_%d", i);
		missing_source_types_add(&list, id, false);
	}
	assert_int_equal(list.count, MISSING_SOURCE_TYPES_MAX);
	assert_string_equal(list.ids[MISSING_SOURCE_TYPES_MAX - 1], "plugin_source_15");

	struct missing_source_types one = {0};
	memset(long_id, 'x', sizeof(long_id) - 1);
	long_id[sizeof(long_id) - 1] = '\0';
	missing_source_types_add(&one, long_id, false);
	assert_int_equal(one.count, 1);
	assert_int_equal(strlen(one.ids[0]), MISSING_SOURCE_TYPE_ID_SIZE - 1);
}

static void test_hints(void **state)
{
	(void)state;
	assert_int_equal(missing_source_types_hint("browser_source"), MISSING_SOURCE_HINT_BROWSER);
	assert_int_equal(missing_source_types_hint("vlc_source"), MISSING_SOURCE_HINT_VLC);
	assert_int_equal(missing_source_types_hint("decklink-input"), MISSING_SOURCE_HINT_PLUGIN);
	assert_int_equal(missing_source_types_hint("browser_source_v2"), MISSING_SOURCE_HINT_PLUGIN);
	assert_int_equal(missing_source_types_hint(NULL), MISSING_SOURCE_HINT_PLUGIN);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_lists_each_missing_type_once),
		cmocka_unit_test(test_nothing_missing),
		cmocka_unit_test(test_list_is_bounded),
		cmocka_unit_test(test_hints),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
