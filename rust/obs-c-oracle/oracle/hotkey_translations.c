/* Test-only oracle: the original libobs/obs-hotkey-translations.c.h,
 * unmodified, with obs_hotkeys_set_translations_s renamed to oracle_* so it
 * can link next to the Rust shim. Its dstr calls are renamed to the C dstr
 * oracle (oracle/dstr.c, the same renames), so the expansion runs on the
 * original util/dstr.c, not on its Rust port. The store it calls,
 * obs_hotkeys_store_key_translation, keeps its name: under cargo test the
 * test binary defines it and records each call, for this oracle and the
 * shim alike. */
#ifndef _WIN32
#define _GNU_SOURCE
#endif
#include <stddef.h>
#include <stdint.h>

#define obs_hotkeys_set_translations_s oracle_obs_hotkeys_set_translations_s

#define astrcmpi oracle_astrcmpi
#define astrcmp_n oracle_astrcmp_n
#define astrcmpi_n oracle_astrcmpi_n
#define astrstri oracle_astrstri
#define strdepad oracle_strdepad
#define strlist_split oracle_strlist_split
#define strlist_free oracle_strlist_free
#define dstr_init_copy_strref oracle_dstr_init_copy_strref
#define dstr_copy oracle_dstr_copy
#define dstr_copy_strref oracle_dstr_copy_strref
#define dstr_ncopy oracle_dstr_ncopy
#define dstr_ncopy_dstr oracle_dstr_ncopy_dstr
#define dstr_cat_dstr oracle_dstr_cat_dstr
#define dstr_cat_strref oracle_dstr_cat_strref
#define dstr_ncat oracle_dstr_ncat
#define dstr_ncat_dstr oracle_dstr_ncat_dstr
#define dstr_insert oracle_dstr_insert
#define dstr_insert_dstr oracle_dstr_insert_dstr
#define dstr_insert_ch oracle_dstr_insert_ch
#define dstr_remove oracle_dstr_remove
#define dstr_safe_printf oracle_dstr_safe_printf
#define dstr_replace oracle_dstr_replace
#define dstr_depad oracle_dstr_depad
#define dstr_left oracle_dstr_left
#define dstr_mid oracle_dstr_mid
#define dstr_right oracle_dstr_right

#include "obs-hotkey-translations.c.h"

/* The layout of struct obs_hotkeys_translations: 0 is sizeof, 1 alignof,
 * 2 + i the offset of member i (in struct order); SIZE_MAX past the end. */
size_t oracle_hotkey_translations_layout(size_t which);

#define TRANSLATION_OFFSET(m) offsetof(struct obs_hotkeys_translations, m)

size_t oracle_hotkey_translations_layout(size_t which)
{
	const size_t values[] = {
		sizeof(struct obs_hotkeys_translations),
		_Alignof(struct obs_hotkeys_translations),
		TRANSLATION_OFFSET(insert),
		TRANSLATION_OFFSET(del),
		TRANSLATION_OFFSET(home),
		TRANSLATION_OFFSET(end),
		TRANSLATION_OFFSET(page_up),
		TRANSLATION_OFFSET(page_down),
		TRANSLATION_OFFSET(num_lock),
		TRANSLATION_OFFSET(scroll_lock),
		TRANSLATION_OFFSET(caps_lock),
		TRANSLATION_OFFSET(backspace),
		TRANSLATION_OFFSET(tab),
		TRANSLATION_OFFSET(print),
		TRANSLATION_OFFSET(pause),
		TRANSLATION_OFFSET(left),
		TRANSLATION_OFFSET(right),
		TRANSLATION_OFFSET(up),
		TRANSLATION_OFFSET(down),
		TRANSLATION_OFFSET(shift),
		TRANSLATION_OFFSET(alt),
		TRANSLATION_OFFSET(control),
		TRANSLATION_OFFSET(meta),
		TRANSLATION_OFFSET(menu),
		TRANSLATION_OFFSET(space),
		TRANSLATION_OFFSET(numpad_num),
		TRANSLATION_OFFSET(numpad_divide),
		TRANSLATION_OFFSET(numpad_multiply),
		TRANSLATION_OFFSET(numpad_minus),
		TRANSLATION_OFFSET(numpad_plus),
		TRANSLATION_OFFSET(numpad_decimal),
		TRANSLATION_OFFSET(apple_keypad_num),
		TRANSLATION_OFFSET(apple_keypad_divide),
		TRANSLATION_OFFSET(apple_keypad_multiply),
		TRANSLATION_OFFSET(apple_keypad_minus),
		TRANSLATION_OFFSET(apple_keypad_plus),
		TRANSLATION_OFFSET(apple_keypad_decimal),
		TRANSLATION_OFFSET(apple_keypad_equal),
		TRANSLATION_OFFSET(mouse_num),
		TRANSLATION_OFFSET(escape),
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : SIZE_MAX;
}

#undef TRANSLATION_OFFSET
