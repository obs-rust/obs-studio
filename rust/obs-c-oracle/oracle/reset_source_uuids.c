/* Original obs_reset_source_uuids() (#5520) and the real-header adapters of
 * obs-reset-source-uuids-fields.c.h on one recording fixture core. Under
 * cargo test these adapters are the only definitions of their real names, so
 * the Rust shim reaches this fixture.
 *
 * The fixture core holds a pool of real struct obs_source entries, linked in
 * the core's real uthash uuid table (hh_uuid) in the case's order, each with
 * a uuid of its own. The mutex calls only count holds. bfree of a listed
 * context's uuid and os_generate_uuid() go to recorders: the uuid generator
 * hands out a numbered, well formed uuid, the same sequence for every run.
 * Traced: every lock and unlock, every uuid free with its source, every
 * uuid generated, every other free (the tables' own, uthash_free), each with
 * a mark when the sources mutex is not held; then the final table: each
 * entry in next-link order with its uuid, the count, the tail and every
 * lookup by the new uuid (the same source) and by the old one (none). The
 * probe runs the original and a candidate on the same case and reports,
 * instead of aborting, any difference in the traces or a mutex left held,
 * so mutants are observable.
 *
 * The original's working and new table heads are locals and the store of
 * the new head keeps the old head's pointer (the first entry stays first),
 * so neither is visible here; the Tier 1 mirrors check that order.
 *
 * Excluded: an uninitialized core and another thread's table change,
 * outside the original's contract; the real table stays unchanged Tier 2
 * (test_reset_source_uuids). No intentional differences. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#define RU_POOL 8
#define RU_TRACE 2048
#define RU_UUID (UUID_STR_LENGTH + 1)

/* One case: `count` pool entries listed in the table in `order` (distinct
 * pool indices), reset `resets` times (1 or 2). */
struct oracle_ru_case {
	const uint8_t *order;
	size_t count;
	uint8_t resets;
};

typedef void (*ru_fn)(void);

struct ru_fixture {
	struct obs_core core;
	struct obs_source sources[RU_POOL];
	bool listed[RU_POOL];
	/* Each listed entry's uuid before the run. */
	char before[RU_POOL][RU_UUID];
	unsigned depth;
	unsigned generated;
	char trace[RU_TRACE];
	size_t trace_len;
};

#if defined(_MSC_VER)
static __declspec(thread) struct ru_fixture *ru_current;
#else
static _Thread_local struct ru_fixture *ru_current;
#endif

static void ru_event(char value)
{
	struct ru_fixture *f = ru_current;
	if (f->trace_len + 1 < sizeof(f->trace)) {
		f->trace[f->trace_len++] = value;
		f->trace[f->trace_len] = '\0';
	}
}

static void ru_text(const char *text)
{
	while (*text)
		ru_event(*text++);
}

static struct obs_core *ru_read_core(void)
{
	return &ru_current->core;
}

static char ru_name(size_t index)
{
	return (char)('a' + (int)index);
}

/* 'X' marks a step without the sources mutex held. */
static void ru_check_locked(void)
{
	if (!ru_current->depth)
		ru_event('X');
}

/* 'L'; 'R' marks another mutex. */
static int ru_lock(pthread_mutex_t *mutex)
{
	struct ru_fixture *f = ru_current;
	if (mutex != &f->core.data.sources_mutex) {
		ru_event('R');
		return 0;
	}
	ru_event('L');
	f->depth++;
	return 0;
}

/* 'U'; 'R' marks an unlock without a hold or of another mutex. */
static int ru_unlock(pthread_mutex_t *mutex)
{
	struct ru_fixture *f = ru_current;
	if (mutex != &f->core.data.sources_mutex || !f->depth) {
		ru_event('R');
		return 0;
	}
	ru_event('U');
	f->depth--;
	return 0;
}

/* 'f' and the source whose current uuid it is; 'T' for any other block
 * (the tables' own). */
static void ru_bfree(void *ptr)
{
	struct ru_fixture *f = ru_current;
	ru_check_locked();
	size_t i = 0;
	while (i < RU_POOL && !(f->listed[i] && ptr && f->sources[i].context.uuid == ptr))
		i++;
	if (i < RU_POOL) {
		ru_event('f');
		ru_event(ru_name(i));
	} else {
		ru_event('T');
	}
	bfree(ptr);
}

static void ru_hex(char *out, size_t digits, unsigned value)
{
	static const char hex[] = "0123456789abcdef";
	for (size_t i = digits; i > 0; i--) {
		out[i - 1] = hex[value & 0xf];
		value >>= 4;
	}
}

/* A well formed uuid: `tag` repeated in the first group, `n` in the last. */
static void ru_format(char *out, char tag, unsigned n)
{
	memcpy(out, "00000000-0000-4000-8000-000000000000", RU_UUID);
	memset(out, tag, 8);
	ru_hex(out + 24, 12, n);
}

/* 'g' and the number handed out. */
static char *ru_generate(void)
{
	struct ru_fixture *f = ru_current;
	ru_check_locked();
	unsigned n = ++f->generated;
	ru_event('g');
	ru_event((char)('0' + (int)(n % 10)));
	char *uuid = bmalloc(RU_UUID);
	ru_format(uuid, 'b', n);
	return uuid;
}

void oracle_obs_reset_source_uuids(void);

/* The mingw build renames every pthread function on the command line
 * (w32-pthreads): keep that name for the rest of the file and use the
 * recorders only for the original and the adapters. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock ru_lock
#define pthread_mutex_unlock ru_unlock
#define obs ru_read_core()
#define bfree ru_bfree
#define os_generate_uuid ru_generate
#define obs_reset_source_uuids oracle_obs_reset_source_uuids
#include "libobs/obs-reset-source-uuids.c.h"
#undef obs_reset_source_uuids
#include "reset-source-uuids-fields.inc"
#undef os_generate_uuid
#undef bfree
#undef obs
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

static struct obs_context_data **ru_table(struct ru_fixture *f)
{
	return (struct obs_context_data **)&f->core.data.sources;
}

static bool ru_valid(const struct oracle_ru_case *c)
{
	if (c->count > RU_POOL || (c->count && !c->order) || c->resets < 1 || c->resets > 2)
		return false;
	bool seen[RU_POOL] = {false};
	for (size_t i = 0; i < c->count; i++) {
		if (c->order[i] >= RU_POOL || seen[c->order[i]])
			return false;
		seen[c->order[i]] = true;
	}
	return true;
}

static void ru_arm(struct ru_fixture *f, const struct oracle_ru_case *c)
{
	memset(f, 0, sizeof(*f));
	ru_current = f;
	for (size_t i = 0; i < c->count; i++) {
		size_t k = c->order[i];
		struct obs_context_data *ctx = &f->sources[k].context;
		char *uuid = bmalloc(RU_UUID);
		ru_format(uuid, 'a', (unsigned)k);
		memcpy(f->before[k], uuid, RU_UUID);
		ctx->uuid = uuid;
		f->listed[k] = true;
		HASH_ADD_UUID(*ru_table(f), uuid, ctx);
	}
}

static size_t ru_index(const struct ru_fixture *f, const void *ctx)
{
	for (size_t i = 0; i < RU_POOL; i++) {
		if (ctx == &f->sources[i].context)
			return i;
	}
	return RU_POOL;
}

/* '|', then each entry of the core's table by next link with its uuid
 * number ('?' when not generated here), the count, '=' when the table's
 * tail is its last entry, and per listed entry 'y' when its new uuid finds
 * it and 'o' when its old one finds nothing. */
static void ru_summary(struct ru_fixture *f)
{
	struct obs_context_data *head = *ru_table(f);
	struct obs_context_data *last = NULL;
	ru_event('|');
	for (struct obs_context_data *ctx = head; ctx; ctx = ctx->hh_uuid.next) {
		size_t i = ru_index(f, ctx);
		if (i == RU_POOL) {
			ru_event('?');
			break;
		}
		ru_event(ru_name(i));
		if (ctx->uuid && ctx->uuid[0] == 'b')
			ru_text(ctx->uuid + 30);
		else
			ru_event('?');
		last = ctx;
	}
	ru_event('#');
	ru_event((char)('0' + (int)HASH_CNT(hh_uuid, head)));
	if (head && head->hh_uuid.tbl->tail == &last->hh_uuid)
		ru_event('=');
	for (size_t i = 0; i < RU_POOL; i++) {
		if (!f->listed[i])
			continue;
		struct obs_context_data *found = NULL;
		const char *uuid = f->sources[i].context.uuid;
		if (uuid)
			HASH_FIND_UUID(head, uuid, found);
		ru_event(found == &f->sources[i].context ? 'y' : 'n');
		found = NULL;
		HASH_FIND_UUID(head, f->before[i], found);
		ru_event(found ? 'n' : 'o');
	}
}

/* Frees the tables and uuids. Every listed entry must be in a live table:
 * the core's, or (for a mutant that stops early) the working one, whose
 * head is the entry with no previous link. */
static void ru_disarm(struct ru_fixture *f)
{
	struct obs_context_data *core = *ru_table(f);
	struct obs_context_data *rest = NULL;
	for (size_t i = 0; i < RU_POOL; i++) {
		if (!f->listed[i])
			continue;
		struct obs_context_data *ctx = &f->sources[i].context;
		struct obs_context_data *found = NULL;
		if (ctx->uuid)
			HASH_FIND_UUID(core, ctx->uuid, found);
		if (found != ctx && !ctx->hh_uuid.prev)
			rest = ctx;
	}
	HASH_CLEAR(hh_uuid, rest);
	HASH_CLEAR(hh_uuid, core);
	*ru_table(f) = NULL;
	for (size_t i = 0; i < RU_POOL; i++) {
		if (f->listed[i])
			bfree((void *)(uintptr_t)f->sources[i].context.uuid);
	}
	ru_current = NULL;
}

static void ru_run(ru_fn fn, const struct oracle_ru_case *c, struct ru_fixture *f)
{
	ru_arm(f, c);
	for (uint8_t i = 0; i < c->resets; i++) {
		if (i)
			ru_event('/');
		fn();
	}
	ru_summary(f);
}

/* 0: match. 1: the traces differ, 4: the candidate (or the original) left
 * the mutex held. 0x100: the case does not fit the fixture, 0x200: out of
 * memory. */
unsigned oracle_reset_source_uuids_probe(ru_fn candidate, const struct oracle_ru_case *c);
unsigned oracle_reset_source_uuids_probe(ru_fn candidate, const struct oracle_ru_case *c)
{
	if (!ru_valid(c))
		return 0x100;
	struct ru_fixture *original = malloc(sizeof(*original));
	struct ru_fixture *f = malloc(sizeof(*f));
	if (!f || !original) {
		free(original);
		free(f);
		return 0x200;
	}
	ru_run(oracle_obs_reset_source_uuids, c, original);
	ru_disarm(original);
	ru_run(candidate, c, f);
	ru_disarm(f);
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
static void ru_copy(char *out, size_t size, const char *text)
{
	if (!size)
		return;
	size_t n = strlen(text);
	if (n >= size)
		n = size - 1;
	memcpy(out, text, n);
	out[n] = '\0';
}

/* The original's trace, for the characterization checks; false when it left
 * the mutex held or the case does not fit. */
bool oracle_reset_source_uuids_original(const struct oracle_ru_case *c, char *trace, size_t trace_size);
bool oracle_reset_source_uuids_original(const struct oracle_ru_case *c, char *trace, size_t trace_size)
{
	if (!ru_valid(c))
		return false;
	struct ru_fixture *f = malloc(sizeof(*f));
	if (!f)
		return false;
	ru_run(oracle_obs_reset_source_uuids, c, f);
	ru_disarm(f);
	ru_copy(trace, trace_size, f->trace);
	bool released = !f->depth;
	free(f);
	return released;
}

/* The adapters against the real fields, one step each. */
bool oracle_reset_source_uuids_adapters(void);
bool oracle_reset_source_uuids_adapters(void)
{
	static const uint8_t order[] = {2, 0, 1};
	const struct oracle_ru_case c = {order, 3, 1};
	struct ru_fixture *f = malloc(sizeof(*f));
	if (!f)
		return false;
	ru_arm(f, &c);
	struct obs_context_data *a = &f->sources[0].context, *b = &f->sources[1].context, *s = &f->sources[2].context;
	bool ok = true;

	obs_reset_source_uuids_lock();
	ok = ok && f->depth == 1;
	struct obs_context_data *ht = obs_reset_source_uuids_head();
	ok = ok && ht == s && obs_reset_source_uuids_next(s) == a && obs_reset_source_uuids_next(a) == b &&
	     obs_reset_source_uuids_next(b) == NULL;

	/* Unlinking the head moves the working head on and leaves the core's
	 * head and the entry's own next link as they were. */
	obs_reset_source_uuids_delete(&ht, s);
	ok = ok && ht == a && *ru_table(f) == s && obs_reset_source_uuids_next(s) == a && HASH_CNT(hh_uuid, ht) == 2;

	obs_reset_source_uuids_free_uuid(s);
	obs_reset_source_uuids_new_uuid(s);
	ok = ok && strcmp(s->uuid, "bbbbbbbb-0000-4000-8000-000000000001") == 0;

	struct obs_context_data *new_ht = NULL;
	obs_reset_source_uuids_add(&new_ht, s);
	struct obs_context_data *found = NULL;
	HASH_FIND_UUID(new_ht, s->uuid, found);
	ok = ok && new_ht == s && found == s && obs_reset_source_uuids_next(s) == NULL &&
	     HASH_CNT(hh_uuid, new_ht) == 1;

	/* A second add appends at the tail. */
	obs_reset_source_uuids_delete(&ht, a);
	obs_reset_source_uuids_free_uuid(a);
	obs_reset_source_uuids_new_uuid(a);
	obs_reset_source_uuids_add(&new_ht, a);
	ok = ok && new_ht == s && obs_reset_source_uuids_next(s) == a && ht == b;

	obs_reset_source_uuids_store(NULL);
	ok = ok && *ru_table(f) == NULL;
	obs_reset_source_uuids_store(new_ht);
	ok = ok && *ru_table(f) == s;
	obs_reset_source_uuids_unlock();
	ok = ok && f->depth == 0;

	ok = ok && strcmp(f->trace, "Lfcg1fag2U") == 0;

	/* b is still alone in the working table. */
	obs_reset_source_uuids_lock();
	obs_reset_source_uuids_delete(&ht, b);
	obs_reset_source_uuids_unlock();
	ok = ok && ht == NULL;
	ok = ok && strcmp(f->trace, "Lfcg1fag2ULTTU") == 0;
	obs_reset_source_uuids_lock();
	obs_reset_source_uuids_add(&new_ht, b);
	obs_reset_source_uuids_unlock();

	ru_disarm(f);
	free(f);
	return ok;
}
