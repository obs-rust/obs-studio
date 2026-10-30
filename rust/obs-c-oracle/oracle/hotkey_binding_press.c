/* Test-only oracle: the original libobs/obs-hotkey-binding-press.c.h,
 * unmodified, with its two entry points renamed to oracle_* so they can link
 * next to the Rust shims. The functions it calls keep their real names: the
 * binding's count accessor obs_hotkey_binding_count_fields() and the dispatch
 * obs_hotkey_dispatch(). Under cargo test the test binary defines them over
 * fake bindings and logs every call, for this oracle and the shims alike. */
#ifndef _WIN32
#define _GNU_SOURCE
#endif
#include <stddef.h>
#include <stdint.h>

#define obs_hotkey_binding_press oracle_obs_hotkey_binding_press
#define obs_hotkey_binding_release oracle_obs_hotkey_binding_release

#include "obs-hotkey-binding-press.c.h"

/* The layouts the C uses, in the order of
 * rust/obs-c-oracle/src/hotkey_binding_press.rs. */
enum {
	ORACLE_HOTKEY_BINDING_PRESS_LAYOUT_COUNT = 7,
};

size_t oracle_hotkey_binding_press_layout(size_t which);

size_t oracle_hotkey_binding_press_layout(size_t which)
{
	const size_t values[ORACLE_HOTKEY_BINDING_PRESS_LAYOUT_COUNT] = {
		sizeof(struct obs_hotkey_binding_count_fields),
		_Alignof(struct obs_hotkey_binding_count_fields),
		offsetof(struct obs_hotkey_binding_count_fields, pressed),
		offsetof(struct obs_hotkey_binding_count_fields, count),
		offsetof(struct obs_hotkey_binding_count_fields, hotkey),
		sizeof(bool),
		sizeof(int),
	};
	return which < ORACLE_HOTKEY_BINDING_PRESS_LAYOUT_COUNT ? values[which] : 0;
}
