/* Original injection wrapper, real field adapter, and a bounded thread-local
 * recorder. Registry mutation changes the live header, never the saved array. */
#include "libobs/build/libobs.pch.h"
#include <assert.h>
#include <string.h>

static _Thread_local struct obs_core inject_core;
static _Thread_local struct obs_hotkey_binding inject_bindings[16];
static _Thread_local bool inject_present, inject_held, inject_strict, inject_mutate;
static _Thread_local size_t inject_state[7];
static _Thread_local uint32_t inject_visits[16][5];

static void inject_record(unsigned step)
{
	inject_state[4] = inject_state[4] * 4 + step;
}

void *oracle_hotkey_inject_entry_lock(void);
void *oracle_hotkey_inject_entry_lock(void)
{
	assert(!inject_held);
	inject_state[0]++;
	inject_record(1);
	if (!inject_present)
		return NULL;
	inject_held = true;
	inject_core.hotkeys.strict_modifiers = inject_strict;
	return &inject_core;
}

static bool inject_lock_bool(void)
{
	return oracle_hotkey_inject_entry_lock() != NULL;
}

void oracle_hotkey_inject_entry_unlock(void);
void oracle_hotkey_inject_entry_unlock(void)
{
	assert(inject_held);
	inject_state[2]++;
	inject_record(3);
	inject_held = false;
}

void oracle_hotkey_inject_entry_binding(obs_hotkey_binding_t *binding, obs_key_combination_t key, bool pressed,
					bool strict);
void oracle_hotkey_inject_entry_binding(obs_hotkey_binding_t *binding, obs_key_combination_t key, bool pressed,
					bool strict)
{
	assert(inject_held);
	const size_t index = inject_state[3]++;
	assert(index < 16 && binding == &inject_bindings[index]);
	inject_visits[index][0] = (uint32_t)index;
	inject_visits[index][1] = key.modifiers;
	inject_visits[index][2] = (uint32_t)key.key;
	inject_visits[index][3] = pressed;
	inject_visits[index][4] = strict;
	inject_record(2);
	inject_core.hotkeys.strict_modifiers = !inject_core.hotkeys.strict_modifiers;
	if (inject_mutate)
		inject_core.hotkeys.bindings.num = 0;
}

void oracle_hotkey_inject_entry_walk(obs_hotkey_binding_internal_enum_func func, void *data);
void oracle_hotkey_inject_entry_walk(obs_hotkey_binding_internal_enum_func func, void *data)
{
	assert(inject_held && func && data);
	inject_state[1]++;
	inject_record(0);
	inject_core.hotkeys.strict_modifiers = !inject_strict;
	const size_t count = inject_core.hotkeys.bindings.num;
	obs_hotkey_binding_t *array = inject_core.hotkeys.bindings.array;
	for (size_t i = 0; i < count; i++)
		assert(func(data, i, &array[i]));
}

void oracle_obs_hotkey_inject_event(obs_key_combination_t hotkey, bool pressed);
#define obs (&inject_core)
#define obs_hotkey_seam_lock inject_lock_bool
#define obs_hotkey_seam_unlock oracle_hotkey_inject_entry_unlock
#define obs_hotkey_seam_enum_bindings oracle_hotkey_inject_entry_walk
#define obs_hotkey_binding_inject oracle_hotkey_inject_entry_binding
#define obs_hotkey_inject_event oracle_obs_hotkey_inject_event
#include "libobs/obs-hotkey-inject-entry.c.h"
#undef obs_hotkey_inject_event
#undef obs_hotkey_binding_inject
#undef obs_hotkey_seam_enum_bindings
#undef obs_hotkey_seam_unlock
#undef obs_hotkey_seam_lock
#undef obs

#define obs_hotkey_inject_entry_strict oracle_hotkey_inject_entry_strict
#include "hotkey-inject-entry-fields.inc"
#undef obs_hotkey_inject_entry_strict

void oracle_hotkey_inject_entry_reset(bool present, bool strict, size_t count, bool mutate);
void oracle_hotkey_inject_entry_reset(bool present, bool strict, size_t count, bool mutate)
{
	assert(!inject_held && count <= 16);
	memset(&inject_core, 0, sizeof(inject_core));
	memset(inject_state, 0, sizeof(inject_state));
	memset(inject_visits, 0, sizeof(inject_visits));
	inject_present = present;
	inject_strict = strict;
	inject_mutate = mutate;
	inject_core.hotkeys.strict_modifiers = !strict;
	inject_core.hotkeys.bindings.array = inject_bindings;
	inject_core.hotkeys.bindings.num = count;
}

void oracle_hotkey_inject_entry_snapshot(size_t *state, uint32_t *visits);
void oracle_hotkey_inject_entry_snapshot(size_t *state, uint32_t *visits)
{
	assert(!inject_held);
	inject_state[5] = inject_core.hotkeys.strict_modifiers;
	inject_state[6] = inject_core.hotkeys.bindings.num;
	memcpy(state, inject_state, sizeof(inject_state));
	memcpy(visits, inject_visits, sizeof(inject_visits));
}

void oracle_hotkey_inject_entry_layout(size_t *out);
void oracle_hotkey_inject_entry_layout(size_t *out)
{
	out[0] = sizeof(obs_key_combination_t);
	out[1] = _Alignof(obs_key_combination_t);
	out[2] = offsetof(obs_key_combination_t, modifiers);
	out[3] = offsetof(obs_key_combination_t, key);
}
