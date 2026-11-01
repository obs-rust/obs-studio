/*
This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 2 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/

#include "libobs/build/libobs-ported-window-match.pch.h"

/* Ported to rust/obs-util (window_match): the hybrid libobs drops this
 * file. */

/* The pure decisions of the window helpers (window-helpers.c), moved: the
 * list's skip tests, the two class tests, the match rating and the codec of
 * the saved window string. window-helpers.c keeps every Win32 call, every
 * read and compare of a window's strings, the walk, the list and every
 * allocation. Declared in window-match.h without EXPORT. */

/* not capturable or internal windows, exact executable names */
static const char *internal_microsoft_exes_exact[] = {
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
	NULL,
};

/* partial matches start from the beginning of the executable name */
static const char *internal_microsoft_exes_partial[] = {
	"windowsinternal",
	NULL,
};

bool ms_window_match_is_internal_exe(const char *exe)
{
	if (!exe)
		return false;

	for (const char **vals = internal_microsoft_exes_exact; *vals; vals++) {
		if (astrcmpi(exe, *vals) == 0)
			return true;
	}

	for (const char **vals = internal_microsoft_exes_partial; *vals; vals++) {
		if (astrcmpi_n(exe, *vals, strlen(*vals)) == 0)
			return true;
	}

	return false;
}

bool ms_window_match_is_untitled_explorer(const char *exe, bool title_empty)
{
	return strcmp(exe ? exe : "", "explorer.exe") == 0 && title_empty;
}

static const char *generic_class_substrings[] = {
	"Chrome",
	"SDL_app",
	NULL,
};

bool ms_window_match_is_generic_class(const char *current_class)
{
	const char **class = generic_class_substrings;
	while (*class) {
		if (astrstri(current_class, *class) != NULL) {
			return true;
		}
		class++;
	}

	return false;
}

bool ms_window_match_is_uwp_class(const char *window_class)
{
	return strcmp(window_class, "Windows.UI.Core.CoreWindow") == 0 ||
	       strcmp(window_class, "WinUIDesktopWin32WindowClass") == 0;
}

int ms_window_match_rating(int priority, bool uwp_window, bool generic_class, bool class_matches, bool exe_matches,
			   int title_cmp)
{
	int val = MS_WINDOW_MATCH_NONE;
	int title_val = abs(title_cmp);

	if (generic_class && (priority == MS_WINDOW_MATCH_PRIORITY_CLASS))
		priority = MS_WINDOW_MATCH_PRIORITY_TITLE;

	/* always match by name with UWP windows */
	if (uwp_window) {
		if (priority == MS_WINDOW_MATCH_PRIORITY_EXE && !exe_matches)
			val = MS_WINDOW_MATCH_NONE;
		else
			val = title_val == 0 ? 0 : MS_WINDOW_MATCH_NONE;

	} else if (priority == MS_WINDOW_MATCH_PRIORITY_CLASS) {
		val = class_matches ? title_val : MS_WINDOW_MATCH_NONE;
		if (val != MS_WINDOW_MATCH_NONE && !exe_matches)
			val += MS_WINDOW_MATCH_EXE_PENALTY;

	} else if (priority == MS_WINDOW_MATCH_PRIORITY_TITLE) {
		val = title_val == 0 ? 0 : MS_WINDOW_MATCH_NONE;

	} else if (priority == MS_WINDOW_MATCH_PRIORITY_EXE) {
		val = exe_matches ? title_val : MS_WINDOW_MATCH_NONE;
	}

	return val;
}

size_t ms_window_match_encode(const char *s, char *out, size_t out_size)
{
	size_t len = 0;

	for (; s && *s; s++) {
		const char *escape = *s == '#' ? "#22" : *s == ':' ? "#3A" : NULL;
		size_t n = escape ? 3 : 1;

		for (size_t i = 0; i < n; i++, len++) {
			if (out && len + 1 < out_size)
				out[len] = escape ? escape[i] : *s;
		}
	}

	if (out && out_size)
		out[len < out_size ? len : out_size - 1] = 0;

	return len;
}

/* One dstr_replace() of "#<a><b>" with to: the leftmost match first, and
 * the scan goes on after it, never into what it wrote. */
static size_t decode_pass(char *s, size_t len, char a, char b, char to)
{
	size_t w = 0;

	for (size_t r = 0; r < len;) {
		if (r + 2 < len && s[r] == '#' && s[r + 1] == a && s[r + 2] == b) {
			s[w++] = to;
			r += 3;
		} else {
			s[w++] = s[r++];
		}
	}

	s[w] = 0;
	return w;
}

size_t ms_window_match_decode(char *s)
{
	if (!s)
		return 0;

	size_t len = decode_pass(s, strlen(s), '3', 'A', ':');
	return decode_pass(s, len, '2', '2', '#');
}

bool ms_window_match_split(const char *str, size_t start[3], size_t len[3])
{
	const char *field = str;
	size_t i = 0;

	for (; field && i < 3; i++) {
		const char *end = strchr(field, ':');

		start[i] = (size_t)(field - str);
		len[i] = end ? (size_t)(end - field) : strlen(field);
		field = end ? end + 1 : NULL;
	}

	if (i == 3)
		return true;

	for (i = 0; i < 3; i++) {
		start[i] = 0;
		len[i] = 0;
	}
	return false;
}
