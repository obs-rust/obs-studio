/* Test-only oracle: the original libobs/obs-hotkey-pair.c.h, unmodified,
 * with its two callbacks renamed to oracle_* so they can link next to the
 * Rust shims. The functions it calls keep their real names: the pair
 * accessor obs_hotkey_pair_fields() and the call wrapper
 * obs_hotkey_pair_call_func(). Under cargo test the test binary defines them
 * over fake pairs and logs every call, for this oracle and the shims
 * alike. */
#ifndef _WIN32
#define _GNU_SOURCE
#endif
#include <stddef.h>
#include <stdint.h>

#define obs_hotkey_pair_first_func oracle_obs_hotkey_pair_first_func
#define obs_hotkey_pair_second_func oracle_obs_hotkey_pair_second_func

#include "obs-hotkey-pair.c.h"

/* The layouts the C uses, in the order of
 * rust/obs-c-oracle/src/hotkey_pair.rs. */
enum {
	ORACLE_HOTKEY_PAIR_LAYOUT_COUNT = 6,
};

size_t oracle_hotkey_pair_layout(size_t which);

size_t oracle_hotkey_pair_layout(size_t which)
{
	const size_t values[ORACLE_HOTKEY_PAIR_LAYOUT_COUNT] = {
		sizeof(struct obs_hotkey_pair_fields),
		_Alignof(struct obs_hotkey_pair_fields),
		offsetof(struct obs_hotkey_pair_fields, pressed0),
		offsetof(struct obs_hotkey_pair_fields, pressed1),
		sizeof(bool),
		sizeof(obs_hotkey_id),
	};
	return which < ORACLE_HOTKEY_PAIR_LAYOUT_COUNT ? values[which] : 0;
}
