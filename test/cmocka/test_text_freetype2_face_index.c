#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>

#include <obs-module.h>

#include "find-font.h"

/* The Text (FreeType 2) source on Linux asked fontconfig for the matched
 * font's FC_INDEX with value position 1 instead of 0, so it always got the
 * default 0 and loaded the first face of the file. A font that is not face 0
 * of its file was never used: the faces of a .ttc/.otc collection (Noto CJK
 * SC/TC/KR...) and the named instances of a variable font (Bold of Adwaita
 * Sans is index 458752) all rendered as face 0 (obsproject/obs-studio#11819,
 * PR #13290 by reyohoho).
 *
 * main() points fontconfig at COLLECTION_FONTCONFIG, whose only font
 * directory holds text-collection.ttc: "OBS Collection Fixture" Regular as
 * face 0 and Bold as face 1 (test/cmocka/data/text-collection.py). */

static void assert_face(const char *style, uint32_t flags, FT_Long expected)
{
	FT_Long idx = -1;
	const char *path = get_font_path("OBS Collection Fixture", 32, style, flags, &idx);
	assert_non_null(path);
	assert_non_null(strstr(path, "text-collection.ttc"));
	if (idx != expected)
		fail_msg("%s resolved to face %ld of %s (expected %ld)", style, (long)idx, path, (long)expected);
}

static void regular_resolves_to_face_0(void **state)
{
	(void)state;
	assert_face("Regular", 0, 0);
}

static void bold_resolves_to_face_1(void **state)
{
	(void)state;
	assert_face("Bold", OBS_FONT_BOLD, 1);
}

int main(void)
{
	/* Before the first fontconfig call, which loads the configuration. */
	setenv("FONTCONFIG_FILE", COLLECTION_FONTCONFIG, 1);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(regular_resolves_to_face_0),
		cmocka_unit_test(bold_resolves_to_face_1),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
