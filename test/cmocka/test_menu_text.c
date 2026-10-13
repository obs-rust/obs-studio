#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

#include "menu-text.h"

/* The Profile and Scene Collection menus list the names the user typed. Qt
 * reads '&' in a menu entry's text as a mnemonic marker: it hides the '&' and
 * underlines the next character, and only "&&" shows as one '&'. A scene
 * collection named "MAME Pac & Pal" showed as "MAME Pac _Pal" (the space
 * underlined), while the title bar showed the full name.
 *
 * shown() follows Qt's rule for the text a menu draws (QAction::iconText(),
 * qt_strippedText): a single '&' is dropped, "&&" becomes '&'. */

static void shown(const char *text, char *out, size_t out_size)
{
	size_t n = 0;
	for (const char *c = text; *c && n + 1 < out_size; c++) {
		if (*c == '&') {
			if (c[1] != '&')
				continue;
			c++;
		}
		out[n++] = *c;
	}
	out[n] = '\0';
}

static void assert_menu_shows_name(const char *name)
{
	char text[256];
	char displayed[256];
	size_t len = menu_text_from_name(text, sizeof(text), name);

	assert_int_equal(len, strlen(text));
	shown(text, displayed, sizeof(displayed));
	assert_string_equal(displayed, name);
}

static void name_with_ampersand_shows_as_typed(void **state)
{
	(void)state;
	assert_menu_shows_name("MAME Pac & Pal");
	assert_menu_shows_name("Q&A");
}

static void name_with_ampersands_at_the_ends_shows_as_typed(void **state)
{
	(void)state;
	assert_menu_shows_name("&Start");
	assert_menu_shows_name("Rock&");
	assert_menu_shows_name("a && b");
	assert_menu_shows_name("&&&");
}

static void name_without_ampersand_is_unchanged(void **state)
{
	(void)state;
	char text[64];

	assert_int_equal(menu_text_from_name(text, sizeof(text), "Untitled"), 8);
	assert_string_equal(text, "Untitled");
	assert_int_equal(menu_text_from_name(text, sizeof(text), ""), 0);
	assert_string_equal(text, "");
}

/* The returned length is what the whole text needs; a short buffer gets a
 * NUL-terminated prefix. */
static void length_and_short_buffer(void **state)
{
	(void)state;
	char text[4];

	size_t needed = menu_text_from_name(NULL, 0, "A & B");
	assert_int_equal(menu_text_from_name(text, sizeof(text), "A & B"), needed);
	assert_int_equal(strlen(text), sizeof(text) - 1);

	char full[16];
	assert_int_equal(menu_text_from_name(full, sizeof(full), "A & B"), needed);
	assert_int_equal(strlen(full), needed);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(name_with_ampersand_shows_as_typed),
		cmocka_unit_test(name_with_ampersands_at_the_ends_shows_as_typed),
		cmocka_unit_test(name_without_ampersand_is_unchanged),
		cmocka_unit_test(length_and_short_buffer),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
