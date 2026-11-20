/* Original key-translation query and store and the real-header adapters of
 * obs-hotkey-key-translation-fields.c.h on one recording fixture core. The
 * probes run the original and a candidate through the same core reads, bfree
 * and bstrdup, and report every difference instead of aborting, so mutants
 * are observable. Under cargo test these adapters are the only definitions of
 * their real names, so the Rust shims and anything calling them (the
 * translations shim, through the store) reach this fixture. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

typedef const char *(*kt_query_fn)(obs_key_t key, const char *def);
typedef void (*kt_store_fn)(obs_key_t key, const char *translation);
/* Called with NULL when a store frees the old name, then with the string a
 * store copies. */
typedef void (*kt_hook_fn)(int key, const char *translation);

static _Thread_local struct obs_core kt_core;
static _Thread_local bool kt_forbid;
static _Thread_local int kt_key, kt_last_key;
static _Thread_local const char *kt_arg;
static _Thread_local long kt_live;
static _Thread_local unsigned kt_reads;
static _Thread_local char kt_events[32];
static _Thread_local size_t kt_event_count;
static _Thread_local kt_hook_fn kt_hook;

static void kt_event(char value)
{
	if (kt_event_count + 1 < sizeof(kt_events)) {
		kt_events[kt_event_count++] = value;
		kt_events[kt_event_count] = '\0';
	}
}

/* Reads are counted, not traced: C leaves the order of a slot read and the
 * bstrdup on the right of the same assignment unspecified. 'X' marks a read
 * of the core for OBS_KEY_NONE. */
static struct obs_core *kt_read_core(void)
{
	if (kt_forbid)
		kt_event('X');
	kt_reads++;
	return &kt_core;
}

/* 'F' frees a name, 'f' frees NULL; 'P' marks a freed pointer that was not
 * the key's current slot. */
static void kt_free(void *ptr)
{
	if (kt_hook) {
		kt_hook(kt_last_key, NULL);
	} else if (ptr != kt_core.hotkeys.translations[kt_key]) {
		kt_event('P');
	}
	kt_event(ptr ? 'F' : 'f');
	if (ptr)
		kt_live--;
	bfree(ptr);
}

/* 'D' copies a name; 'A' marks a copy of anything but the caller's string. */
static char *kt_dup(const char *str)
{
	if (kt_hook) {
		kt_hook(kt_last_key, str);
	} else if (str != kt_arg) {
		kt_event('A');
	}
	kt_event('D');
	char *copy = bstrdup(str);
	if (copy)
		kt_live++;
	return copy;
}

const char *oracle_obs_get_hotkey_translation(obs_key_t key, const char *def);
void oracle_obs_hotkeys_store_key_translation(obs_key_t key, const char *translation);
char **oracle_kt_fields_slot(obs_key_t key);

#define obs kt_read_core()
#define bfree kt_free
#define bstrdup kt_dup
#define obs_get_hotkey_translation oracle_obs_get_hotkey_translation
#define obs_hotkeys_store_key_translation oracle_obs_hotkeys_store_key_translation
#include "libobs/obs-hotkey-key-translation.c.h"
#undef obs_hotkeys_store_key_translation
#undef obs_get_hotkey_translation
#define obs_hotkey_key_translation_slot oracle_kt_fields_slot
#include "hotkey_key_translation_fields.inc"
#undef obs_hotkey_key_translation_slot
#undef bstrdup
#undef bfree
#undef obs

/* The real-named slot adapter: the fields adapter, remembering the key for
 * the store hook. */
char **obs_hotkey_key_translation_slot(obs_key_t key);
char **obs_hotkey_key_translation_slot(obs_key_t key)
{
	kt_last_key = key;
	return oracle_kt_fields_slot(key);
}

static void kt_clear(void)
{
	for (size_t i = 0; i < OBS_KEY_LAST_VALUE; i++) {
		bfree(kt_core.hotkeys.translations[i]);
		kt_core.hotkeys.translations[i] = NULL;
	}
}

static void kt_arm(int key, const char *initial, const char *arg, bool forbid)
{
	kt_hook = NULL;
	kt_clear();
	if (initial)
		kt_core.hotkeys.translations[key] = bstrdup(initial);
	kt_key = key;
	kt_last_key = key;
	kt_arg = arg;
	kt_live = 0;
	kt_reads = 0;
	kt_forbid = forbid;
	kt_event_count = 0;
	kt_events[0] = '\0';
}

/* Every slot but `key` is still NULL. */
static bool kt_others_clear(int key)
{
	for (int i = 0; i < OBS_KEY_LAST_VALUE; i++) {
		if (i != key && kt_core.hotkeys.translations[i])
			return false;
	}
	return true;
}

/* Free every slot and route each later store to `hook`, or stop with NULL. */
void oracle_hotkey_key_translation_record(kt_hook_fn hook);
void oracle_hotkey_key_translation_record(kt_hook_fn hook)
{
	kt_hook = NULL;
	kt_clear();
	kt_hook = hook;
}

struct kt_query_result {
	char events[32];
	unsigned reads;
	int kind; /* 0 NULL, 1 the default, 2 the stored slot, 3 anything else */
	bool state;
};

static void kt_run_query(kt_query_fn query, int key, const char *stored, const char *def, struct kt_query_result *out)
{
	kt_arm(key, stored, NULL, key == OBS_KEY_NONE);
	char *before = kt_core.hotkeys.translations[key];
	const char *result = query((obs_key_t)key, def);
	memset(out, 0, sizeof(*out));
	memcpy(out->events, kt_events, sizeof(out->events));
	out->reads = kt_reads;
	out->kind = !result ? 0 : result == def ? 1 : result == before ? 2 : 3;
	out->state = kt_core.hotkeys.translations[key] == before && kt_live == 0 && kt_others_clear(key);
	kt_clear();
}

/* Bits of the result: 1 trace or read count, 2 returned pointer, 4 slot or
 * allocations changed. */
unsigned oracle_hotkey_key_translation_query_probe(int key, const char *stored, const char *def, kt_query_fn candidate);
unsigned oracle_hotkey_key_translation_query_probe(int key, const char *stored, const char *def, kt_query_fn candidate)
{
	struct kt_query_result original, other;
	kt_run_query(oracle_obs_get_hotkey_translation, key, stored, def, &original);
	kt_run_query(candidate, key, stored, def, &other);
	unsigned diff = 0;
	if (strcmp(original.events, other.events) != 0 || original.reads != other.reads || strchr(other.events, 'X'))
		diff |= 1;
	if (original.kind != other.kind)
		diff |= 2;
	if (!original.state || !other.state)
		diff |= 4;
	return diff;
}

struct kt_store_result {
	char events[32];
	unsigned reads;
	long live;
	bool null_slot, same_text, own_copy, others_clear;
};

static void kt_run_store(kt_store_fn store, int key, const char *initial, const char *arg, struct kt_store_result *out)
{
	kt_arm(key, initial, arg, false);
	store((obs_key_t)key, arg);
	const char *slot = kt_core.hotkeys.translations[key];
	memset(out, 0, sizeof(*out));
	memcpy(out->events, kt_events, sizeof(out->events));
	out->reads = kt_reads;
	out->live = kt_live;
	out->null_slot = !slot;
	out->same_text = slot && arg && strcmp(slot, arg) == 0;
	out->own_copy = !slot || slot != arg;
	out->others_clear = kt_others_clear(key);
	kt_clear();
}

/* Bits of the result: 1 trace or read count, 2 stored slot, 4 allocation
 * balance or another key touched. */
unsigned oracle_hotkey_key_translation_store_probe(int key, const char *initial, const char *arg,
						   kt_store_fn candidate);
unsigned oracle_hotkey_key_translation_store_probe(int key, const char *initial, const char *arg, kt_store_fn candidate)
{
	struct kt_store_result original, other;
	kt_run_store(oracle_obs_hotkeys_store_key_translation, key, initial, arg, &original);
	kt_run_store(candidate, key, initial, arg, &other);
	unsigned diff = 0;
	if (strcmp(original.events, other.events) != 0 || original.reads != other.reads || strpbrk(other.events, "PA"))
		diff |= 1;
	if (original.null_slot != other.null_slot || original.same_text != other.same_text || !other.own_copy)
		diff |= 2;
	long expected = (arg ? 1 : 0) - (initial ? 1 : 0);
	if (original.live != other.live || other.live != expected || !other.others_clear)
		diff |= 4;
	return diff;
}

/* One store's or query's trace and read count, for exact expectations. */
void oracle_hotkey_key_translation_trace(bool query, int key, const char *initial, const char *arg, char *out,
					 size_t size, unsigned *reads);
void oracle_hotkey_key_translation_trace(bool query, int key, const char *initial, const char *arg, char *out,
					 size_t size, unsigned *reads)
{
	if (query) {
		struct kt_query_result run;
		kt_run_query(oracle_obs_get_hotkey_translation, key, initial, arg, &run);
		strncpy(out, run.events, size - 1);
		*reads = run.reads;
	} else {
		struct kt_store_result run;
		kt_run_store(oracle_obs_hotkeys_store_key_translation, key, initial, arg, &run);
		strncpy(out, run.events, size - 1);
		*reads = run.reads;
	}
	out[size - 1] = '\0';
}

void oracle_hotkey_key_translation_layout(size_t *out);
void oracle_hotkey_key_translation_layout(size_t *out)
{
	out[0] = OBS_KEY_LAST_VALUE;
	out[1] = sizeof(kt_core.hotkeys.translations);
	out[2] = sizeof(kt_core.hotkeys.translations[0]);
	out[3] = sizeof(obs_key_t);
	out[4] = OBS_KEY_NONE;
	out[5] = OBS_KEY_A;
	out[6] = OBS_KEY_B;
}
