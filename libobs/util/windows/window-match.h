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
#include <stddef.h>

/* The pure decisions of the window helpers (window-helpers.c), on C strings,
 * bools and ints, and the codec of the saved window string (window-match.c).
 * They decide which windows the window and game capture property lists show,
 * and which window a capture source picks when it looks for its saved window
 * again:
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
 *   0 is exact, lower is better, MS_WINDOW_MATCH_NONE is no match;
 * - ms_window_match_encode(), ms_window_match_decode() and
 *   ms_window_match_split(): a capture source saves its window as
 *   "title:class:exe", each part with '#' escaped as "#22", then ':' as
 *   "#3A"; ms_build_window_strings() splits it at the first two ':' and
 *   decodes "#3A", then "#22", in each part.
 *
 * window-helpers.c keeps every Win32 call, the reads of each window's exe,
 * title and class where they were, the three dstr_cmpi() compares, the
 * window walk, the best-so-far compares, the list building and every
 * allocation of the window strings. Nothing here includes a Windows header,
 * so the C tests build it on every platform. Not EXPORT: only libobs calls
 * these. */

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

/* s with '#' escaped as "#22" and ':' as "#3A", like snprintf(): the
 * escaped length is returned, and when out is not NULL, at most
 * out_size - 1 bytes of it are written, then a NUL (none when out_size is
 * 0). A NULL s is "". */
size_t ms_window_match_encode(const char *s, char *out, size_t out_size);

/* s decoded in place: every "#3A" (this case) to ':', then every "#22" to
 * '#', each pass left to right and never into what it wrote ("#2222" is
 * "#22"). Returns the new length; a NULL s is left alone and is 0. */
size_t ms_window_match_decode(char *s);

/* The first three fields of str split at ':', empty ones kept: field i is
 * the len[i] bytes at str + start[i], the third ending at the next ':' or
 * the end, and later fields are ignored. With fewer than three fields
 * (fewer than two ':') or a NULL str, false, and all six are 0. */
bool ms_window_match_split(const char *str, size_t start[3], size_t len[3]);
