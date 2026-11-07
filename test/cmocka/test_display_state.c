/* Characterize the display state accessors before their Rust port (#3848).
 * No graphics context: these functions touch only scalar fields in a caller-
 * owned display. Compare the whole object to pin unrelated-field preservation.
 * NULL setters being silent no-ops is characterized, not endorsed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include "obs-internal.h"
#include <string.h>

static void test_null(void **state)
{
	(void)state;
	assert_false(obs_display_enabled(NULL));
	obs_display_set_enabled(NULL, true);
	obs_display_set_enabled(NULL, false);
	obs_display_set_background_color(NULL, UINT32_MAX);
}

static void test_enabled(void **state)
{
	(void)state;
	struct obs_display display = {0};
	display.update_color_space = true;
	display.cx = 1920;
	display.cy = 1080;
	display.background_color = UINT32_C(0x89abcdef);
	struct obs_display expected;
	memcpy(&expected, &display, sizeof(display));
	const bool values[] = {false, true, true, false, false, true};
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		expected.enabled = values[i];
		obs_display_set_enabled(&display, values[i]);
		assert_int_equal(obs_display_enabled(&display), values[i]);
		assert_memory_equal(&display, &expected, sizeof(display));
	}
}

static void test_color(void **state)
{
	(void)state;
	struct obs_display display = {0};
	display.enabled = true;
	display.next_cx = 1280;
	display.next_cy = 720;
	display.use_clear_workaround = true;
	struct obs_display expected;
	memcpy(&expected, &display, sizeof(display));
	const uint32_t colors[] = {0, UINT32_MAX, UINT32_C(0x80000000), UINT32_C(0x01020304),
				   UINT32_C(0xff000000), UINT32_C(0x00ffffff)};
	for (size_t i = 0; i < sizeof(colors) / sizeof(colors[0]); i++) {
		expected.background_color = colors[i];
		obs_display_set_background_color(&display, colors[i]);
		assert_int_equal(display.background_color, colors[i]);
		assert_true(obs_display_enabled(&display));
		assert_memory_equal(&display, &expected, sizeof(display));
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_null),
		cmocka_unit_test(test_enabled),
		cmocka_unit_test(test_color),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
