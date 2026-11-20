/* Original obs_canvas_clear_mix() and the real-header adapters of
 * obs-canvas-clear-mix-fields.c.h on one recording fixture core. Under
 * cargo test these adapters are the only definitions of their real names, so
 * the Rust shim reaches this fixture. Every read of the core, the mutex
 * lock and unlock, and the teardown are traced; the probe runs the original
 * and a candidate on the same fixture and reports, instead of aborting, any
 * difference in the trace, the list left behind, the mixes freed, or the
 * bytes of the canvas, its references, its source and the mixes, so mutants
 * are observable. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

#define CM_POOL 8
#define CM_NONE 0xff
#define CM_TRACE 96

static _Thread_local struct obs_core cm_core;
static _Thread_local struct obs_core *cm_current;
static _Thread_local struct obs_canvas cm_canvas;
static _Thread_local struct obs_weak_canvas cm_control;
static _Thread_local struct obs_source cm_source;
static _Thread_local struct obs_core_video_mix cm_pool[CM_POOL];
static _Thread_local struct obs_core_video_mix *cm_storage[16];
static _Thread_local bool cm_locked;
static _Thread_local char cm_trace[CM_TRACE];
static _Thread_local size_t cm_trace_len;

static void cm_event(char value)
{
	if (cm_trace_len + 1 < sizeof(cm_trace)) {
		cm_trace[cm_trace_len++] = value;
		cm_trace[cm_trace_len] = '\0';
	}
}

static char cm_name(const struct obs_core_video_mix *mix)
{
	if (!mix)
		return '-';
	for (size_t i = 0; i < CM_POOL; i++) {
		if (mix == &cm_pool[i])
			return (char)('0' + i);
	}
	return '?';
}

/* 'o' reads the core. */
static struct obs_core *cm_read_core(void)
{
	cm_event('o');
	return cm_current;
}

/* 'L' locks; 'R' marks a recursive lock or another mutex. */
static int cm_lock(pthread_mutex_t *mutex)
{
	if (mutex != &cm_core.video.mixes_mutex || cm_locked)
		cm_event('R');
	cm_event('L');
	int result = pthread_mutex_lock(mutex);
	cm_locked = true;
	return result;
}

/* 'U' unlocks; 'D' marks a canvas already detached at the unlock. */
static int cm_unlock(pthread_mutex_t *mutex)
{
	if (mutex != &cm_core.video.mixes_mutex || !cm_locked)
		cm_event('R');
	if (!cm_canvas.mix)
		cm_event('D');
	cm_event('U');
	cm_locked = false;
	return pthread_mutex_unlock(mutex);
}

/* 'F' then the mix's name frees it; 'X' marks a free outside the lock, 'D' a
 * canvas already detached, 'E' a list not yet erased. */
static _Thread_local size_t cm_count_at_free;
static void cm_free(struct obs_core_video_mix *mix)
{
	if (!cm_locked)
		cm_event('X');
	if (cm_canvas.mix != mix)
		cm_event('D');
	if (cm_core.video.mixes.num != cm_count_at_free)
		cm_event('E');
	cm_event('F');
	cm_event(cm_name(mix));
}

void oracle_obs_canvas_clear_mix(obs_canvas_t *canvas);

/* The mingw build renames every pthread function on the command line
 * (w32-pthreads): keep that name for the rest of the file and use the
 * recorders only for the original and the adapters. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock cm_lock
#define pthread_mutex_unlock cm_unlock
#define obs cm_read_core()
#define obs_free_video_mix cm_free
#define obs_canvas_clear_mix oracle_obs_canvas_clear_mix
#include "libobs/obs-canvas-clear-mix.c.h"
#undef obs_canvas_clear_mix
#include "canvas_clear_mix_fields.inc"
#undef obs_free_video_mix
#undef obs
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

typedef void (*cm_clear_fn)(obs_canvas_t *canvas);

struct cm_result {
	char trace[CM_TRACE];
	char list[17];
	bool detached;
	bool untouched;
};

/* `list` names pool mixes (CM_NONE is a NULL entry); `canvas_mix` names the
 * canvas's mix, CM_NONE for none. A mix the list does not name is an
 * unregistered one. */
static void cm_arm(const uint8_t *list, size_t count, uint8_t canvas_mix, bool null_core)
{
	memset(&cm_core, 0, sizeof(cm_core));
	memset(&cm_canvas, 0x5a, sizeof(cm_canvas));
	memset(&cm_control, 0, sizeof(cm_control));
	memset(&cm_source, 0x3c, sizeof(cm_source));
	memset(cm_pool, 0x77, sizeof(cm_pool));
	cm_control.canvas = &cm_canvas;
	cm_control.ref.refs = 17;
	cm_control.ref.weak_refs = 23;
	cm_canvas.context.control = (struct obs_weak_object *)&cm_control;
	cm_canvas.sources = &cm_source;
	cm_canvas.view.channels[0] = &cm_source;
	cm_canvas.mix = canvas_mix == CM_NONE ? NULL : &cm_pool[canvas_mix % CM_POOL];
	pthread_mutex_init(&cm_core.video.mixes_mutex, NULL);
	for (size_t i = 0; i < count; i++)
		cm_storage[i] = list[i] == CM_NONE ? NULL : &cm_pool[list[i] % CM_POOL];
	cm_core.video.mixes.array = cm_storage;
	cm_core.video.mixes.num = count;
	cm_core.video.mixes.capacity = sizeof(cm_storage) / sizeof(cm_storage[0]);
	cm_count_at_free = count ? count - 1 : 0;
	cm_current = null_core ? NULL : &cm_core;
	cm_locked = false;
	cm_trace_len = 0;
	cm_trace[0] = '\0';
}

static void cm_run(cm_clear_fn clear, const uint8_t *list, size_t count, uint8_t canvas_mix, bool null_core,
		   struct cm_result *out)
{
	static _Thread_local struct obs_canvas canvas_before;
	static _Thread_local struct obs_weak_canvas control_before;
	static _Thread_local struct obs_source source_before;
	static _Thread_local struct obs_core_video_mix pool_before[CM_POOL];
	cm_arm(list, count, canvas_mix, null_core);
	memcpy(&canvas_before, &cm_canvas, sizeof(cm_canvas));
	memcpy(&control_before, &cm_control, sizeof(cm_control));
	memcpy(&source_before, &cm_source, sizeof(cm_source));
	memcpy(pool_before, cm_pool, sizeof(cm_pool));
	clear(&cm_canvas);
	memcpy(out->trace, cm_trace, sizeof(out->trace));
	size_t n = cm_core.video.mixes.num;
	for (size_t i = 0; i < n && i < 16; i++)
		out->list[i] = cm_name(cm_core.video.mixes.array[i]);
	out->list[n < 16 ? n : 16] = '\0';
	out->detached = cm_canvas.mix == NULL;
	/* The C detaches only a canvas that had a mix. */
	canvas_before.mix = canvas_mix == CM_NONE ? canvas_before.mix : NULL;
	out->untouched = memcmp(&cm_canvas, &canvas_before, sizeof(cm_canvas)) == 0 &&
			 memcmp(&cm_control, &control_before, sizeof(cm_control)) == 0 &&
			 memcmp(&cm_source, &source_before, sizeof(cm_source)) == 0 &&
			 memcmp(cm_pool, pool_before, sizeof(cm_pool)) == 0 && !cm_locked &&
			 cm_core.video.mixes.array == cm_storage;
	cm_current = NULL;
	pthread_mutex_destroy(&cm_core.video.mixes_mutex);
}

/* 0: match. 1: the traces differ, 2: the lists left differ, 4: the detach
 * differs, 8: the candidate touched anything else. */
unsigned oracle_canvas_clear_mix_probe(cm_clear_fn candidate, const uint8_t *list, size_t count, uint8_t canvas_mix,
				       bool null_core);
unsigned oracle_canvas_clear_mix_probe(cm_clear_fn candidate, const uint8_t *list, size_t count, uint8_t canvas_mix,
				       bool null_core)
{
	static _Thread_local struct cm_result original, actual;
	if (count > 16)
		return 0x100;
	cm_run(oracle_obs_canvas_clear_mix, list, count, canvas_mix, null_core, &original);
	cm_run(candidate, list, count, canvas_mix, null_core, &actual);
	unsigned diff = 0;
	if (strcmp(original.trace, actual.trace) != 0)
		diff |= 1;
	if (strcmp(original.list, actual.list) != 0)
		diff |= 2;
	if (original.detached != actual.detached)
		diff |= 4;
	if (!actual.untouched || !original.untouched)
		diff |= 8;
	return diff;
}

/* No stdio here: an oracle member defining the UCRT stdio inlines can be
 * pulled into unrelated Windows test links. */
static void cm_copy(char *out, size_t size, const char *text)
{
	if (!size)
		return;
	size_t n = strlen(text);
	if (n >= size)
		n = size - 1;
	memcpy(out, text, n);
	out[n] = '\0';
}

/* The original's trace and list, for the characterization checks. */
void oracle_canvas_clear_mix_original(const uint8_t *list, size_t count, uint8_t canvas_mix, char *trace,
				      size_t trace_size, char *rest, size_t rest_size);
void oracle_canvas_clear_mix_original(const uint8_t *list, size_t count, uint8_t canvas_mix, char *trace,
				      size_t trace_size, char *rest, size_t rest_size)
{
	static _Thread_local struct cm_result result;
	cm_run(oracle_obs_canvas_clear_mix, list, count, canvas_mix, false, &result);
	cm_copy(trace, trace_size, result.trace);
	cm_copy(rest, rest_size, result.list);
}

/* The adapters' addresses and reads against the real fields. */
bool oracle_canvas_clear_mix_adapters(void);
bool oracle_canvas_clear_mix_adapters(void)
{
	static const uint8_t list[] = {3, 1, CM_NONE};
	cm_arm(list, 3, 1, false);
	bool ok = obs_canvas_clear_mix_slot(&cm_canvas) == &cm_canvas.mix && obs_canvas_clear_mix_count() == 3 &&
		  obs_canvas_clear_mix_at(0) == &cm_pool[3] && obs_canvas_clear_mix_at(1) == &cm_pool[1] &&
		  obs_canvas_clear_mix_at(2) == NULL && strcmp(cm_trace, "oooo") == 0;
	cm_current = NULL;
	pthread_mutex_destroy(&cm_core.video.mixes_mutex);
	return ok;
}
