/* Original-C pair metadata setters, real private layout and integer-key hash.
 * Single setters are probes; actual copying/signals are covered by Tier 2. */
#include "libobs/build/libobs.pch.h"
#include <assert.h>
#include <string.h>

static _Thread_local struct obs_core meta_core;
static _Thread_local struct obs_hotkey_pair meta_pair;
static _Thread_local size_t meta_trace[32];
static _Thread_local size_t meta_count;
static _Thread_local const char *meta_inputs[2];
static _Thread_local bool meta_redirect;

void oracle_obs_hotkey_pair_set_names(obs_hotkey_pair_id id, const char *first, const char *second);
void oracle_obs_hotkey_pair_set_descriptions(obs_hotkey_pair_id id, const char *first, const char *second);

#define obs (&meta_core)
#define obs_hotkey_pair_set_names oracle_obs_hotkey_pair_set_names
#define obs_hotkey_pair_set_descriptions oracle_obs_hotkey_pair_set_descriptions
#include "obs-hotkey-pair-meta.c.h"
#undef obs_hotkey_pair_set_descriptions
#undef obs_hotkey_pair_set_names
#undef obs

/* Probe exactly which public setter, ID and original pointer are forwarded. */
static void meta_record(size_t field, obs_hotkey_id id, const char *value)
{
	assert(meta_count + 3 <= 32);
	meta_trace[meta_count++] = field;
	meta_trace[meta_count++] = id;
	meta_trace[meta_count++] = value == meta_inputs[0] ? 0 : value == meta_inputs[1] ? 1 : 2;
	if (meta_redirect && meta_count == 3)
		meta_pair.id[1] = 33;
}

void obs_hotkey_set_name(obs_hotkey_id id, const char *value)
{
	meta_record(0, id, value);
}

void obs_hotkey_set_description(obs_hotkey_id id, const char *value)
{
	meta_record(1, id, value);
}

void oracle_pair_meta_reset(size_t id, bool present, bool redirect, const char *first, const char *second);
void oracle_pair_meta_reset(size_t id, bool present, bool redirect, const char *first, const char *second)
{
	if (meta_core.hotkeys.hotkey_pairs)
		HASH_CLEAR(hh, meta_core.hotkeys.hotkey_pairs);
	memset(&meta_core, 0, sizeof(meta_core));
	memset(&meta_pair, 0, sizeof(meta_pair));
	meta_count = 0;
	meta_inputs[0] = first;
	meta_inputs[1] = second;
	meta_redirect = redirect;
	meta_pair.pair_id = id;
	meta_pair.id[0] = 11;
	meta_pair.id[1] = 22;
	if (present) {
		unsigned hash = (unsigned)(id % UINT_MAX);
		HASH_ADD_KEYPTR_BYHASHVALUE(hh, meta_core.hotkeys.hotkey_pairs, &meta_pair.pair_id, sizeof(size_t),
					    hash, &meta_pair);
	}
}

obs_hotkey_pair_t *oracle_pair_meta_find(size_t id);
obs_hotkey_pair_t *oracle_pair_meta_find(size_t id)
{
	obs_hotkey_pair_t *pair;
	unsigned hash = (unsigned)(id % UINT_MAX);
	HASH_FIND_BYHASHVALUE(hh, meta_core.hotkeys.hotkey_pairs, &id, sizeof(size_t), hash, pair);
	return pair;
}

size_t oracle_pair_meta_id(const obs_hotkey_pair_t *pair, size_t half);
size_t oracle_pair_meta_id(const obs_hotkey_pair_t *pair, size_t half)
{
	assert(pair == &meta_pair && half < 2);
	return pair->id[half];
}

size_t oracle_pair_meta_finish(size_t *out);
size_t oracle_pair_meta_finish(size_t *out)
{
	memcpy(out, meta_trace, meta_count * sizeof(size_t));
	if (meta_core.hotkeys.hotkey_pairs)
		HASH_CLEAR(hh, meta_core.hotkeys.hotkey_pairs);
	return meta_count;
}
