/* Original obs_enum_outputs(), obs_enum_encoders() and obs_enum_services()
 * and the real-header adapters of obs-enum-contexts-fields.c.h on one
 * recording fixture core. Under cargo test these adapters are the only
 * definitions of their real names, so the Rust shim reaches this fixture.
 *
 * The fixture core holds the same list for each of the three kinds, each
 * with its own re-entrant list mutex and its own pool of contexts carrying a
 * reference count. The kind's get_ref takes a reference unless the count is
 * zero (a dying entry); its release drops one, and the last one destroys the
 * context, which unlinks it under the kind's mutex as obs_*_destroy does.
 * The callback can stop the walk, enumerate a kind again, create a context
 * (inserted at the head under the mutex) or release the owner's reference
 * to the current entry. Traced: every lock and unlock with its kind, every
 * get_ref with its answer, every callback, every walk release and destroy,
 * and the callback's own actions; a head or next read, get_ref or callback
 * outside the kind's lock, a callback handed a destroyed context or another
 * param, and an unlock without a hold are marked. The probe runs the
 * original and a candidate on the same fixture and reports, instead of
 * aborting, any difference in the trace or in the final contexts, links,
 * heads and counts, or a mutex left held, so mutants are observable.
 *
 * Excluded: an uninitialized core, a NULL callback, a dead listed context and
 * a list change from another thread (outside the original's contract; the
 * real lifecycle stays unchanged Tier 2, test_obs_enum_contexts and
 * test_obs_enum_outputs_removal). No intentional differences. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

#define EC_KINDS 3
#define EC_POOL 12
#define EC_LIST 8
#define EC_TRACE 1024
#define EC_NEVER 0xff

/* One case, as the Rust side describes it: `count` contexts listed head
 * first in every kind, with `refs[i]` initial references each (0: dying).
 * The walk enumerates `kind`; its callback answers false at visit
 * `stop_after`, releases the owner's reference of the current entry at visit
 * `release_at`, enumerates `nested_kind` at visit `nested_at` and creates a
 * context of `kind` at visit `create_at` (EC_NEVER: never). */
struct oracle_ec_case {
	const uint8_t *refs;
	size_t count;
	uint8_t kind;
	uint8_t stop_after;
	uint8_t release_at;
	uint8_t nested_at;
	uint8_t nested_kind;
	uint8_t create_at;
};

typedef bool (*ec_proc)(void *, void *);
typedef void (*ec_fn)(ec_proc, void *);

/* The final state the probe compares. */
struct ec_state {
	struct obs_context_data pool[EC_KINDS][EC_POOL];
	struct obs_context_data *heads[EC_KINDS];
	long refs[EC_KINDS][EC_POOL];
	bool destroyed[EC_KINDS][EC_POOL];
	size_t used[EC_KINDS];
};

static _Thread_local struct obs_core ec_core;
static _Thread_local struct ec_state ec;
static _Thread_local unsigned ec_depth[EC_KINDS];
static _Thread_local const struct oracle_ec_case *ec_case;
static _Thread_local const ec_fn *ec_candidates;
/* The outer walk's param, then the nested walk's. */
static _Thread_local char ec_params[2];
static _Thread_local size_t ec_visits;
static _Thread_local char ec_trace[EC_TRACE];
static _Thread_local size_t ec_trace_len;

static void ec_event(char value)
{
	if (ec_trace_len + 1 < sizeof(ec_trace)) {
		ec_trace[ec_trace_len++] = value;
		ec_trace[ec_trace_len] = '\0';
	}
}

static void ec_event3(char what, size_t kind, size_t index)
{
	ec_event(what);
	ec_event((char)('0' + kind));
	ec_event((char)('a' + index));
}

static struct obs_core *ec_read_core(void)
{
	return &ec_core;
}

static pthread_mutex_t *ec_mutex(size_t kind)
{
	switch (kind) {
	case 0:
		return &ec_core.data.outputs_mutex;
	case 1:
		return &ec_core.data.encoders_mutex;
	default:
		return &ec_core.data.services_mutex;
	}
}

/* The core's list head of the kind, as the original's obs_enum() sees it. */
static struct obs_context_data **ec_head(size_t kind)
{
	switch (kind) {
	case 0:
		return (struct obs_context_data **)&ec_core.data.first_output;
	case 1:
		return (struct obs_context_data **)&ec_core.data.first_encoder;
	default:
		return (struct obs_context_data **)&ec_core.data.first_service;
	}
}

static size_t ec_mutex_kind(const pthread_mutex_t *mutex)
{
	for (size_t kind = 0; kind < EC_KINDS; kind++) {
		if (mutex == ec_mutex(kind))
			return kind;
	}
	return EC_KINDS;
}

/* 'L' and the kind locks; 'R' marks another mutex. */
static int ec_lock(pthread_mutex_t *mutex)
{
	size_t kind = ec_mutex_kind(mutex);
	if (kind == EC_KINDS) {
		ec_event('R');
		return pthread_mutex_lock(mutex);
	}
	ec_event('L');
	ec_event((char)('0' + kind));
	int result = pthread_mutex_lock(mutex);
	ec_depth[kind]++;
	return result;
}

/* 'U' and the kind unlocks; 'R' marks an unlock without a hold or of
 * another mutex. */
static int ec_unlock(pthread_mutex_t *mutex)
{
	size_t kind = ec_mutex_kind(mutex);
	if (kind == EC_KINDS || !ec_depth[kind]) {
		ec_event('R');
		return 0;
	}
	ec_event('U');
	ec_event((char)('0' + kind));
	ec_depth[kind]--;
	return pthread_mutex_unlock(mutex);
}

/* 'X' marks a list read, get_ref or callback outside the kind's lock. */
static void ec_check_locked(size_t kind)
{
	if (kind >= EC_KINDS || !ec_depth[kind])
		ec_event('X');
}

/* The kind and index of a pool context; EC_KINDS for none. */
static size_t ec_find(const struct obs_context_data *context, size_t *index)
{
	for (size_t kind = 0; kind < EC_KINDS; kind++) {
		if (context >= &ec.pool[kind][0] && context < &ec.pool[kind][EC_POOL]) {
			*index = (size_t)(context - &ec.pool[kind][0]);
			return kind;
		}
	}
	*index = 0;
	return EC_KINDS;
}

/* obs_*_destroy: 'D', then unlinks the context under the kind's mutex. */
static void ec_destroy(size_t kind, size_t index)
{
	struct obs_context_data *context = &ec.pool[kind][index];
	ec_event3('D', kind, index);
	ec_lock(ec_mutex(kind));
	struct obs_context_data **link = ec_head(kind);
	while (*link && *link != context)
		link = &(*link)->next;
	if (*link)
		*link = context->next;
	context->next = NULL;
	ec.destroyed[kind][index] = true;
	ec_unlock(ec_mutex(kind));
}

static void ec_drop(size_t kind, size_t index)
{
	if (ec.refs[kind][index] > 0 && --ec.refs[kind][index] == 0)
		ec_destroy(kind, index);
}

/* 'g', the context, then '+' for a reference or '-' for a dying entry; '?'
 * marks a context that is not the kind's. */
static struct obs_context_data *ec_get_ref(size_t want, struct obs_context_data *context)
{
	size_t index;
	size_t kind = ec_find(context, &index);
	ec_check_locked(kind);
	if (kind != want) {
		ec_event('?');
		return NULL;
	}
	ec_event3('g', kind, index);
	if (ec.destroyed[kind][index] || ec.refs[kind][index] == 0) {
		ec_event('-');
		return NULL;
	}
	ec.refs[kind][index]++;
	ec_event('+');
	return context;
}

/* 'r' and the context released by the walk. */
static void ec_release(size_t want, struct obs_context_data *context)
{
	size_t index;
	size_t kind = ec_find(context, &index);
	if (kind != want) {
		ec_event('?');
		return;
	}
	ec_event3('r', kind, index);
	ec_drop(kind, index);
}

static obs_output_t *ec_output_get_ref(obs_output_t *output)
{
	return (obs_output_t *)ec_get_ref(0, (struct obs_context_data *)output);
}

static obs_encoder_t *ec_encoder_get_ref(obs_encoder_t *encoder)
{
	return (obs_encoder_t *)ec_get_ref(1, (struct obs_context_data *)encoder);
}

static obs_service_t *ec_service_get_ref(obs_service_t *service)
{
	return (obs_service_t *)ec_get_ref(2, (struct obs_context_data *)service);
}

static void ec_output_release(obs_output_t *output)
{
	ec_release(0, (struct obs_context_data *)output);
}

static void ec_encoder_release(obs_encoder_t *encoder)
{
	ec_release(1, (struct obs_context_data *)encoder);
}

static void ec_service_release(obs_service_t *service)
{
	ec_release(2, (struct obs_context_data *)service);
}

void oracle_obs_enum_outputs(bool (*enum_proc)(void *, obs_output_t *), void *param);
void oracle_obs_enum_encoders(bool (*enum_proc)(void *, obs_encoder_t *), void *param);
void oracle_obs_enum_services(bool (*enum_proc)(void *, obs_service_t *), void *param);

/* The mingw build renames every pthread function on the command line
 * (w32-pthreads): keep that name for the rest of the file and use the
 * recorders only for the original and the adapters. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock ec_lock
#define pthread_mutex_unlock ec_unlock
#define obs ec_read_core()
#define obs_output_get_ref ec_output_get_ref
#define obs_encoder_get_ref ec_encoder_get_ref
#define obs_service_get_ref ec_service_get_ref
#define obs_output_release ec_output_release
#define obs_encoder_release ec_encoder_release
#define obs_service_release ec_service_release
#define obs_enum_outputs oracle_obs_enum_outputs
#define obs_enum_encoders oracle_obs_enum_encoders
#define obs_enum_services oracle_obs_enum_services
#include "libobs/obs-enum-contexts.c.h"
#undef obs_enum_services
#undef obs_enum_encoders
#undef obs_enum_outputs
/* The list read adapters are wrapped below to check the lock. */
#define obs_enum_contexts_first ec_inner_first
#define obs_enum_contexts_next ec_inner_next
#include "enum-contexts-fields.inc"
#undef obs_enum_contexts_next
#undef obs_enum_contexts_first
#undef obs_service_release
#undef obs_encoder_release
#undef obs_output_release
#undef obs_service_get_ref
#undef obs_encoder_get_ref
#undef obs_output_get_ref
#undef obs
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

struct obs_context_data *obs_enum_contexts_first(uint32_t kind);
struct obs_context_data *obs_enum_contexts_first(uint32_t kind)
{
	ec_check_locked(kind);
	return ec_inner_first(kind);
}

struct obs_context_data *obs_enum_contexts_next(const struct obs_context_data *context);
struct obs_context_data *obs_enum_contexts_next(const struct obs_context_data *context)
{
	size_t index;
	ec_check_locked(ec_find(context, &index));
	return ec_inner_next(context);
}

/* 'c' and the context handed to the callback; 'Z' marks a destroyed one and
 * 'P' another param. The outer walk acts on its visits; 'N' ... 'n'
 * brackets a nested walk, 'O' the owner's release of the current entry and
 * 'C' a context created at the head. */
static bool ec_visit(void *param, void *object)
{
	struct obs_context_data *context = object;
	size_t index;
	size_t kind = ec_find(context, &index);
	ec_check_locked(kind);
	if (kind == EC_KINDS) {
		ec_event('?');
		return true;
	}
	ec_event3('c', kind, index);
	if (ec.destroyed[kind][index])
		ec_event('Z');
	if (param == &ec_params[1])
		return true;
	if (param != &ec_params[0]) {
		ec_event('P');
		return true;
	}

	const struct oracle_ec_case *c = ec_case;
	size_t visit = ec_visits++;
	if (visit == c->nested_at) {
		ec_event('N');
		ec_candidates[c->nested_kind % EC_KINDS](ec_visit, &ec_params[1]);
		ec_event('n');
	}
	if (visit == c->create_at && ec.used[c->kind] < EC_POOL) {
		size_t created = ec.used[c->kind]++;
		struct obs_context_data *fresh = &ec.pool[c->kind][created];
		ec_event3('C', c->kind, created);
		ec_lock(ec_mutex(c->kind));
		fresh->next = *ec_head(c->kind);
		*ec_head(c->kind) = fresh;
		ec.refs[c->kind][created] = 1;
		ec_unlock(ec_mutex(c->kind));
	}
	if (visit == c->release_at) {
		ec_event3('O', kind, index);
		ec_drop(kind, index);
	}
	return visit != c->stop_after;
}

static void ec_arm(const struct oracle_ec_case *c)
{
	memset(&ec_core, 0, sizeof(ec_core));
	/* Every byte around the links is filled, so a write anywhere shows. */
	memset(&ec, 0, sizeof(ec));
	memset(ec.pool, 0x5a, sizeof(ec.pool));
	for (size_t kind = 0; kind < EC_KINDS; kind++) {
		pthread_mutex_init_recursive(ec_mutex(kind));
		ec_depth[kind] = 0;
		*ec_head(kind) = c->count ? &ec.pool[kind][0] : NULL;
		for (size_t i = 0; i < EC_POOL; i++)
			ec.pool[kind][i].next = NULL;
		for (size_t i = 0; i < c->count; i++) {
			ec.pool[kind][i].next = i + 1 < c->count ? &ec.pool[kind][i + 1] : NULL;
			ec.refs[kind][i] = c->refs[i];
		}
		ec.used[kind] = c->count;
	}
	ec_case = c;
	ec_visits = 0;
	ec_trace_len = 0;
	ec_trace[0] = '\0';
}

/* The core's heads, for the final state. */
static void ec_sync_heads_in(void)
{
	for (size_t kind = 0; kind < EC_KINDS; kind++)
		ec.heads[kind] = *ec_head(kind);
}

static void ec_disarm(void)
{
	for (size_t kind = 0; kind < EC_KINDS; kind++)
		pthread_mutex_destroy(ec_mutex(kind));
	ec_case = NULL;
	ec_candidates = NULL;
}

struct ec_result {
	char trace[EC_TRACE];
	struct ec_state state;
	bool released;
};

static void ec_run(const ec_fn candidates[EC_KINDS], const struct oracle_ec_case *c, struct ec_result *out)
{
	ec_arm(c);
	ec_candidates = candidates;
	candidates[c->kind % EC_KINDS](ec_visit, &ec_params[0]);
	ec_sync_heads_in();
	memcpy(out->trace, ec_trace, sizeof(out->trace));
	memcpy(&out->state, &ec, sizeof(ec));
	out->released = !ec_depth[0] && !ec_depth[1] && !ec_depth[2];
	ec_disarm();
}

static const ec_fn ec_originals[EC_KINDS] = {
	(ec_fn)oracle_obs_enum_outputs,
	(ec_fn)oracle_obs_enum_encoders,
	(ec_fn)oracle_obs_enum_services,
};

static bool ec_valid(const struct oracle_ec_case *c)
{
	return c->count <= EC_LIST && (c->count == 0 || c->refs);
}

/* 0: match. 1: the traces differ, 2: the final contexts, links, heads or
 * counts differ, 4: the candidate (or the original) left a mutex held.
 * 0x100: the case does not fit the fixture. */
unsigned oracle_enum_contexts_probe(const ec_fn candidates[EC_KINDS], const struct oracle_ec_case *c);
unsigned oracle_enum_contexts_probe(const ec_fn candidates[EC_KINDS], const struct oracle_ec_case *c)
{
	static _Thread_local struct ec_result original, actual;
	if (!ec_valid(c))
		return 0x100;
	ec_run(ec_originals, c, &original);
	ec_run(candidates, c, &actual);
	unsigned diff = 0;
	if (strcmp(original.trace, actual.trace) != 0)
		diff |= 1;
	if (memcmp(&original.state, &actual.state, sizeof(original.state)) != 0)
		diff |= 2;
	if (!original.released || !actual.released)
		diff |= 4;
	return diff;
}

/* No stdio here: an oracle member defining the UCRT stdio inlines can be
 * pulled into unrelated Windows test links. */
static void ec_copy(char *out, size_t size, const char *text)
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
 * a mutex held or the case does not fit. */
bool oracle_enum_contexts_original(const struct oracle_ec_case *c, char *trace, size_t trace_size);
bool oracle_enum_contexts_original(const struct oracle_ec_case *c, char *trace, size_t trace_size)
{
	static _Thread_local struct ec_result result;
	if (!ec_valid(c))
		return false;
	ec_run(ec_originals, c, &result);
	ec_copy(trace, trace_size, result.trace);
	return result.released;
}

/* The adapters against the real fields, for each kind. */
bool oracle_enum_contexts_adapters(void);
bool oracle_enum_contexts_adapters(void)
{
	static const uint8_t refs[] = {1, 0, 2};
	struct oracle_ec_case c = {refs, 3, 0, EC_NEVER, EC_NEVER, EC_NEVER, 0, EC_NEVER};
	bool ok = true;
	for (uint32_t kind = 0; kind < EC_KINDS; kind++) {
		ec_arm(&c);
		struct obs_context_data *pool = ec.pool[kind];
		obs_enum_contexts_lock(kind);
		ok = ok && ec_depth[kind] == 1 && obs_enum_contexts_first(kind) == &pool[0] &&
		     obs_enum_contexts_next(&pool[0]) == &pool[1] && obs_enum_contexts_next(&pool[1]) == &pool[2] &&
		     obs_enum_contexts_next(&pool[2]) == NULL &&
		     obs_enum_contexts_get_ref(kind, &pool[0]) == &pool[0] &&
		     obs_enum_contexts_get_ref(kind, &pool[1]) == NULL && ec.refs[kind][0] == 2;
		obs_enum_contexts_unlock(kind);
		obs_enum_contexts_release(kind, &pool[0]);
		ok = ok && ec_depth[kind] == 0 && ec.refs[kind][0] == 1;
		char expected[] = "L?g?a+g?b-U?r?a";
		for (size_t i = 0; i < sizeof(expected); i++) {
			if (expected[i] == '?')
				expected[i] = (char)('0' + kind);
		}
		ok = ok && strcmp(ec_trace, expected) == 0;
		ec_disarm();
	}
	/* An empty list. */
	c.count = 0;
	ec_arm(&c);
	for (uint32_t kind = 0; kind < EC_KINDS; kind++) {
		obs_enum_contexts_lock(kind);
		ok = ok && obs_enum_contexts_first(kind) == NULL;
		obs_enum_contexts_unlock(kind);
	}
	ok = ok && strcmp(ec_trace, "L0U0L1U1L2U2") == 0;
	ec_disarm();
	return ok;
}
