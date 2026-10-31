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

#pragma once

#include <stdbool.h>

/* The pure decisions of the window helpers (window-helpers.c), on C strings,
 * bools and ints (window-match.c). They decide which windows the window and
 * game capture property lists show, and which window a capture source picks
 * when it looks for its saved window again:
 * - ms_window_match_is_internal_exe(): Microsoft shell and system processes
 *   the list hides (15 exact names and the prefix "windowsinternal", without
 *   regard to case). A NULL exe is not internal;
 * - ms_window_match_is_untitled_explorer(): explorer.exe (this exact case)
 *   with an empty title, which the list hides too;
 * - ms_window_match_is_generic_class(): a saved class containing "Chrome" or
 *   "SDL_app" (without regard to case) says little about the window, so the
 *   class priority falls back to the title;
 * - ms_window_match_is_uwp_class(): the saved class of a UWP or WinUI 3
 *   window, which matches on its title alone;
 * - ms_window_match_rating(): how well a window matches the saved one, from
 *   the priority, those two class flags and what window_rating() compared:
 *   0 is exact, lower is better, MS_WINDOW_MATCH_NONE is no match.
 *
 * window-helpers.c keeps every Win32 call, the reads of each window's exe,
 * title and class where they were, the three dstr_cmpi() compares, the
 * window walk, the best-so-far compares and the list building. Nothing here
 * includes a Windows header, so the C tests build it on every platform. Not
 * EXPORT: only libobs calls these. */

/* enum window_priority (window-helpers.h), which includes windows.h;
 * window-helpers.c checks these with _Static_assert. */
#define MS_WINDOW_MATCH_PRIORITY_CLASS 0
#define MS_WINDOW_MATCH_PRIORITY_TITLE 1
#define MS_WINDOW_MATCH_PRIORITY_EXE 2

/* The rating of a window that does not match. */
#define MS_WINDOW_MATCH_NONE 0x7FFFFFFF

/* Added under the class priority when the class matches but the exe does
 * not. */
#define MS_WINDOW_MATCH_EXE_PENALTY 0x1000

bool ms_window_match_is_internal_exe(const char *exe);
bool ms_window_match_is_untitled_explorer(const char *exe, bool title_empty);
bool ms_window_match_is_generic_class(const char *window_class);
bool ms_window_match_is_uwp_class(const char *window_class);

/* title_cmp is dstr_cmpi() of the window's title with the saved one; its
 * absolute value is the title's distance. */
int ms_window_match_rating(int priority, bool uwp_window, bool generic_class, bool class_matches, bool exe_matches,
			   int title_cmp);
