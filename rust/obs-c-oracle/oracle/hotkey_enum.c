/* The extracted original C, unchanged, over a bounded thread-local registry. */
#include "libobs/build/libobs.pch.h"
#include "obs-hotkey-seam.h"
#include <stdlib.h>

#if defined(_MSC_VER)
#define HKENUM_TLS __declspec(thread)
#else
#define HKENUM_TLS _Thread_local
#endif
static HKENUM_TLS struct obs_core hkenum_core;
static HKENUM_TLS size_t *hkenum_trace;
static HKENUM_TLS size_t hkenum_count;
static HKENUM_TLS size_t hkenum_visits;
static HKENUM_TLS size_t hkenum_limit;
static HKENUM_TLS bool hkenum_lock_ok;
#undef HKENUM_TLS

static void hkenum_record(size_t value)
{
	hkenum_trace[hkenum_count++] = value;
}
static bool hkenum_lock(void)
{
	hkenum_record(SIZE_MAX);
	return hkenum_lock_ok;
}
static void hkenum_unlock(void)
{
	hkenum_record(SIZE_MAX - 1);
}
static obs_hotkey_t *hkenum_find(obs_hotkey_id id)
{
	hkenum_record(id + 1000);
	for (obs_hotkey_t *key = hkenum_core.hotkeys.hotkeys; key; key = key->hh.next)
		if (key->id == id)
			return key;
	return NULL;
}
static void hkenum_bindings(obs_hotkey_binding_internal_enum_func func, void *data)
{
	for (size_t i = 0; i < hkenum_core.hotkeys.bindings.num; i++)
		if (!func(data, i, &hkenum_core.hotkeys.bindings.array[i]))
			break;
}

void oracle_obs_enum_hotkeys(obs_hotkey_enum_func func, void *data);
void oracle_obs_enum_hotkey_bindings(obs_hotkey_binding_enum_func func, void *data);
void oracle_obs_hotkey_seam_enum_context(struct obs_context_data *context, obs_hotkey_internal_enum_func func,
					 void *data);
#define obs (&hkenum_core)
#define obs_hotkey_seam_lock hkenum_lock
#define obs_hotkey_seam_unlock hkenum_unlock
#define obs_hotkey_seam_find hkenum_find
#define obs_hotkey_seam_enum_bindings hkenum_bindings
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
#undef obs

static bool hkenum_collect(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	(void)data;
	if (key->id != id)
		abort();
	hkenum_record(id);
	return ++hkenum_visits < hkenum_limit;
}
static bool hkenum_collect_context(void *data, obs_hotkey_t *key)
{
	return hkenum_collect(data, key->id, key);
}
static bool hkenum_collect_binding(void *data, size_t index, obs_hotkey_binding_t *binding)
{
	(void)data;
	if (index != hkenum_visits)
		abort();
	hkenum_record(binding->hotkey_id);
	return ++hkenum_visits < hkenum_limit;
}

size_t oracle_hotkey_enum_run(size_t live, const size_t *ids, size_t count, size_t limit, int kind, bool lock_ok,
			      size_t *trace);
size_t oracle_hotkey_enum_run(size_t live, const size_t *ids, size_t count, size_t limit, int kind, bool lock_ok,
			      size_t *trace)
{
	if (live > 16 || count > 32)
		abort();
	struct obs_hotkey keys[16] = {0};
	struct obs_hotkey_binding bindings[16] = {0};
	struct obs_context_data context = {0};
	memset(&hkenum_core, 0, sizeof(hkenum_core));
	for (size_t i = 0; i < live; i++) {
		keys[i].id = i;
		keys[i].hh.next = i + 1 < live ? &keys[i + 1] : NULL;
		bindings[i].hotkey_id = i;
	}
	hkenum_core.hotkeys.hotkeys = live ? keys : NULL;
	hkenum_core.hotkeys.bindings.array = bindings;
	hkenum_core.hotkeys.bindings.num = live;
	/* The original only reads this storage. Avoid casting away const. */
	size_t context_ids[32];
	for (size_t i = 0; i < count; i++)
		context_ids[i] = ids[i];
	context.hotkeys.array = context_ids;
	context.hotkeys.num = count;
	hkenum_trace = trace;
	hkenum_count = hkenum_visits = 0;
	hkenum_limit = limit;
	hkenum_lock_ok = lock_ok;
	if (kind == 0)
		oracle_obs_enum_hotkeys(hkenum_collect, NULL);
	else if (kind == 1)
		oracle_obs_hotkey_seam_enum_context(&context, hkenum_collect_context, NULL);
	else
		oracle_obs_enum_hotkey_bindings(hkenum_collect_binding, NULL);
	hkenum_core.hotkeys.hotkeys = NULL;
	hkenum_core.hotkeys.bindings.array = NULL;
	hkenum_trace = NULL;
	return hkenum_count;
}
