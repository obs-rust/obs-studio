/* Original atomic update, with instrumented recursive registry seams. */
#include "libobs/build/libobs.pch.h"
#include <assert.h>
#include <string.h>

static _Thread_local bool atomic_initialized;
static _Thread_local size_t atomic_depth;
static _Thread_local size_t atomic_trace[256];
static _Thread_local size_t atomic_count;
static _Thread_local void *atomic_data;

static void atomic_record(size_t event)
{
	assert(atomic_count < 256);
	atomic_trace[atomic_count++] = event;
}

bool oracle_hotkey_atomic_lock(void);
bool oracle_hotkey_atomic_lock(void)
{
	atomic_record(64 + atomic_depth);
	if (atomic_initialized)
		atomic_depth++;
	return atomic_initialized;
}

void oracle_hotkey_atomic_unlock(void);
void oracle_hotkey_atomic_unlock(void)
{
	assert(atomic_depth > 0);
	atomic_record(128 + atomic_depth);
	atomic_depth--;
}

void oracle_obs_hotkey_update_atomic(obs_hotkey_atomic_update_func func, void *data);
#define obs_hotkey_seam_lock oracle_hotkey_atomic_lock
#define obs_hotkey_seam_unlock oracle_hotkey_atomic_unlock
#define obs_hotkey_update_atomic oracle_obs_hotkey_update_atomic
#include "obs-hotkey-update-atomic.c.h"
#undef obs_hotkey_update_atomic
#undef obs_hotkey_seam_unlock
#undef obs_hotkey_seam_lock

void oracle_hotkey_atomic_reset(bool initialized, void *data);
void oracle_hotkey_atomic_reset(bool initialized, void *data)
{
	assert(atomic_depth == 0);
	atomic_initialized = initialized;
	atomic_data = data;
	atomic_count = 0;
}

size_t oracle_hotkey_atomic_callback(void *data);
size_t oracle_hotkey_atomic_callback(void *data)
{
	atomic_record(192 + atomic_depth);
	atomic_record(data == atomic_data);
	return atomic_depth;
}

size_t oracle_hotkey_atomic_trace(size_t *out);
size_t oracle_hotkey_atomic_trace(size_t *out)
{
	assert(atomic_depth == 0);
	memcpy(out, atomic_trace, atomic_count * sizeof(size_t));
	return atomic_count;
}
