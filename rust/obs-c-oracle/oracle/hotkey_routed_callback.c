/* Compile the unchanged extracted dispatch and the actual private adapter. */
#include "libobs/build/libobs.pch.h"
#include <assert.h>
#include <string.h>

static _Thread_local struct obs_core routed_core;
static _Thread_local struct obs_hotkey routed_keys[2];
static _Thread_local bool routed_initialized;
static _Thread_local bool routed_nested;
static _Thread_local size_t routed_depth;
static _Thread_local size_t routed_trace[256];
static _Thread_local size_t routed_count;
static _Thread_local void (*routed_entry)(obs_hotkey_id, bool);

static void routed_record(size_t kind, size_t id, size_t pressed, size_t data, size_t hotkey)
{
	assert(routed_count + 6 <= 256);
	routed_trace[routed_count++] = kind;
	routed_trace[routed_count++] = id;
	routed_trace[routed_count++] = pressed;
	routed_trace[routed_count++] = routed_depth;
	routed_trace[routed_count++] = data;
	routed_trace[routed_count++] = hotkey;
}

static struct obs_core *routed_read_core(void)
{
	assert(routed_depth > 0);
	return &routed_core;
}

static bool routed_lock(void)
{
	routed_record(1, 0, 0, 0, 0);
	if (routed_initialized)
		routed_depth++;
	return routed_initialized;
}

static void routed_unlock(void)
{
	assert(routed_depth > 0);
	routed_record(2, 0, 0, 0, 0);
	routed_depth--;
}

static obs_hotkey_t *routed_find(obs_hotkey_id id)
{
	assert(routed_depth > 0);
	for (size_t i = 0; i < 2; i++)
		if (routed_keys[i].id == id)
			return &routed_keys[i];
	return NULL;
}

static void routed_func(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	assert(routed_depth > 0);
	routed_record(3, id, pressed, (size_t)data, (size_t)hotkey);
	if (routed_nested && routed_depth == 1)
		routed_entry(routed_keys[hotkey == &routed_keys[0] ? 1 : 0].id, !pressed);
	/* A callback can reenter; the outer lock must still be held on return. */
	assert(routed_depth > 0);
	routed_record(4, id, pressed, (size_t)data, (size_t)hotkey);
}

void oracle_obs_hotkey_trigger_routed_callback(obs_hotkey_id id, bool pressed);
#define obs routed_read_core()
#define obs_hotkey_seam_lock routed_lock
#define obs_hotkey_seam_unlock routed_unlock
#define obs_hotkey_seam_find routed_find
#define obs_hotkey_trigger_routed_callback oracle_obs_hotkey_trigger_routed_callback
#include "obs-hotkey-routed-callback.c.h"
#undef obs_hotkey_trigger_routed_callback
#include "hotkey_routed_callback_fields.inc"
#undef obs_hotkey_seam_find
#undef obs_hotkey_seam_unlock
#undef obs_hotkey_seam_lock
#undef obs

void oracle_routed_callback_reset(bool initialized, bool enabled, size_t first, size_t second, bool nested,
				  void (*entry)(obs_hotkey_id, bool));
void oracle_routed_callback_reset(bool initialized, bool enabled, size_t first, size_t second, bool nested,
				  void (*entry)(obs_hotkey_id, bool))
{
	assert(routed_depth == 0);
	if (routed_core.hotkeys.hotkeys) {
		/* Free the previous uthash table, never the static hotkeys themselves. */
		HASH_CLEAR(hh, routed_core.hotkeys.hotkeys);
	}
	memset(&routed_core, 0, sizeof(routed_core));
	memset(routed_keys, 0, sizeof(routed_keys));
	routed_initialized = initialized;
	routed_nested = nested;
	routed_entry = entry;
	routed_count = 0;
	routed_core.hotkeys.reroute_hotkeys = enabled;
	for (size_t i = 0; i < 2; i++) {
		routed_keys[i].id = i == 0 ? first : second;
		routed_keys[i].data = &routed_keys[1 - i];
		routed_keys[i].func = routed_func;
		/* Match the registry's integer hash used by the extracted unit. */
		unsigned hash = (unsigned)(routed_keys[i].id % UINT_MAX);
		HASH_ADD_KEYPTR_BYHASHVALUE(hh, routed_core.hotkeys.hotkeys, &routed_keys[i].id, sizeof(size_t), hash,
					    &routed_keys[i]);
	}
}

size_t oracle_routed_callback_trace(size_t *out);
size_t oracle_routed_callback_trace(size_t *out)
{
	assert(routed_depth == 0);
	memcpy(out, routed_trace, routed_count * sizeof(size_t));
	/* Retire the table even on a test's last invocation. */
	HASH_CLEAR(hh, routed_core.hotkeys.hotkeys);
	return routed_count;
}
