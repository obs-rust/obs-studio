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

#include "libobs/build/libobs-window-match.pch.h"

/* The pure decisions of the window helpers (window-helpers.c), moved: the
 * list's skip tests, the two class tests and the match rating. window-helpers.c
 * keeps every Win32 call, every read and compare of a window's strings, the
 * walk and the list. Declared in window-match.h without EXPORT. */

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
