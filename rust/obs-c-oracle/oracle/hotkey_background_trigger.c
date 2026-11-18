/* Original C query and real adapter over a thread-local, real-header core. */
#include "libobs/build/libobs.pch.h"
#include <assert.h>
#include <stddef.h>
#include <string.h>

static _Thread_local struct obs_core trigger_core;
static _Thread_local bool trigger_available, trigger_held, trigger_found;
static _Thread_local const unsigned char *trigger_bytes;
static _Thread_local size_t trigger_length;
static _Thread_local bool trigger_allocate;
static _Thread_local obs_key_combination_t trigger_expected;
static _Thread_local char *trigger_storage;
static _Thread_local size_t trigger_counts[3];
static _Thread_local int trigger_cookie;

static struct obs_core *trigger_read_core(void)
{
	assert(trigger_held);
	return &trigger_core;
}

static bool trigger_lock(void)
{
	assert(!trigger_held);
	trigger_counts[0]++;
	trigger_held = trigger_available;
	return trigger_held;
}

static void trigger_unlock(void)
{
	assert(trigger_held);
	trigger_counts[1]++;
	trigger_held = false;
}

static bool trigger_callback(void *data, obs_key_combination_t combo, struct dstr *out)
{
	assert(trigger_held);
	assert(data == &trigger_cookie);
	assert(combo.key == trigger_expected.key && combo.modifiers == trigger_expected.modifiers);
	assert(!out->array && !out->len && !out->capacity);
	trigger_counts[2]++;
	if (trigger_allocate) {
		out->array = bmalloc(trigger_length + 1);
		memcpy(out->array, trigger_bytes, trigger_length);
		out->array[trigger_length] = '\0';
		out->len = trigger_length;
		out->capacity = trigger_length + 1;
	}
	trigger_storage = out->array;
	return trigger_found;
}

char *oracle_obs_hotkey_combination_get_background_trigger(obs_key_combination_t combo);
#define obs trigger_read_core()
#define obs_hotkey_seam_lock trigger_lock
#define obs_hotkey_seam_unlock trigger_unlock
#define obs_hotkey_combination_get_background_trigger oracle_obs_hotkey_combination_get_background_trigger
#include "libobs/obs-hotkey-background-trigger.c.h"
#undef obs_hotkey_combination_get_background_trigger
#include "hotkey_background_trigger_fields.inc"
#undef obs_hotkey_seam_unlock
#undef obs_hotkey_seam_lock
#undef obs

void oracle_hotkey_trigger_set(bool available, bool callback, bool found, bool allocate, const unsigned char *bytes,
			       size_t length, obs_key_combination_t combo);
void oracle_hotkey_trigger_set(bool available, bool callback, bool found, bool allocate, const unsigned char *bytes,
			       size_t length, obs_key_combination_t combo)
{
	assert(!trigger_held);
	trigger_available = available;
	trigger_found = found;
	trigger_allocate = allocate;
	trigger_bytes = bytes;
	trigger_length = length;
	trigger_expected = combo;
	trigger_storage = NULL;
	trigger_core.hotkeys.background.shortcut_trigger = callback ? trigger_callback : NULL;
	trigger_core.hotkeys.background.wake_data = &trigger_cookie;
	memset(trigger_counts, 0, sizeof(trigger_counts));
}

void oracle_hotkey_trigger_snapshot(size_t *counts, char **storage);
void oracle_hotkey_trigger_snapshot(size_t *counts, char **storage)
{
	assert(!trigger_held);
	memcpy(counts, trigger_counts, sizeof(trigger_counts));
	*storage = trigger_storage;
}

void oracle_hotkey_trigger_layout(size_t *out);
void oracle_hotkey_trigger_layout(size_t *out)
{
	out[0] = sizeof(obs_key_combination_t);
	out[1] = _Alignof(obs_key_combination_t);
	out[2] = offsetof(obs_key_combination_t, modifiers);
	out[3] = offsetof(obs_key_combination_t, key);
	out[4] = sizeof(struct dstr);
	out[5] = _Alignof(struct dstr);
	out[6] = offsetof(struct dstr, array);
	out[7] = offsetof(struct dstr, len);
	out[8] = offsetof(struct dstr, capacity);
}
