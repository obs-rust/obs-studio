/* Original obs_save_sources_filtered() and obs_save_sources() (#5567) and the
 * real-header adapters of obs-save-sources-fields.c.h on one recording
 * fixture core. Under cargo test these adapters are the only definitions of
 * their real names, so the Rust shim reaches this fixture.
 *
 * The fixture core holds a pool of real struct obs_source entries, linked
 * through their real hh_uuid next links from the core's real sources head in
 * the case's order, each with the case's filter type, removed, temp_removed
 * and private fields. The mutex calls only count holds. obs_data_array_create,
 * obs_save_source, obs_data_array_push_back and obs_data_release go to
 * recorders that hand out and check tagged pointers. The caller's callback is
 * a recorder too: it checks its parameter, keeps the source when the case's
 * bit for that call is set (counted across runs, so its answer for one source
 * can change between calls), and on a chosen call edits the fixture: it sets
 * a source's fields or relinks the current source's next link, so a read made
 * before the callback, or not made again, shows.
 *
 * Traced: every lock and unlock, the array created, every callback with its
 * source, every save, push and release with its source (and a mark when the
 * push is into another array or the sources mutex is not held), and per run
 * whether the returned array is the one created. The probe runs the original
 * and a candidate on the same case and reports, instead of aborting, any
 * difference in the traces or a mutex left held, so mutants are observable.
 *
 * Excluded: an uninitialized core and another thread's table change, outside
 * the original's contract; the real table and data stay unchanged Tier 2
 * (test_save_sources). One intentional difference, not probed: a NULL
 * callback keeps no source, where the original calls NULL. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#define SS_POOL 8
#define SS_TRACE 2048
#define SS_NONE 0xff

#define SS_FILTER 1
#define SS_REMOVED 2
#define SS_TEMP_REMOVED 4
#define SS_PRIVATE 8

/* One case: `count` pool entries listed in `order` (distinct pool indices),
 * each with its `flags`; `runs` calls (1 or 2) of obs_save_sources_filtered
 * (`filtered`) or obs_save_sources. Callback call k (from 0, across runs)
 * keeps its source when bit k of `keep` is set. On callback call
 * `edit_call`, `edit_kind` 0 sets pool entry `edit_target`'s flags to
 * `edit_flags`; 1 relinks the current source's next link to `edit_target`
 * (SS_NONE: NULL), only when that keeps the walk finite. */
struct oracle_ss_case {
	const uint8_t *order;
	size_t count;
	uint8_t flags[SS_POOL];
	uint8_t runs;
	uint8_t filtered;
	uint32_t keep;
	uint8_t edit_call;
	uint8_t edit_kind;
	uint8_t edit_target;
	uint8_t edit_flags;
};

typedef obs_data_array_t *(*ss_filtered_fn)(obs_save_source_filter_cb cb, void *param);
typedef obs_data_array_t *(*ss_all_fn)(void);

struct ss_fixture {
	struct obs_core core;
	struct obs_source sources[SS_POOL];
	/* Tags the data and array recorders hand out. */
	char datas[SS_POOL];
	char arrays[4];
	const struct oracle_ss_case *c;
	unsigned depth;
	unsigned arrays_made;
	unsigned calls;
	obs_data_array_t *current_array;
	char trace[SS_TRACE];
	size_t trace_len;
};

#if defined(_MSC_VER)
static __declspec(thread) struct ss_fixture *ss_current;
#else
static _Thread_local struct ss_fixture *ss_current;
#endif

static void ss_event(char value)
{
	struct ss_fixture *f = ss_current;
	if (f->trace_len + 1 < sizeof(f->trace)) {
		f->trace[f->trace_len++] = value;
		f->trace[f->trace_len] = '\0';
	}
}

static struct obs_core *ss_read_core(void)
{
	return &ss_current->core;
}

static char ss_name(size_t index)
{
	return (char)('a' + (int)index);
}

static size_t ss_source_index(const obs_source_t *source)
{
	for (size_t i = 0; i < SS_POOL; i++) {
		if (source == &ss_current->sources[i])
			return i;
	}
	return SS_POOL;
}

static size_t ss_data_index(const obs_data_t *data)
{
	for (size_t i = 0; i < SS_POOL; i++) {
		if ((const void *)data == (const void *)&ss_current->datas[i])
			return i;
	}
	return SS_POOL;
}

/* The source, or '?' for any other pointer. */
static void ss_source_event(const obs_source_t *source)
{
	size_t i = ss_source_index(source);
	ss_event(i < SS_POOL ? ss_name(i) : '?');
}

/* 'X' marks a step without the sources mutex held. */
static void ss_check_locked(void)
{
	if (!ss_current->depth)
		ss_event('X');
}

/* 'L'; 'R' marks another mutex. */
static int ss_lock(pthread_mutex_t *mutex)
{
	struct ss_fixture *f = ss_current;
	if (mutex != &f->core.data.sources_mutex) {
		ss_event('R');
		return 0;
	}
	ss_event('L');
	f->depth++;
	return 0;
}

/* 'U'; 'R' marks an unlock without a hold or of another mutex. */
static int ss_unlock(pthread_mutex_t *mutex)
{
	struct ss_fixture *f = ss_current;
	if (mutex != &f->core.data.sources_mutex || !f->depth) {
		ss_event('R');
		return 0;
	}
	ss_event('U');
	f->depth--;
	return 0;
}

/* 'A' and the array's number; 'Y' when made under the mutex. */
static obs_data_array_t *ss_array_create(void)
{
	struct ss_fixture *f = ss_current;
	if (f->depth)
		ss_event('Y');
	unsigned n = f->arrays_made++;
	ss_event('A');
	ss_event((char)('0' + (int)(n % 10)));
	obs_data_array_t *array = (obs_data_array_t *)(void *)&f->arrays[n % 4];
	f->current_array = array;
	return array;
}

/* 's' and the source; the data is tagged by the source's pool index. */
static obs_data_t *ss_save(obs_source_t *source)
{
	struct ss_fixture *f = ss_current;
	ss_check_locked();
	ss_event('s');
	ss_source_event(source);
	size_t i = ss_source_index(source);
	return i < SS_POOL ? (obs_data_t *)(void *)&f->datas[i] : NULL;
}

/* 'p' and the data's source; '!' when into another array than the last one
 * made. */
static void ss_push(obs_data_array_t *array, obs_data_t *data)
{
	struct ss_fixture *f = ss_current;
	ss_check_locked();
	ss_event('p');
	size_t i = ss_data_index(data);
	ss_event(i < SS_POOL ? ss_name(i) : '?');
	if (array != f->current_array)
		ss_event('!');
}

/* 'r' and the data's source. */
static void ss_release(obs_data_t *data)
{
	ss_check_locked();
	ss_event('r');
	size_t i = ss_data_index(data);
	ss_event(i < SS_POOL ? ss_name(i) : '?');
}

static void ss_apply_flags(struct obs_source *source, uint8_t flags)
{
	source->info.type = (flags & SS_FILTER) ? OBS_SOURCE_TYPE_FILTER : OBS_SOURCE_TYPE_INPUT;
	source->removed = (flags & SS_REMOVED) != 0;
	source->temp_removed = (flags & SS_TEMP_REMOVED) != 0;
	source->context.private = (flags & SS_PRIVATE) != 0;
}

/* The position of pool entry `index` in the case's listing, or SS_POOL. */
static size_t ss_position(const struct oracle_ss_case *c, size_t index)
{
	for (size_t i = 0; i < c->count; i++) {
		if (c->order[i] == index)
			return i;
	}
	return SS_POOL;
}

/* Whether relinking `current` to `target` keeps the walk finite: NULL, an
 * unlisted entry (its next link is NULL) or one listed after `current`. */
static bool ss_later(const struct oracle_ss_case *c, size_t current, uint8_t target)
{
	if (target >= SS_POOL)
		return target == SS_NONE;
	size_t at = ss_position(c, target);
	return at == SS_POOL || at > ss_position(c, current);
}

/* The caller's callback: 'c' and the source, '!' for another parameter. */
static bool ss_callback(void *param, obs_source_t *source)
{
	struct ss_fixture *f = ss_current;
	ss_check_locked();
	ss_event('c');
	ss_source_event(source);
	if (param != (void *)f)
		ss_event('!');
	const struct oracle_ss_case *c = f->c;
	unsigned call = f->calls++;
	if (call == c->edit_call) {
		if (c->edit_kind == 0 && c->edit_target < SS_POOL) {
			ss_apply_flags(&f->sources[c->edit_target], c->edit_flags);
		} else if (c->edit_kind == 1) {
			size_t i = ss_source_index(source);
			if (i < SS_POOL && ss_later(c, i, c->edit_target))
				f->sources[i].context.hh_uuid.next =
					c->edit_target < SS_POOL ? &f->sources[c->edit_target].context : NULL;
		}
	}
	return call < 32 && (c->keep & (1u << call)) != 0;
}

obs_data_array_t *oracle_obs_save_sources_filtered(obs_save_source_filter_cb cb, void *data_);
obs_data_array_t *oracle_obs_save_sources(void);

/* The mingw build renames every pthread function on the command line
 * (w32-pthreads): keep that name for the rest of the file and use the
 * recorders only for the original and the adapters. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock ss_lock
#define pthread_mutex_unlock ss_unlock
#define obs ss_read_core()
#define obs_data_array_create ss_array_create
#define obs_save_source ss_save
#define obs_data_array_push_back ss_push
#define obs_data_release ss_release
#define obs_save_sources_filtered oracle_obs_save_sources_filtered
#define obs_save_sources oracle_obs_save_sources
#include "libobs/obs-save-sources.c.h"
#undef obs_save_sources
#undef obs_save_sources_filtered
#include "save-sources-fields.inc"
#undef obs_data_release
#undef obs_data_array_push_back
#undef obs_save_source
#undef obs_data_array_create
#undef obs
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

static bool ss_valid(const struct oracle_ss_case *c)
{
	if (c->count > SS_POOL || (c->count && !c->order) || c->runs < 1 || c->runs > 2)
		return false;
	bool seen[SS_POOL] = {false};
	for (size_t i = 0; i < c->count; i++) {
		if (c->order[i] >= SS_POOL || seen[c->order[i]])
			return false;
		seen[c->order[i]] = true;
	}
	return true;
}

static void ss_arm(struct ss_fixture *f, const struct oracle_ss_case *c)
{
	memset(f, 0, sizeof(*f));
	ss_current = f;
	f->c = c;
	for (size_t i = 0; i < SS_POOL; i++)
		ss_apply_flags(&f->sources[i], c->flags[i]);
	struct obs_source *prev = NULL;
	for (size_t i = 0; i < c->count; i++) {
		struct obs_source *source = &f->sources[c->order[i]];
		if (prev)
			prev->context.hh_uuid.next = &source->context;
		else
			f->core.data.sources = source;
		prev = source;
	}
}

static void ss_run(ss_filtered_fn filtered, ss_all_fn all, const struct oracle_ss_case *c, struct ss_fixture *f)
{
	ss_arm(f, c);
	for (uint8_t i = 0; i < c->runs; i++) {
		if (i)
			ss_event('/');
		obs_data_array_t *array = c->filtered ? filtered(ss_callback, f) : all();
		ss_event('=');
		ss_event(array && array == f->current_array ? 'y' : 'n');
	}
	ss_current = NULL;
}

/* 0: match. 1: the traces differ, 4: the candidate (or the original) left
 * the mutex held. 0x100: the case does not fit the fixture, 0x200: out of
 * memory. */
unsigned oracle_save_sources_probe(ss_filtered_fn filtered, ss_all_fn all, const struct oracle_ss_case *c);
unsigned oracle_save_sources_probe(ss_filtered_fn filtered, ss_all_fn all, const struct oracle_ss_case *c)
{
	if (!ss_valid(c))
		return 0x100;
	struct ss_fixture *original = malloc(sizeof(*original));
	struct ss_fixture *f = malloc(sizeof(*f));
	if (!f || !original) {
		free(original);
		free(f);
		return 0x200;
	}
	ss_run(oracle_obs_save_sources_filtered, oracle_obs_save_sources, c, original);
	ss_run(filtered, all, c, f);
	unsigned diff = 0;
	if (strcmp(original->trace, f->trace) != 0)
		diff |= 1;
	if (original->depth || f->depth)
		diff |= 4;
	free(original);
	free(f);
	return diff;
}

/* No stdio here: an oracle member defining the UCRT stdio inlines can be
 * pulled into unrelated Windows test links. */
static void ss_copy(char *out, size_t size, const char *text)
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
 * original's when either candidate is NULL; false when it left the mutex held
 * or the case does not fit. */
bool oracle_save_sources_trace(ss_filtered_fn filtered, ss_all_fn all, const struct oracle_ss_case *c, char *trace,
			       size_t trace_size);
bool oracle_save_sources_trace(ss_filtered_fn filtered, ss_all_fn all, const struct oracle_ss_case *c, char *trace,
			       size_t trace_size)
{
	if (!ss_valid(c))
		return false;
	struct ss_fixture *f = malloc(sizeof(*f));
	if (!f)
		return false;
	if (filtered && all)
		ss_run(filtered, all, c, f);
	else
		ss_run(oracle_obs_save_sources_filtered, oracle_obs_save_sources, c, f);
	ss_copy(trace, trace_size, f->trace);
	bool released = !f->depth;
	free(f);
	return released;
}

/* The adapters against the real fields, one step each. */
bool oracle_save_sources_adapters(void);
bool oracle_save_sources_adapters(void)
{
	static const uint8_t order[] = {2, 0, 1};
	const struct oracle_ss_case c = {
		order, 3, {SS_PRIVATE, SS_REMOVED, SS_FILTER | SS_TEMP_REMOVED}, 1, 0, 0, SS_NONE, 0, 0, 0};
	struct ss_fixture *f = malloc(sizeof(*f));
	if (!f)
		return false;
	ss_arm(f, &c);
	obs_source_t *a = &f->sources[0], *b = &f->sources[1], *s = &f->sources[2];
	bool ok = true;

	obs_data_array_t *array = obs_save_sources_array_create();
	ok = ok && array == f->current_array;
	obs_save_sources_lock();
	ok = ok && f->depth == 1;
	ok = ok && obs_save_sources_first() == s && obs_save_sources_next(s) == a && obs_save_sources_next(a) == b &&
	     obs_save_sources_next(b) == NULL;
	ok = ok && obs_save_sources_is_filter(s) && !obs_save_sources_is_filter(a) && !obs_save_sources_is_filter(b);
	ok = ok && !obs_save_sources_removed(s) && !obs_save_sources_removed(a) && obs_save_sources_removed(b);
	ok = ok && obs_save_sources_temp_removed(s) && !obs_save_sources_temp_removed(a) &&
	     !obs_save_sources_temp_removed(b);
	ok = ok && !obs_save_sources_private(s) && obs_save_sources_private(a) && !obs_save_sources_private(b);
	obs_data_t *data = obs_save_sources_save(a);
	ok = ok && data == (obs_data_t *)(void *)&f->datas[0];
	obs_save_sources_push(array, data);
	obs_save_sources_release(data);
	obs_save_sources_unlock();
	ok = ok && f->depth == 0;
	ok = ok && strcmp(f->trace, "A0LsaparaU") == 0;

	ss_current = NULL;
	free(f);
	return ok;
}
