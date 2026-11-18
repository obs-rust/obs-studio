/* Unchanged enumeration bodies and real private-layout adapters over a bounded
 * per-thread C world. No production layout is mirrored in Rust. */
#include "libobs/build/libobs.pch.h"
#include <assert.h>
#include <stdint.h>
#include <string.h>

static _Thread_local struct obs_core hkenum_core;
static _Thread_local struct obs_context_data hkenum_context;
static _Thread_local struct obs_hotkey hkenum_keys[16];
static _Thread_local struct obs_hotkey_binding hkenum_bindings[16];
static _Thread_local size_t hkenum_ids[32];
static _Thread_local size_t hkenum_replacement_ids[1];
static _Thread_local size_t hkenum_counts[5];
static _Thread_local bool hkenum_active;
static _Thread_local bool hkenum_available;

/* Separate adapter archive members (including the benchmark's) share the real
 * fixture core without introducing a placeholder production obs global. */
struct obs_core *oracle_hotkey_enum_core(void);
struct obs_core *oracle_hotkey_enum_core(void)
{
	return &hkenum_core;
}

bool oracle_hotkey_enum_active(void);
bool oracle_hotkey_enum_active(void)
{
	return hkenum_active;
}

bool oracle_hotkey_enum_lock(void);
bool oracle_hotkey_enum_lock(void)
{
	hkenum_counts[0]++;
	if (!hkenum_available)
		return false;
	hkenum_counts[2]++;
	if (hkenum_counts[2] > hkenum_counts[3])
		hkenum_counts[3] = hkenum_counts[2];
	return true;
}

void oracle_hotkey_enum_unlock(void);
void oracle_hotkey_enum_unlock(void)
{
	assert(hkenum_counts[2]);
	hkenum_counts[1]++;
	hkenum_counts[2]--;
}

obs_hotkey_t *oracle_hotkey_enum_find(obs_hotkey_id id);
obs_hotkey_t *oracle_hotkey_enum_find(obs_hotkey_id id)
{
	hkenum_counts[4]++;
	for (obs_hotkey_t *key = hkenum_core.hotkeys.hotkeys; key; key = key->hh.next)
		if (key->id == id)
			return key;
	return NULL;
}

/* Dependency seam: the original enum_bindings snapshots its header once. */
void oracle_hotkey_enum_bindings(obs_hotkey_binding_internal_enum_func func, void *data);
void oracle_hotkey_enum_bindings(obs_hotkey_binding_internal_enum_func func, void *data)
{
	const size_t num = hkenum_core.hotkeys.bindings.num;
	obs_hotkey_binding_t *array = hkenum_core.hotkeys.bindings.array;
	for (size_t i = 0; i < num; i++) {
		if (!func(data, i, &array[i]))
			break;
	}
}

void oracle_obs_enum_hotkeys(obs_hotkey_enum_func func, void *data);
void oracle_obs_enum_hotkey_bindings(obs_hotkey_binding_enum_func func, void *data);
void oracle_obs_hotkey_seam_enum_context(struct obs_context_data *context, obs_hotkey_internal_enum_func func,
					 void *data);
#define obs (&hkenum_core)
#define obs_hotkey_seam_lock oracle_hotkey_enum_lock
#define obs_hotkey_seam_unlock oracle_hotkey_enum_unlock
#define obs_hotkey_seam_find oracle_hotkey_enum_find
#define obs_hotkey_seam_enum_bindings oracle_hotkey_enum_bindings
#define obs_enum_hotkeys oracle_obs_enum_hotkeys
#define obs_enum_hotkey_bindings oracle_obs_enum_hotkey_bindings
#define obs_hotkey_seam_enum_context oracle_obs_hotkey_seam_enum_context
#include "libobs/obs-hotkey-enum.c.h"
#undef obs_hotkey_seam_enum_context
#undef obs_enum_hotkey_bindings
#undef obs_enum_hotkeys
#undef obs_hotkey_seam_enum_bindings
#undef obs_hotkey_seam_find
#undef obs_hotkey_seam_unlock
#undef obs_hotkey_seam_lock

#define obs_hotkey_enum_first oracle_hotkey_enum_first
#define obs_hotkey_enum_next oracle_hotkey_enum_next
#define obs_hotkey_enum_id oracle_hotkey_enum_id
#define obs_hotkey_enum_context_fields oracle_hotkey_enum_context_fields
#include "hotkey-enum-fields.inc"
#undef obs_hotkey_enum_context_fields
#undef obs_hotkey_enum_id
#undef obs_hotkey_enum_next
#undef obs_hotkey_enum_first
#undef obs

void oracle_hotkey_enum_reset(bool available, size_t count, uint32_t live, const size_t *ids, size_t num);
void oracle_hotkey_enum_reset(bool available, size_t count, uint32_t live, const size_t *ids, size_t num)
{
	assert(count <= 16 && num <= 32);
	memset(&hkenum_core, 0, sizeof(hkenum_core));
	memset(&hkenum_context, 0, sizeof(hkenum_context));
	memset(hkenum_keys, 0, sizeof(hkenum_keys));
	memset(hkenum_bindings, 0, sizeof(hkenum_bindings));
	memset(hkenum_counts, 0, sizeof(hkenum_counts));
	hkenum_active = true;
	hkenum_available = available;
	obs_hotkey_t *previous = NULL;
	for (size_t i = 0; i < count; i++) {
		hkenum_keys[i].id = i;
		if (!(live & (UINT32_C(1) << i)))
			continue;
		if (previous)
			previous->hh.next = &hkenum_keys[i];
		else
			hkenum_core.hotkeys.hotkeys = &hkenum_keys[i];
		previous = &hkenum_keys[i];
		hkenum_bindings[hkenum_core.hotkeys.bindings.num].hotkey_id = i;
		hkenum_bindings[hkenum_core.hotkeys.bindings.num++].hotkey = &hkenum_keys[i];
	}
	hkenum_core.hotkeys.bindings.array = hkenum_bindings;
	for (size_t i = 0; i < num; i++)
		hkenum_ids[i] = ids[i];
	hkenum_context.hotkeys.array = hkenum_ids;
	hkenum_context.hotkeys.num = num;
}

void *oracle_hotkey_enum_context(void);
void *oracle_hotkey_enum_context(void)
{
	return &hkenum_context;
}

void oracle_hotkey_enum_clear(void);
void oracle_hotkey_enum_clear(void)
{
	assert(!hkenum_counts[2]);
	hkenum_active = false;
}

void oracle_hotkey_enum_counts(size_t *out);
void oracle_hotkey_enum_counts(size_t *out)
{
	memcpy(out, hkenum_counts, sizeof(hkenum_counts));
}

void oracle_hotkey_enum_remove(size_t id);
void oracle_hotkey_enum_remove(size_t id)
{
	assert(id < 16);
	obs_hotkey_t *key = hkenum_core.hotkeys.hotkeys;
	obs_hotkey_t *previous = NULL;
	while (key && key->id != id) {
		previous = key;
		key = key->hh.next;
	}
	if (key) {
		if (previous)
			previous->hh.next = key->hh.next;
		else
			hkenum_core.hotkeys.hotkeys = key->hh.next;
	}
	/* Poison the removed entry's link: HASH_ITER must have cached it. */
	hkenum_keys[id].hh.next = NULL;
}

void oracle_hotkey_enum_replace_context(void);
void oracle_hotkey_enum_replace_context(void)
{
	hkenum_replacement_ids[0] = SIZE_MAX;
	hkenum_context.hotkeys.array = hkenum_replacement_ids;
	hkenum_context.hotkeys.num = 1;
}

size_t oracle_hotkey_enum_binding_id(const obs_hotkey_binding_t *binding);
size_t oracle_hotkey_enum_binding_id(const obs_hotkey_binding_t *binding)
{
	assert(binding->hotkey->id == binding->hotkey_id);
	return binding->hotkey_id;
}
