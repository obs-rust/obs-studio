/* Original obs_load_sources() (#5574) and the real-header adapters of
 * obs-load-sources-fields.c.h on one recording fixture. Under cargo test
 * these adapters are the only definitions of their real names, so the Rust
 * shim reaches this fixture.
 *
 * The fixture holds a pool of real struct obs_source entries and one tagged
 * data array of up to eight entries. obs_data_array_count, obs_data_array_item,
 * obs_data_release, obs_load_source, obs_transition_load, obs_source_load2 and
 * obs_source_release go to recorders that hand out and check tagged pointers
 * and count every data and source reference. obs_load_source returns the
 * case's pool entry for that array entry (or NULL), setting its real
 * info.type to a transition or an input as the case says; the adapters read
 * that real field. On a chosen obs_source_load2 call the fixture flips a pool
 * entry's type, so a type read made before that point shows. The caller's
 * callback is a recorder too and checks its private data.
 *
 * Traced: the count read, every item take and data release with its entry,
 * every load with its entry, every transition load with its source and
 * entry, every second-pass load, callback and source release with its source
 * (NULL included), a mark for a call on another array, a data release or
 * load of an entry not held, or a reference left at the end of a run. The
 * probe runs the original and a candidate on the same case and reports,
 * instead of aborting, any difference in the traces, so mutants are
 * observable.
 *
 * Excluded: an uninitialized core and another thread's change of the array,
 * outside the original's contract; the real data, loads and references stay
 * unchanged Tier 2 (test_load_sources). No intentional differences. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#define LS_POOL 8
#define LS_TRACE 1024
#define LS_NONE 0xff

/* One case: an array of `count` entries (NULL when `null_array`); entry i
 * loads pool entry `slots[i]` (LS_NONE: NULL), made a transition when bit
 * `slots[i]` of `transition` is set and an input otherwise. `runs` calls
 * (1 or 2), with the recording callback when `with_cb`, NULL otherwise. On
 * obs_source_load2 call `edit_call` (from 0, across runs) pool entry
 * `edit_target`'s type flips between transition and input. */
struct oracle_ls_case {
	uint8_t count;
	uint8_t null_array;
	uint8_t slots[LS_POOL];
	uint8_t transition;
	uint8_t with_cb;
	uint8_t runs;
	uint8_t edit_call;
	uint8_t edit_target;
};

typedef void (*ls_fn)(obs_data_array_t *array, obs_load_source_cb cb, void *private_data);

struct ls_fixture {
	struct obs_source sources[LS_POOL];
	/* Tags the array and data recorders hand out. */
	char array_tag;
	char datas[LS_POOL];
	int data_refs[LS_POOL];
	int source_refs[LS_POOL];
	const struct oracle_ls_case *c;
	unsigned load2_calls;
	char trace[LS_TRACE];
	size_t trace_len;
};

#if defined(_MSC_VER)
static __declspec(thread) struct ls_fixture *ls_current;
#else
static _Thread_local struct ls_fixture *ls_current;
#endif

static void ls_event(char value)
{
	struct ls_fixture *f = ls_current;
	if (f->trace_len + 1 < sizeof(f->trace)) {
		f->trace[f->trace_len++] = value;
		f->trace[f->trace_len] = '\0';
	}
}

static obs_data_array_t *ls_array(void)
{
	struct ls_fixture *f = ls_current;
	return f->c->null_array ? NULL : (obs_data_array_t *)(void *)&f->array_tag;
}

static size_t ls_count_of(void)
{
	return ls_current->c->null_array ? 0 : ls_current->c->count;
}

static size_t ls_source_index(const obs_source_t *source)
{
	for (size_t i = 0; i < LS_POOL; i++) {
		if (source == &ls_current->sources[i])
			return i;
	}
	return LS_POOL;
}

static size_t ls_data_index(const obs_data_t *data)
{
	for (size_t i = 0; i < LS_POOL; i++) {
		if ((const void *)data == (const void *)&ls_current->datas[i])
			return i;
	}
	return LS_POOL;
}

/* The source by pool letter, '-' for NULL, '?' for any other pointer. */
static void ls_source_event(const obs_source_t *source)
{
	size_t i = ls_source_index(source);
	ls_event(!source ? '-' : i < LS_POOL ? (char)('a' + (int)i) : '?');
}

/* The data by entry digit, '-' for NULL, '?' for any other pointer; '!' when
 * `held` and this walk holds no reference to it. */
static void ls_data_event(const obs_data_t *data, bool held)
{
	size_t i = ls_data_index(data);
	ls_event(!data ? '-' : i < LS_POOL ? (char)('0' + (int)i) : '?');
	if (held && i < LS_POOL && ls_current->data_refs[i] <= 0)
		ls_event('!');
}

/* 'N'; '!' for another array. */
static size_t ls_count(obs_data_array_t *array)
{
	ls_event('N');
	if (array != ls_array())
		ls_event('!');
	return array ? ls_count_of() : 0;
}

/* 'i' and the entry; NULL past the end or for a NULL array, as the real one. */
static obs_data_t *ls_item(obs_data_array_t *array, size_t index)
{
	struct ls_fixture *f = ls_current;
	ls_event('i');
	ls_event(index < LS_POOL ? (char)('0' + (int)index) : '?');
	if (array != ls_array())
		ls_event('!');
	if (!array || index >= ls_count_of())
		return NULL;
	f->data_refs[index]++;
	return (obs_data_t *)(void *)&f->datas[index];
}

/* 'r' and the entry. */
static void ls_release_data(obs_data_t *data)
{
	ls_event('r');
	ls_data_event(data, true);
	size_t i = ls_data_index(data);
	if (i < LS_POOL)
		ls_current->data_refs[i]--;
}

static void ls_set_type(struct obs_source *source, bool transition)
{
	source->info.type = transition ? OBS_SOURCE_TYPE_TRANSITION : OBS_SOURCE_TYPE_INPUT;
}

/* 'l' and the entry; the case's pool entry, typed and referenced, or NULL. */
static obs_source_t *ls_load(obs_data_t *data)
{
	struct ls_fixture *f = ls_current;
	ls_event('l');
	ls_data_event(data, true);
	size_t i = ls_data_index(data);
	if (i >= LS_POOL)
		return NULL;
	uint8_t slot = f->c->slots[i];
	if (slot >= LS_POOL)
		return NULL;
	ls_set_type(&f->sources[slot], (f->c->transition & (1u << slot)) != 0);
	f->source_refs[slot]++;
	return &f->sources[slot];
}

/* 'T', the source and the entry. */
static void ls_transition_load(obs_source_t *source, obs_data_t *data)
{
	ls_event('T');
	ls_source_event(source);
	ls_data_event(data, true);
}

/* 'D' and the source; flips the case's target type on the chosen call. */
static void ls_load2(obs_source_t *source)
{
	struct ls_fixture *f = ls_current;
	ls_event('D');
	ls_source_event(source);
	if (f->load2_calls++ == f->c->edit_call && f->c->edit_target < LS_POOL) {
		struct obs_source *target = &f->sources[f->c->edit_target];
		ls_set_type(target, target->info.type != OBS_SOURCE_TYPE_TRANSITION);
	}
}

/* 'x' and the source ('-' for NULL). */
static void ls_release_source(obs_source_t *source)
{
	ls_event('x');
	ls_source_event(source);
	size_t i = ls_source_index(source);
	if (i < LS_POOL)
		ls_current->source_refs[i]--;
}

/* The caller's callback: 'c' and the source, '!' for other private data. */
static void ls_callback(void *private_data, obs_source_t *source)
{
	ls_event('c');
	ls_source_event(source);
	if (private_data != (void *)ls_current)
		ls_event('!');
}

void oracle_obs_load_sources(obs_data_array_t *array, obs_load_source_cb cb, void *private_data);

#define obs_data_array_count ls_count
#define obs_data_array_item ls_item
#define obs_data_release ls_release_data
#define obs_load_source ls_load
#define obs_transition_load ls_transition_load
#define obs_source_load2 ls_load2
#define obs_source_release ls_release_source
#define obs_load_sources oracle_obs_load_sources
#include "libobs/obs-load-sources.c.h"
#undef obs_load_sources
#include "load-sources-fields.inc"
#undef obs_source_release
#undef obs_source_load2
#undef obs_transition_load
#undef obs_load_source
#undef obs_data_release
#undef obs_data_array_item
#undef obs_data_array_count

static bool ls_valid(const struct oracle_ls_case *c)
{
	if (c->count > LS_POOL || c->runs < 1 || c->runs > 2)
		return false;
	for (size_t i = 0; i < c->count; i++) {
		if (c->slots[i] >= LS_POOL && c->slots[i] != LS_NONE)
			return false;
	}
	return true;
}

static void ls_arm(struct ls_fixture *f, const struct oracle_ls_case *c)
{
	memset(f, 0, sizeof(*f));
	ls_current = f;
	f->c = c;
}

static void ls_run(ls_fn fn, const struct oracle_ls_case *c, struct ls_fixture *f)
{
	ls_arm(f, c);
	for (uint8_t i = 0; i < c->runs; i++) {
		if (i)
			ls_event('/');
		fn(ls_array(), c->with_cb ? ls_callback : NULL, f);
		ls_event('=');
		/* '?' for a reference the run left behind. */
		for (size_t k = 0; k < LS_POOL; k++) {
			if (f->data_refs[k] || f->source_refs[k])
				ls_event('?');
		}
	}
	ls_current = NULL;
}

/* 0: match, 1: the traces differ. 0x100: the case does not fit the fixture,
 * 0x200: out of memory. */
unsigned oracle_load_sources_probe(ls_fn candidate, const struct oracle_ls_case *c);
unsigned oracle_load_sources_probe(ls_fn candidate, const struct oracle_ls_case *c)
{
	if (!ls_valid(c))
		return 0x100;
	struct ls_fixture *original = malloc(sizeof(*original));
	struct ls_fixture *f = malloc(sizeof(*f));
	if (!f || !original) {
		free(original);
		free(f);
		return 0x200;
	}
	ls_run(oracle_obs_load_sources, c, original);
	ls_run(candidate, c, f);
	unsigned diff = strcmp(original->trace, f->trace) != 0 ? 1 : 0;
	free(original);
	free(f);
	return diff;
}

/* No stdio here: an oracle member defining the UCRT stdio inlines can be
 * pulled into unrelated Windows test links. */
static void ls_copy(char *out, size_t size, const char *text)
{
	if (!size)
		return;
	size_t n = strlen(text);
	if (n >= size)
		n = size - 1;
	memcpy(out, text, n);
	out[n] = '\0';
}

/* A run's trace, for the characterization checks: the candidate's, or the
 * original's when the candidate is NULL; false when the case does not fit. */
bool oracle_load_sources_trace(ls_fn candidate, const struct oracle_ls_case *c, char *trace, size_t trace_size);
bool oracle_load_sources_trace(ls_fn candidate, const struct oracle_ls_case *c, char *trace, size_t trace_size)
{
	if (!ls_valid(c))
		return false;
	struct ls_fixture *f = malloc(sizeof(*f));
	if (!f)
		return false;
	ls_run(candidate ? candidate : oracle_obs_load_sources, c, f);
	ls_copy(trace, trace_size, f->trace);
	free(f);
	return true;
}

/* The adapters against the real calls and field, one step each. */
bool oracle_load_sources_adapters(void);
bool oracle_load_sources_adapters(void)
{
	const struct oracle_ls_case c = {2, 0, {3, 5}, 1u << 5, 1, 1, LS_NONE, LS_NONE};
	struct ls_fixture *f = malloc(sizeof(*f));
	if (!f)
		return false;
	ls_arm(f, &c);
	obs_data_array_t *array = ls_array();
	bool ok = true;

	ok = ok && obs_load_sources_count(array) == 2;
	obs_data_t *d0 = obs_load_sources_item(array, 0);
	obs_data_t *d1 = obs_load_sources_item(array, 1);
	ok = ok && d0 == (obs_data_t *)(void *)&f->datas[0] && d1 == (obs_data_t *)(void *)&f->datas[1];
	ok = ok && obs_load_sources_item(array, 2) == NULL;
	obs_source_t *input = obs_load_sources_load(d0);
	obs_source_t *transition = obs_load_sources_load(d1);
	ok = ok && input == &f->sources[3] && transition == &f->sources[5];
	ok = ok && !obs_load_sources_is_transition(input) && obs_load_sources_is_transition(transition);
	obs_load_sources_transition_load(transition, d1);
	obs_load_sources_load2(input);
	obs_load_sources_release_data(d0);
	obs_load_sources_release_data(d1);
	obs_load_sources_release_source(input);
	obs_load_sources_release_source(transition);
	obs_load_sources_release_source(NULL);
	ok = ok && f->data_refs[0] == 0 && f->data_refs[1] == 0 && f->source_refs[3] == 0 && f->source_refs[5] == 0;
	ok = ok && strcmp(f->trace, "Ni0i1i2l0l1Tf1Ddr0r1xdxfx-") == 0;

	ls_current = NULL;
	free(f);
	return ok;
}
