/* Test-only oracle: the original libobs/util/windows/window-match.c.h,
 * unmodified, with its eight functions renamed to oracle_* so they can link
 * next to the Rust shims, plus its constants as the C sees them. Its
 * compares are the oracle's C dstr (oracle/dstr.c: astrcmpi, astrcmpi_n and
 * astrstri with C's tolower()), strcmp(), strchr(), strlen() and abs(). */
#include <stddef.h>

#define astrcmpi oracle_astrcmpi
#define astrcmpi_n oracle_astrcmpi_n
#define astrstri oracle_astrstri

#define ms_window_match_is_internal_exe oracle_ms_window_match_is_internal_exe
#define ms_window_match_is_untitled_explorer oracle_ms_window_match_is_untitled_explorer
#define ms_window_match_is_generic_class oracle_ms_window_match_is_generic_class
#define ms_window_match_is_uwp_class oracle_ms_window_match_is_uwp_class
#define ms_window_match_rating oracle_ms_window_match_rating
#define ms_window_match_encode oracle_ms_window_match_encode
#define ms_window_match_decode oracle_ms_window_match_decode
#define ms_window_match_split oracle_ms_window_match_split

#include "util/windows/window-match.c.h"

/* The values the C uses, in the order of
 * rust/obs-c-oracle/src/window_match.rs. */
enum {
	ORACLE_WINDOW_MATCH_VALUE_COUNT = 5,
};

int oracle_window_match_value(size_t which);

int oracle_window_match_value(size_t which)
{
	const int values[ORACLE_WINDOW_MATCH_VALUE_COUNT] = {
		MS_WINDOW_MATCH_PRIORITY_CLASS, MS_WINDOW_MATCH_PRIORITY_TITLE, MS_WINDOW_MATCH_PRIORITY_EXE,
		MS_WINDOW_MATCH_NONE,           MS_WINDOW_MATCH_EXE_PENALTY,
	};
	return which < ORACLE_WINDOW_MATCH_VALUE_COUNT ? values[which] : -1;
}
