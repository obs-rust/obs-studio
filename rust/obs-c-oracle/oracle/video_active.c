/* Original obs_video_active() and the real-header adapters of
 * obs-video-active-fields.c.h on one recording fixture core. Under cargo
 * test these adapters are the only definitions of their real names, so the
 * Rust shim reaches this fixture. Every read of the core, the mutex lock and
 * unlock, and every counter load (with the mix it belongs to) are traced; the
 * probe runs the original and a candidate on the same fixture and reports,
 * instead of aborting, any difference in the answer or the trace, a counter
 * loaded outside the lock, the mutex left held, or any byte of the mix list
 * or the mixes changed, so mutants are observable.
 *
 * Excluded: an uninitialized core, a NULL or dead listed mix and a concurrent
 * mix list change (outside the original's contract; the real lifecycle stays
 * unchanged Tier 2, test_video_active). No intentional differences. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

#define VACT_POOL 8
#define VACT_LIST 16
#define VACT_TRACE 160

static _Thread_local struct obs_core vact_core;
static _Thread_local struct obs_core_video_mix vact_pool[VACT_POOL];
static _Thread_local struct obs_core_video_mix *vact_storage[VACT_LIST];
static _Thread_local bool vact_locked;
static _Thread_local char vact_trace[VACT_TRACE];
static _Thread_local size_t vact_trace_len;

static void vact_event(char value)
{
	if (vact_trace_len + 1 < sizeof(vact_trace)) {
		vact_trace[vact_trace_len++] = value;
		vact_trace[vact_trace_len] = '\0';
	}
}

/* 'o' reads the core. */
static struct obs_core *vact_read_core(void)
{
	vact_event('o');
	return &vact_core;
}

/* 'L' locks; 'R' marks a recursive lock or another mutex. */
static int vact_lock(pthread_mutex_t *mutex)
{
	if (mutex != &vact_core.video.mixes_mutex || vact_locked)
		vact_event('R');
	vact_event('L');
	int result = pthread_mutex_lock(mutex);
	vact_locked = true;
	return result;
}

/* 'U' unlocks; 'R' marks an unlock without the lock or of another mutex. */
static int vact_unlock(pthread_mutex_t *mutex)
{
	if (mutex != &vact_core.video.mixes_mutex || !vact_locked)
		vact_event('R');
	vact_event('U');
	vact_locked = false;
	return pthread_mutex_unlock(mutex);
}

/* 'r' or 'g' then the mix's name loads its raw or GPU count; 'X' marks a
 * load outside the lock, '?' a load of anything else. */
static long vact_load(const volatile long *ptr)
{
	if (!vact_locked)
		vact_event('X');
	for (size_t i = 0; i < VACT_POOL; i++) {
		if (ptr == &vact_pool[i].raw_active || ptr == &vact_pool[i].gpu_encoder_active) {
			vact_event(ptr == &vact_pool[i].raw_active ? 'r' : 'g');
			vact_event((char)('0' + i));
			return os_atomic_load_long(ptr);
		}
	}
	vact_event('?');
	return os_atomic_load_long(ptr);
}

bool oracle_obs_video_active(void);

/* The mingw build renames every pthread function on the command line
 * (w32-pthreads): keep that name for the rest of the file and use the
 * recorders only for the original and the adapters. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock vact_lock
#define pthread_mutex_unlock vact_unlock
#define os_atomic_load_long vact_load
#define obs vact_read_core()
#define obs_video_active oracle_obs_video_active
#include "libobs/obs-video-active.c.h"
#undef obs_video_active
#include "video_active_fields.inc"
#undef obs
#undef os_atomic_load_long
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

typedef bool (*vact_fn)(void);

struct vact_result {
	char trace[VACT_TRACE];
	bool answer;
	bool untouched;
};

/* `list` names pool mixes (duplicates allowed); `raw` and `gpu` hold each
 * pool mix's counts. The bytes around the counts are filled, so a write
 * anywhere in a mix shows. */
static void vact_arm(const uint8_t *list, size_t count, const long *raw, const long *gpu)
{
	memset(&vact_core, 0, sizeof(vact_core));
	memset(vact_pool, 0x77, sizeof(vact_pool));
	memset(vact_storage, 0, sizeof(vact_storage));
	for (size_t i = 0; i < VACT_POOL; i++) {
		vact_pool[i].raw_active = raw[i];
		vact_pool[i].gpu_encoder_active = gpu[i];
	}
	pthread_mutex_init(&vact_core.video.mixes_mutex, NULL);
	for (size_t i = 0; i < count; i++)
		vact_storage[i] = &vact_pool[list[i] % VACT_POOL];
	vact_core.video.mixes.array = vact_storage;
	vact_core.video.mixes.num = count;
	vact_core.video.mixes.capacity = VACT_LIST;
	vact_locked = false;
	vact_trace_len = 0;
	vact_trace[0] = '\0';
}

static void vact_run(vact_fn active, const uint8_t *list, size_t count, const long *raw, const long *gpu,
		     struct vact_result *out)
{
	static _Thread_local struct obs_core_video_mix pool_before[VACT_POOL];
	static _Thread_local struct obs_core_video_mix *storage_before[VACT_LIST];
	vact_arm(list, count, raw, gpu);
	memcpy(pool_before, vact_pool, sizeof(vact_pool));
	memcpy(storage_before, vact_storage, sizeof(vact_storage));
	out->answer = active();
	memcpy(out->trace, vact_trace, sizeof(out->trace));
	out->untouched = memcmp(vact_pool, pool_before, sizeof(vact_pool)) == 0 &&
			 memcmp(vact_storage, storage_before, sizeof(vact_storage)) == 0 && !vact_locked &&
			 vact_core.video.mixes.array == vact_storage && vact_core.video.mixes.num == count &&
			 vact_core.video.mixes.capacity == VACT_LIST;
	pthread_mutex_destroy(&vact_core.video.mixes_mutex);
}

/* 0: match. 1: the answers differ, 2: the traces differ, 4: the candidate
 * (or the original) left the mutex held or changed a byte of the list or a
 * mix. 0x100: the list is too long for the fixture. */
unsigned oracle_video_active_probe(vact_fn candidate, const uint8_t *list, size_t count, const long *raw,
				   const long *gpu);
unsigned oracle_video_active_probe(vact_fn candidate, const uint8_t *list, size_t count, const long *raw,
				   const long *gpu)
{
	static _Thread_local struct vact_result original, actual;
	if (count > VACT_LIST)
		return 0x100;
	vact_run(oracle_obs_video_active, list, count, raw, gpu, &original);
	vact_run(candidate, list, count, raw, gpu, &actual);
	unsigned diff = 0;
	if (original.answer != actual.answer)
		diff |= 1;
	if (strcmp(original.trace, actual.trace) != 0)
		diff |= 2;
	if (!actual.untouched || !original.untouched)
		diff |= 4;
	return diff;
}

/* No stdio here: an oracle member defining the UCRT stdio inlines can be
 * pulled into unrelated Windows test links. */
static void vact_copy(char *out, size_t size, const char *text)
{
	if (!size)
		return;
	size_t n = strlen(text);
	if (n >= size)
		n = size - 1;
	memcpy(out, text, n);
	out[n] = '\0';
}

/* The original's answer and trace, for the characterization checks. */
bool oracle_video_active_original(const uint8_t *list, size_t count, const long *raw, const long *gpu, char *trace,
				  size_t trace_size);
bool oracle_video_active_original(const uint8_t *list, size_t count, const long *raw, const long *gpu, char *trace,
				  size_t trace_size)
{
	static _Thread_local struct vact_result result;
	if (count > VACT_LIST)
		return false;
	vact_run(oracle_obs_video_active, list, count, raw, gpu, &result);
	vact_copy(trace, trace_size, result.trace);
	return result.answer;
}

/* The adapters' reads against the real fields. */
bool oracle_video_active_adapters(void);
bool oracle_video_active_adapters(void)
{
	static const uint8_t list[] = {3, 1};
	long raw[VACT_POOL] = {0}, gpu[VACT_POOL] = {0};
	raw[3] = -5;
	gpu[3] = 9;
	raw[1] = 4;
	vact_arm(list, 2, raw, gpu);
	obs_video_active_lock();
	bool ok = vact_locked && obs_video_active_count() == 2 && obs_video_active_at(0) == &vact_pool[3] &&
		  obs_video_active_at(1) == &vact_pool[1] && obs_video_active_raw(&vact_pool[3]) == -5 &&
		  obs_video_active_gpu(&vact_pool[3]) == 9 && obs_video_active_raw(&vact_pool[1]) == 4 &&
		  obs_video_active_gpu(&vact_pool[1]) == 0;
	obs_video_active_unlock();
	ok = ok && !vact_locked && strcmp(vact_trace, "oLooor3g3r1g1oU") == 0;
	pthread_mutex_destroy(&vact_core.video.mixes_mutex);
	return ok;
}
