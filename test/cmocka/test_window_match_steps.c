#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "util/windows/window-match.h"

/* Characterization of the window helpers' pure decisions, moved from
 * libobs/util/windows/window-helpers.c to libobs/util/windows/window-match.c:
 * which windows the window and game capture lists hide, the generic and UWP
 * class tests, and the rating ms_find_window() and
 * ms_find_window_top_level() pick a window by. The file includes no Windows
 * header, so this runs on every platform; the window helpers themselves only
 * run on Windows. The functions are hidden in libobs, so this characterizes
 * that C source; the test calls only them. */

#define NONE MS_WINDOW_MATCH_NONE
#define PENALTY MS_WINDOW_MATCH_EXE_PENALTY
#define P_CLASS MS_WINDOW_MATCH_PRIORITY_CLASS
#define P_TITLE MS_WINDOW_MATCH_PRIORITY_TITLE
#define P_EXE MS_WINDOW_MATCH_PRIORITY_EXE

static const char *internal_names[] = {
	"startmenuexperiencehost.exe",
	"applicationframehost.exe",
	"peopleexperiencehost.exe",
	"shellexperiencehost.exe",
	"microsoft.notes.exe",
	"systemsettings.exe",
	"textinputhost.exe",
	"searchapp.exe",
	"video.ui.exe",
	"searchui.exe",
	"lockapp.exe",
	"cortana.exe",
	"gamebar.exe",
	"tabtip.exe",
	"time.exe",
};

#define N_INTERNAL (sizeof(internal_names) / sizeof(internal_names[0]))

static void expect_bool(bool got, bool want, const char *what, const char *arg)
{
	if (got != want) {
		fprintf(stderr, "%s(\"%s\"): %d, expected %d\n", what, arg ? arg : "(null)", got, want);
		fail();
	}
}

static void expect_internal(const char *exe, bool want)
{
	expect_bool(ms_window_match_is_internal_exe(exe), want, "is_internal_exe", exe);
}

/* s in a case pattern: 0 as is, 1 upper, 2 first upper, 3 alternate */
static void recase(char *dst, size_t size, const char *s, int pattern)
{
	size_t i = 0;
	for (; s[i] && i + 1 < size; i++) {
		char c = s[i];
		bool up = pattern == 1 || (pattern == 2 && i == 0) || (pattern == 3 && (i & 1));
		if (up && c >= 'a' && c <= 'z')
			c = (char)(c - 'a' + 'A');
		dst[i] = c;
	}
	dst[i] = 0;
}

static void constants(void **state)
{
	(void)state;
	assert_int_equal(P_CLASS, 0);
	assert_int_equal(P_TITLE, 1);
	assert_int_equal(P_EXE, 2);
	assert_int_equal(NONE, 0x7FFFFFFF);
	assert_int_equal(PENALTY, 0x1000);
}

/* the 15 exact names, in any case, and nothing near them */
static void internal_exact_names(void **state)
{
	(void)state;
	char buf[64];

	for (size_t i = 0; i < N_INTERNAL; i++) {
		const char *name = internal_names[i];
		for (int pattern = 0; pattern < 4; pattern++) {
			recase(buf, sizeof(buf), name, pattern);
			expect_internal(buf, true);
		}

		snprintf(buf, sizeof(buf), "%sx", name);
		expect_internal(buf, false);
		snprintf(buf, sizeof(buf), "x%s", name);
		expect_internal(buf, false);
		snprintf(buf, sizeof(buf), "%.*s", (int)strlen(name) - 1, name);
		expect_internal(buf, false);
		snprintf(buf, sizeof(buf), "%s ", name);
		expect_internal(buf, false);
	}
}

/* "windowsinternal" matches from the start, in any case, over its own
 * length */
static void internal_prefix(void **state)
{
	(void)state;
	expect_internal("windowsinternal", true);
	expect_internal("windowsinternal.foo.exe", true);
	expect_internal("WindowsInternal", true);
	expect_internal("WINDOWSINTERNAL.CBSPREVIEW.EXE", true);
	expect_internal("windowsinternal\xc3\xa9", true);
	expect_internal("windowsinterna", false);
	expect_internal("xwindowsinternal", false);
	expect_internal(" windowsinternal", false);
}

static void internal_other_exes(void **state)
{
	(void)state;
	expect_internal(NULL, false);
	expect_internal("", false);
	expect_internal("explorer.exe", false);
	expect_internal("obs64.exe", false);
	expect_internal("chrome.exe", false);
	expect_internal("unknown", false);
	expect_internal("time", false);
	expect_internal(".exe", false);
	expect_internal("\xc3\xa9", false);
}

/* explorer.exe with an empty title is not listed; the compare is
 * case-sensitive, unlike the internal names */
static void untitled_explorer(void **state)
{
	(void)state;
	assert_true(ms_window_match_is_untitled_explorer("explorer.exe", true));
	assert_false(ms_window_match_is_untitled_explorer("explorer.exe", false));

	/* characterized, not endorsed: Explorer.exe with an empty title is
	 * listed */
	assert_false(ms_window_match_is_untitled_explorer("Explorer.exe", true));
	assert_false(ms_window_match_is_untitled_explorer("EXPLORER.EXE", true));

	assert_false(ms_window_match_is_untitled_explorer("explorer.ex", true));
	assert_false(ms_window_match_is_untitled_explorer("explorer.exe ", true));
	assert_false(ms_window_match_is_untitled_explorer("xexplorer.exe", true));
	assert_false(ms_window_match_is_untitled_explorer("", true));
	assert_false(ms_window_match_is_untitled_explorer(NULL, true));
	assert_false(ms_window_match_is_untitled_explorer(NULL, false));
}

/* "Chrome" or "SDL_app" anywhere in the class, in any case */
static void generic_class(void **state)
{
	(void)state;
	static const char *generic[] = {
		"Chrome_WidgetWin_1",
		"Chrome_RenderWidgetHostHWND",
		"chrome",
		"CHROME",
		"xCHROMEx",
		"MozillaChromeWindow",
		"SDL_app",
		"sdl_APP",
		"MySDL_appWindow",
		"\xc3\xa9"
		"Chrome",
	};
	static const char *specific[] = {
		"",         "Chrom", "hrome", "SDL_ap", "DL_app", "SDL app", "Notepad", "Windows.UI.Core.CoreWindow",
		"\xc3\xa9",
	};

	for (size_t i = 0; i < sizeof(generic) / sizeof(generic[0]); i++)
		expect_bool(ms_window_match_is_generic_class(generic[i]), true, "is_generic_class", generic[i]);
	for (size_t i = 0; i < sizeof(specific) / sizeof(specific[0]); i++)
		expect_bool(ms_window_match_is_generic_class(specific[i]), false, "is_generic_class", specific[i]);
}

/* the two UWP class names, exactly */
static void uwp_class(void **state)
{
	(void)state;
	static const char *other[] = {
		"",
		"windows.ui.core.corewindow",
		"WINDOWS.UI.CORE.COREWINDOW",
		"Windows.UI.Core.CoreWindo",
		"Windows.UI.Core.CoreWindowx",
		"winuidesktopwin32windowclass",
		"WinUIDesktopWin32WindowClas",
		"ApplicationFrameWindow",
		"Chrome_WidgetWin_1",
	};

	assert_true(ms_window_match_is_uwp_class("Windows.UI.Core.CoreWindow"));
	assert_true(ms_window_match_is_uwp_class("WinUIDesktopWin32WindowClass"));
	for (size_t i = 0; i < sizeof(other) / sizeof(other[0]); i++)
		expect_bool(ms_window_match_is_uwp_class(other[i]), false, "is_uwp_class", other[i]);
}

static int rate(int priority, bool uwp, bool generic, bool class_matches, bool exe_matches, int title_cmp)
{
	return ms_window_match_rating(priority, uwp, generic, class_matches, exe_matches, title_cmp);
}

/* class priority: the title's distance when the class matches, plus the
 * penalty when the exe does not */
static void rating_class_priority(void **state)
{
	(void)state;
	assert_int_equal(rate(P_CLASS, false, false, true, true, 0), 0);
	assert_int_equal(rate(P_CLASS, false, false, true, true, 1), 1);
	assert_int_equal(rate(P_CLASS, false, false, true, true, -1), 1);
	assert_int_equal(rate(P_CLASS, false, false, true, true, 200), 200);
	assert_int_equal(rate(P_CLASS, false, false, true, true, -200), 200);
	assert_int_equal(rate(P_CLASS, false, false, true, false, 0), PENALTY);
	assert_int_equal(rate(P_CLASS, false, false, true, false, -7), PENALTY + 7);
	assert_int_equal(rate(P_CLASS, false, false, false, true, 0), NONE);
	assert_int_equal(rate(P_CLASS, false, false, false, false, 0), NONE);
	assert_int_equal(rate(P_CLASS, false, false, false, false, 200), NONE);
}

/* title priority: the exact title alone */
static void rating_title_priority(void **state)
{
	(void)state;
	for (int m = 0; m < 4; m++) {
		bool class_matches = m & 1, exe_matches = m & 2;
		assert_int_equal(rate(P_TITLE, false, false, class_matches, exe_matches, 0), 0);
		assert_int_equal(rate(P_TITLE, false, false, class_matches, exe_matches, 1), NONE);
		assert_int_equal(rate(P_TITLE, false, false, class_matches, exe_matches, -1), NONE);
		assert_int_equal(rate(P_TITLE, false, false, class_matches, exe_matches, 200), NONE);
	}
}

/* exe priority: the title's distance when the exe matches; the class does
 * not count */
static void rating_exe_priority(void **state)
{
	(void)state;
	for (int class_matches = 0; class_matches < 2; class_matches++) {
		assert_int_equal(rate(P_EXE, false, false, class_matches, true, 0), 0);
		assert_int_equal(rate(P_EXE, false, false, class_matches, true, -1), 1);
		assert_int_equal(rate(P_EXE, false, false, class_matches, true, 200), 200);
		assert_int_equal(rate(P_EXE, false, false, class_matches, false, 0), NONE);
		assert_int_equal(rate(P_EXE, false, false, class_matches, false, 1), NONE);
	}
}

/* a generic saved class turns the class priority into the title priority,
 * and leaves the others alone */
static void rating_generic_class(void **state)
{
	(void)state;
	assert_int_equal(rate(P_CLASS, false, true, true, true, 0), 0);
	assert_int_equal(rate(P_CLASS, false, true, true, true, 1), NONE);
	assert_int_equal(rate(P_CLASS, false, true, true, false, 0), 0);
	assert_int_equal(rate(P_CLASS, false, true, false, false, 0), 0);
	assert_int_equal(rate(P_CLASS, false, true, false, false, 200), NONE);
	assert_int_equal(rate(P_TITLE, false, true, false, false, 0), 0);
	assert_int_equal(rate(P_TITLE, false, true, true, true, 1), NONE);
	assert_int_equal(rate(P_EXE, false, true, false, true, 5), 5);
	assert_int_equal(rate(P_EXE, false, true, true, false, 0), NONE);
}

/* a UWP saved class matches on the exact title alone; under the exe
 * priority the exe has to match too */
static void rating_uwp(void **state)
{
	(void)state;
	for (int generic = 0; generic < 2; generic++) {
		for (int class_matches = 0; class_matches < 2; class_matches++) {
			for (int exe_matches = 0; exe_matches < 2; exe_matches++) {
				assert_int_equal(rate(P_CLASS, true, generic, class_matches, exe_matches, 0), 0);
				assert_int_equal(rate(P_CLASS, true, generic, class_matches, exe_matches, 1), NONE);
				assert_int_equal(rate(P_TITLE, true, generic, class_matches, exe_matches, 0), 0);
				assert_int_equal(rate(P_TITLE, true, generic, class_matches, exe_matches, -1), NONE);
				assert_int_equal(rate(P_EXE, true, generic, class_matches, exe_matches, 0),
						 exe_matches ? 0 : NONE);
				assert_int_equal(rate(P_EXE, true, generic, class_matches, exe_matches, 200), NONE);
			}
		}
	}
}

/* characterized, not endorsed: a priority past the last (or below the
 * first) matches nothing, except a UWP window by its exact title */
static void rating_unknown_priority(void **state)
{
	(void)state;
	static const int priorities[] = {3, -1, 100};

	for (size_t i = 0; i < sizeof(priorities) / sizeof(priorities[0]); i++) {
		int p = priorities[i];
		assert_int_equal(rate(p, false, false, true, true, 0), NONE);
		assert_int_equal(rate(p, false, true, true, true, 0), NONE);
		assert_int_equal(rate(p, false, false, true, true, 5), NONE);
		assert_int_equal(rate(p, true, false, false, false, 0), 0);
		assert_int_equal(rate(p, true, false, true, true, 1), NONE);
	}
}

/* the penalty cannot push a class match past "no match": the largest
 * distance astrcmpi() returns is 255 */
static void rating_penalty_bound(void **state)
{
	(void)state;
	assert_int_equal(rate(P_CLASS, false, false, true, false, 255), PENALTY + 255);
	assert_int_equal(rate(P_CLASS, false, false, true, false, -255), PENALTY + 255);
	assert_true(rate(P_CLASS, false, false, true, false, 255) < NONE);
	assert_int_equal(rate(P_EXE, false, false, false, true, -255), 255);
}

/* every priority, crossed with the four flags and title results of 0, 1,
 * -1 and 200, against the rules above written out per case */
static void rating_grid(void **state)
{
	(void)state;
	static const int titles[] = {0, 1, -1, 200};

	for (int p = 0; p <= 3; p++) {
		for (int flags = 0; flags < 16; flags++) {
			bool uwp = flags & 1, generic = flags & 2, class_matches = flags & 4, exe_matches = flags & 8;
			for (size_t t = 0; t < 4; t++) {
				int cmp = titles[t];
				int dist = cmp < 0 ? -cmp : cmp;
				int want;
				int eff = (generic && p == P_CLASS) ? P_TITLE : p;

				if (uwp)
					want = (eff == P_EXE && !exe_matches) || dist ? NONE : 0;
				else if (eff == P_CLASS)
					want = class_matches ? dist + (exe_matches ? 0 : PENALTY) : NONE;
				else if (eff == P_TITLE)
					want = dist ? NONE : 0;
				else if (eff == P_EXE)
					want = exe_matches ? dist : NONE;
				else
					want = NONE;

				int got = rate(p, uwp, generic, class_matches, exe_matches, cmp);
				if (got != want) {
					fprintf(stderr, "rating(%d, %d, %d, %d, %d, %d) = %d, expected %d\n", p, uwp,
						generic, class_matches, exe_matches, cmp, got, want);
					fail();
				}
			}
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(internal_exact_names),
		cmocka_unit_test(internal_prefix),
		cmocka_unit_test(internal_other_exes),
		cmocka_unit_test(untitled_explorer),
		cmocka_unit_test(generic_class),
		cmocka_unit_test(uwp_class),
		cmocka_unit_test(rating_class_priority),
		cmocka_unit_test(rating_title_priority),
		cmocka_unit_test(rating_exe_priority),
		cmocka_unit_test(rating_generic_class),
		cmocka_unit_test(rating_uwp),
		cmocka_unit_test(rating_unknown_priority),
		cmocka_unit_test(rating_penalty_bound),
		cmocka_unit_test(rating_grid),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
