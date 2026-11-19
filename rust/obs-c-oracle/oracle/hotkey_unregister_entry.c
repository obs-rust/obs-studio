/* Original extracted wrappers. Observe their existing lock/remover seams;
 * unchanged Tier 2 fixtures additionally execute the real pair helper. */
#include "obs.h"
#include <assert.h>
#include <string.h>

static _Thread_local bool entry_present, entry_locked;
static _Thread_local size_t entry_trace[128], entry_count;

static void entry_record(size_t value)
{
	assert(entry_count < 128);
	entry_trace[entry_count++] = value;
}

bool oracle_hotkey_unregister_entry_lock(void);
bool oracle_hotkey_unregister_entry_lock(void)
{
	assert(!entry_locked);
	if (!entry_present)
		return false;
	entry_locked = true;
	entry_record(1);
	return true;
}

void oracle_hotkey_unregister_entry_unlock(void);
void oracle_hotkey_unregister_entry_unlock(void)
{
	assert(entry_locked);
	entry_record(4);
	entry_locked = false;
}

void oracle_hotkey_unregister_entry_single(size_t id);
void oracle_hotkey_unregister_entry_single(size_t id)
{
	assert(entry_locked);
	entry_record(2);
	entry_record(id);
}

void oracle_hotkey_unregister_entry_pair(size_t id);
void oracle_hotkey_unregister_entry_pair(size_t id)
{
	assert(entry_locked);
	entry_record(3);
	entry_record(id);
}

void oracle_obs_hotkey_unregister(obs_hotkey_id id);
void oracle_obs_hotkey_pair_unregister(obs_hotkey_pair_id id);
#define obs_hotkey_seam_lock oracle_hotkey_unregister_entry_lock
#define obs_hotkey_seam_unlock oracle_hotkey_unregister_entry_unlock
#define obs_hotkey_unregister_one oracle_hotkey_unregister_entry_single
#define obs_hotkey_unregister_pair_call oracle_hotkey_unregister_entry_pair
#define obs_hotkey_unregister oracle_obs_hotkey_unregister
#define obs_hotkey_pair_unregister oracle_obs_hotkey_pair_unregister
#include "hotkey-unregister-entry.inc"
#undef obs_hotkey_pair_unregister
#undef obs_hotkey_unregister
#undef obs_hotkey_unregister_pair_call
#undef obs_hotkey_unregister_one
#undef obs_hotkey_seam_unlock
#undef obs_hotkey_seam_lock

void oracle_hotkey_unregister_entry_reset(bool present);
void oracle_hotkey_unregister_entry_reset(bool present)
{
	assert(!entry_locked);
	entry_present = present;
	entry_count = 0;
	memset(entry_trace, 0, sizeof(entry_trace));
}

size_t oracle_hotkey_unregister_entry_snapshot(size_t *trace);
size_t oracle_hotkey_unregister_entry_snapshot(size_t *trace)
{
	assert(!entry_locked);
	memcpy(trace, entry_trace, sizeof(entry_trace));
	return entry_count;
}
