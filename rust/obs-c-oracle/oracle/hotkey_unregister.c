/* Original extracted implementation, with dependencies recording effects. */
#include "obs-internal.h"
#include <limits.h>
#include <stdlib.h>
#include <string.h>

struct unregister_trace {
	struct obs_core core;
	obs_hotkey_t key;
	int events[16];
	size_t count;
	size_t source_releases;
};

#ifdef _MSC_VER
static __declspec(thread) struct unregister_trace *unregister_current;
#else
static _Thread_local struct unregister_trace *unregister_current;
#endif

static void unregister_event(int event)
{
	unregister_current->events[unregister_current->count++] = event;
}

static void unregister_signal(const char *signal, obs_hotkey_t *key)
{
	(void)signal;
	(void)key;
	/* HASH_DEL happened before this callback. */
	unregister_event(unregister_current->core.hotkeys.hotkeys ? -1 : 1);
	unregister_event(2);
}

static bool unregister_bindings(obs_hotkey_id id)
{
	(void)id;
	/* The clear is a field write, so observe it at the next callback. */
	if (unregister_current->key.registerer_type != OBS_HOTKEY_REGISTERER_SOURCE)
		unregister_event(unregister_current->key.registerer ? -3 : 3);
	unregister_event(4);
	return true;
}

static void unregister_source(void *owner)
{
	if (unregister_current->source_releases++)
		unregister_event(unregister_current->key.registerer ? -3 : 3);
	unregister_event(12 + (owner != NULL));
}

static void unregister_output(void *owner)
{
	unregister_event(14 + (owner != NULL));
}

static void unregister_encoder(void *owner)
{
	unregister_event(16 + (owner != NULL));
}

static void unregister_service(void *owner)
{
	unregister_event(18 + (owner != NULL));
}

static void unregister_free(void *ptr)
{
	if (ptr == unregister_current->key.name)
		unregister_event(5);
	else if (ptr == unregister_current->key.description)
		unregister_event(6);
	else if (ptr == &unregister_current->key)
		unregister_event(7);
}

#define obs (&unregister_current->core)
#define obs_hotkey_seam_signal unregister_signal
#define obs_hotkey_seam_remove_bindings unregister_bindings
#define obs_weak_source_release unregister_source
#define obs_weak_output_release unregister_output
#define obs_weak_encoder_release unregister_encoder
#define obs_weak_service_release unregister_service
#define bfree unregister_free
#define obs_hotkey_unregister_release_registerer oracle_hotkey_unregister_release_registerer
#define obs_hotkey_unregister_one oracle_hotkey_unregister_one
#include "hotkey-unregister.inc"

#undef HASH_FUNCTION
#define HASH_FUNCTION(s, len, hashv) (hashv) = (unsigned int)(*s % UINT_MAX)

size_t oracle_hotkey_unregister_trace(int category, bool owner, bool present, size_t id, int *events);

size_t oracle_hotkey_unregister_trace(int category, bool owner, bool present, size_t id, int *events)
{
	struct unregister_trace *trace = calloc(1, sizeof(*trace));
	if (!trace)
		abort();
	unregister_current = trace;
	trace->core.hotkeys.next_id = 1;
	trace->key.id = 0;
	trace->key.name = (char *)&trace->events[0];
	trace->key.description = (char *)&trace->events[1];
	trace->key.registerer_type = (obs_hotkey_registerer_t)category;
	trace->key.registerer = owner ? trace : NULL;
	if (present)
		HASH_ADD(hh, trace->core.hotkeys.hotkeys, id, sizeof(size_t), &trace->key);
	unregister_hotkey(id);
	size_t count = trace->count;
	memcpy(events, trace->events, count * sizeof(*events));
	HASH_CLEAR(hh, trace->core.hotkeys.hotkeys);
	free(trace);
	unregister_current = NULL;
	return count;
}
