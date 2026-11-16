/* Original enumeration and real-header adapter over a test-owned core. No
 * placeholder obs global: both read this thread's core only while held. */
#include "libobs/build/libobs.pch.h"
#include <assert.h>
#include <stddef.h>
#include <string.h>

static _Thread_local struct obs_core background_enum_core;
static _Thread_local bool background_enum_available;
static _Thread_local bool background_enum_held;
static _Thread_local size_t background_enum_counts[2];

static struct obs_core *background_enum_read(void)
{
	assert(background_enum_held);
	return &background_enum_core;
}

bool obs_hotkey_background_enum_test_lock(void);
bool obs_hotkey_background_enum_test_lock(void)
{
	assert(!background_enum_held);
	background_enum_counts[0]++;
	background_enum_held = background_enum_available;
	return background_enum_available;
}

void obs_hotkey_background_enum_test_unlock(void);
void obs_hotkey_background_enum_test_unlock(void)
{
	assert(background_enum_held);
	background_enum_counts[1]++;
	background_enum_held = false;
}

void oracle_obs_hotkeys_enum_background_shortcuts(obs_hotkey_background_enum_func func, void *data);
#define obs background_enum_read()
#define obs_hotkeys_enum_background_shortcuts oracle_obs_hotkeys_enum_background_shortcuts
#define obs_hotkey_seam_lock obs_hotkey_background_enum_test_lock
#define obs_hotkey_seam_unlock obs_hotkey_background_enum_test_unlock
#include "libobs/obs-hotkey-background-enum.c.h"
#include "hotkey_background_enum_fields.inc"
#undef obs_hotkey_seam_unlock
#undef obs_hotkey_seam_lock
#undef obs_hotkeys_enum_background_shortcuts
#undef obs

/* Install the list and availability while unlocked; resets the counters. */
void oracle_hotkey_background_enum_set(bool available, struct obs_hotkey_background_shortcut *array, size_t num);
void oracle_hotkey_background_enum_set(bool available, struct obs_hotkey_background_shortcut *array, size_t num)
{
	assert(!background_enum_held);
	background_enum_available = available;
	background_enum_core.hotkeys.background.shortcuts.array = array;
	background_enum_core.hotkeys.background.shortcuts.num = num;
	background_enum_core.hotkeys.background.shortcuts.capacity = num;
	memset(background_enum_counts, 0, sizeof(background_enum_counts));
}

/* Replace the live list from inside a callback, while the mutex is held. */
void oracle_hotkey_background_enum_replace(struct obs_hotkey_background_shortcut *array, size_t num);
void oracle_hotkey_background_enum_replace(struct obs_hotkey_background_shortcut *array, size_t num)
{
	assert(background_enum_held);
	background_enum_core.hotkeys.background.shortcuts.array = array;
	background_enum_core.hotkeys.background.shortcuts.num = num;
}

bool oracle_hotkey_background_enum_held(void);
bool oracle_hotkey_background_enum_held(void)
{
	return background_enum_held;
}

void oracle_hotkey_background_enum_counts(size_t *out);
void oracle_hotkey_background_enum_counts(size_t *out)
{
	memcpy(out, background_enum_counts, sizeof(background_enum_counts));
}

/* The address of this thread's real-header list, for the borrowed-pointer check. */
const struct darray *oracle_hotkey_background_enum_list(void);
const struct darray *oracle_hotkey_background_enum_list(void)
{
	return &background_enum_core.hotkeys.background.shortcuts.da;
}

void oracle_hotkey_background_enum_layout(size_t *out);
void oracle_hotkey_background_enum_layout(size_t *out)
{
	struct obs_hotkeys_background *background = &background_enum_core.hotkeys.background;
	out[0] = sizeof(struct obs_hotkey_background_shortcut);
	out[1] = _Alignof(struct obs_hotkey_background_shortcut);
	out[2] = offsetof(struct obs_hotkey_background_shortcut, combo);
	out[3] = offsetof(struct obs_hotkey_background_shortcut, trigger);
	out[4] = offsetof(struct obs_hotkey_background_shortcut, bound);
	out[5] = sizeof(obs_key_combination_t);
	out[6] = (size_t)((char *)&background->shortcuts.array - (char *)&background->shortcuts.da.array);
	out[7] = (size_t)((char *)&background->shortcuts.num - (char *)&background->shortcuts.da.num);
}
