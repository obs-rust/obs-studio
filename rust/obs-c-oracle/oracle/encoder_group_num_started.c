/* Original obs_encoder_group_num_started() and the real-header adapters of
 * obs-encoder-group-num-started-fields.c.h on two recording fixture groups.
 * Under cargo test these adapters are the only definitions of their real
 * names, so the Rust shim reaches this fixture. Every lock and unlock of the
 * group mutex is recorded; either can rewrite the count, as the unchanged
 * test_encoder_group_num_started.c does, so a read outside the pair, a
 * second read or a value taken after the unlock shows. The probe runs the
 * original and a candidate on the same case and reports any difference in
 * the result, the trace or the groups left behind, instead of aborting, so
 * mutants are observable.
 *
 * The interleave parity fixture (rust/obs-output/tests/output/
 * interleave_parity.rs) has groups of its own that the same shim reaches:
 * it installs per-thread handlers, and the adapters forward every group that
 * is not one of these two to them. One definition of each name serves both
 * (the scene_enum_items.c shape). */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

#define EGNS_TRACE 64

/* Case flags: rewrite the count in the lock, in the unlock, and query the
 * second group instead of the first. */
#define EGNS_CHANGE_ON_LOCK 0x01
#define EGNS_CHANGE_ON_UNLOCK 0x02
#define EGNS_SECOND 0x04

typedef uint32_t (*egns_query_fn)(struct obs_encoder_group *group);
typedef void (*egns_foreign_fn)(void *group);
typedef uint32_t (*egns_foreign_read_fn)(const void *group);

static _Thread_local struct obs_encoder_group egns_groups[2];
static _Thread_local struct obs_encoder_group *egns_selected;
static _Thread_local bool egns_locked, egns_change_on_lock, egns_change_on_unlock;
static _Thread_local uint32_t egns_locked_value, egns_unlocked_value;
static _Thread_local char egns_trace[EGNS_TRACE];
static _Thread_local size_t egns_trace_len;
static _Thread_local egns_foreign_fn egns_foreign_lock, egns_foreign_unlock;
static _Thread_local egns_foreign_read_fn egns_foreign_read;

static void egns_event(char value)
{
	if (egns_trace_len + 1 < sizeof(egns_trace)) {
		egns_trace[egns_trace_len++] = value;
		egns_trace[egns_trace_len] = '\0';
	}
}

static bool egns_selected_mutex(const pthread_mutex_t *mutex)
{
	return egns_selected && mutex == &egns_selected->mutex;
}

/* 'L' and 'U'; '?' for another mutex, 'R' for a lock already held, 'X' for
 * an unlock not held. */
static int egns_lock(pthread_mutex_t *mutex)
{
	egns_event('L');
	if (!egns_selected_mutex(mutex))
		egns_event('?');
	if (egns_locked)
		egns_event('R');
	egns_locked = true;
	if (egns_change_on_lock && egns_selected)
		egns_selected->num_encoders_started = egns_locked_value;
	return 0;
}

static int egns_unlock(pthread_mutex_t *mutex)
{
	egns_event('U');
	if (!egns_selected_mutex(mutex))
		egns_event('?');
	if (!egns_locked)
		egns_event('X');
	if (egns_change_on_unlock && egns_selected)
		egns_selected->num_encoders_started = egns_unlocked_value;
	egns_locked = false;
	return 0;
}

uint32_t oracle_obs_encoder_group_num_started(struct obs_encoder_group *group);

/* winpthreads defines the lock functions as macros: rename them to the
 * recorders only for the original and the adapters. The adapters keep
 * private names here; the real names below dispatch to them. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock egns_lock
#define pthread_mutex_unlock egns_unlock
#define obs_encoder_group_num_started oracle_obs_encoder_group_num_started
#include "libobs/obs-encoder-group-num-started.c.h"
#undef obs_encoder_group_num_started
#define obs_encoder_group_num_started_lock egns_adapter_lock
#define obs_encoder_group_num_started_read egns_adapter_read
#define obs_encoder_group_num_started_unlock egns_adapter_unlock
#include "encoder_group_num_started_fields.inc"
#undef obs_encoder_group_num_started_unlock
#undef obs_encoder_group_num_started_read
#undef obs_encoder_group_num_started_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

static bool egns_own(const struct obs_encoder_group *group)
{
	return group == &egns_groups[0] || group == &egns_groups[1];
}

/* Another fixture's groups on this thread go to its handlers; NULL clears
 * them. */
void oracle_encoder_group_num_started_foreign(egns_foreign_fn lock, egns_foreign_read_fn read, egns_foreign_fn unlock);
void oracle_encoder_group_num_started_foreign(egns_foreign_fn lock, egns_foreign_read_fn read, egns_foreign_fn unlock)
{
	egns_foreign_lock = lock;
	egns_foreign_read = read;
	egns_foreign_unlock = unlock;
}

void obs_encoder_group_num_started_lock(struct obs_encoder_group *group);
void obs_encoder_group_num_started_lock(struct obs_encoder_group *group)
{
	if (!egns_own(group) && egns_foreign_lock)
		egns_foreign_lock(group);
	else
		egns_adapter_lock(group);
}

uint32_t obs_encoder_group_num_started_read(const struct obs_encoder_group *group);
uint32_t obs_encoder_group_num_started_read(const struct obs_encoder_group *group)
{
	if (!egns_own(group) && egns_foreign_read)
		return egns_foreign_read(group);
	return egns_adapter_read(group);
}

void obs_encoder_group_num_started_unlock(struct obs_encoder_group *group);
void obs_encoder_group_num_started_unlock(struct obs_encoder_group *group)
{
	if (!egns_own(group) && egns_foreign_unlock)
		egns_foreign_unlock(group);
	else
		egns_adapter_unlock(group);
}

struct egns_case {
	uint32_t first;
	uint32_t second;
	uint32_t on_lock;
	uint32_t on_unlock;
	uint8_t flags;
};

struct egns_result {
	uint32_t value;
	char trace[EGNS_TRACE];
	struct obs_encoder_group groups[2];
};

/* The test's groups: both counts, and unrelated lifecycle state that must
 * neither clamp nor gate the count. */
static void egns_reset(const struct egns_case *c)
{
	memset(egns_groups, 0, sizeof(egns_groups));
	egns_groups[0].num_encoders_started = c->first;
	egns_groups[1].num_encoders_started = c->second;
	egns_groups[0].destroy_on_stop = true;
	egns_groups[1].frame_rate_divisors_lcm = UINT32_MAX;
	egns_selected = &egns_groups[(c->flags & EGNS_SECOND) ? 1 : 0];
	egns_change_on_lock = (c->flags & EGNS_CHANGE_ON_LOCK) != 0;
	egns_change_on_unlock = (c->flags & EGNS_CHANGE_ON_UNLOCK) != 0;
	egns_locked_value = c->on_lock;
	egns_unlocked_value = c->on_unlock;
	egns_locked = false;
	egns_trace_len = 0;
	egns_trace[0] = '\0';
}

/* The trace ends with 'K' if the call left the mutex held. */
static void egns_run(egns_query_fn query, const struct egns_case *c, struct egns_result *out)
{
	egns_reset(c);
	out->value = query(egns_selected);
	if (egns_locked)
		egns_event('K');
	memcpy(out->trace, egns_trace, sizeof(out->trace));
	memcpy(out->groups, egns_groups, sizeof(out->groups));
	egns_selected = NULL;
	egns_locked = false;
}

/* 0: match. 1: the results differ, 2: the traces differ, 4: the groups left
 * differ, 0x100: bad arguments. */
unsigned oracle_encoder_group_num_started_probe(egns_query_fn query, uint32_t first, uint32_t second, uint32_t on_lock,
						uint32_t on_unlock, uint8_t flags);
unsigned oracle_encoder_group_num_started_probe(egns_query_fn query, uint32_t first, uint32_t second, uint32_t on_lock,
						uint32_t on_unlock, uint8_t flags)
{
	static _Thread_local struct egns_result original, actual;
	if (!query)
		return 0x100;
	const struct egns_case c = {first, second, on_lock, on_unlock, flags};
	egns_run(oracle_obs_encoder_group_num_started, &c, &original);
	egns_run(query, &c, &actual);
	unsigned diff = 0;
	if (original.value != actual.value)
		diff |= 1;
	if (strcmp(original.trace, actual.trace) != 0)
		diff |= 2;
	if (memcmp(original.groups, actual.groups, sizeof(original.groups)) != 0)
		diff |= 4;
	return diff;
}

/* The original's result, its trace and the queried group's count after the
 * call, for the characterization checks. */
uint32_t oracle_encoder_group_num_started_original(uint32_t first, uint32_t second, uint32_t on_lock,
						   uint32_t on_unlock, uint8_t flags, char *trace, size_t trace_size,
						   uint32_t *after);
uint32_t oracle_encoder_group_num_started_original(uint32_t first, uint32_t second, uint32_t on_lock,
						   uint32_t on_unlock, uint8_t flags, char *trace, size_t trace_size,
						   uint32_t *after)
{
	static _Thread_local struct egns_result result;
	const struct egns_case c = {first, second, on_lock, on_unlock, flags};
	egns_run(oracle_obs_encoder_group_num_started, &c, &result);
	if (trace_size) {
		size_t n = strlen(result.trace);
		if (n >= trace_size)
			n = trace_size - 1;
		memcpy(trace, result.trace, n);
		trace[n] = '\0';
	}
	if (after)
		*after = result.groups[(flags & EGNS_SECOND) ? 1 : 0].num_encoders_started;
	return result.value;
}

/* Where the real header puts the fields the adapters touch. */
void oracle_encoder_group_num_started_layout(size_t *mutex, size_t *count, size_t *count_size);
void oracle_encoder_group_num_started_layout(size_t *mutex, size_t *count, size_t *count_size)
{
	*mutex = offsetof(struct obs_encoder_group, mutex);
	*count = offsetof(struct obs_encoder_group, num_encoders_started);
	*count_size = sizeof(((struct obs_encoder_group *)0)->num_encoders_started);
}

/* The adapters against the real fields: the lock and unlock on the group's
 * own mutex, and a read of exactly num_encoders_started, which changes
 * nothing. */
bool oracle_encoder_group_num_started_adapters(void);
bool oracle_encoder_group_num_started_adapters(void)
{
	const struct egns_case c = {0x12345678u, 0x9abcdef0u, 0, 0, EGNS_SECOND};
	egns_reset(&c);
	unsigned char before[sizeof(egns_groups)];
	memcpy(before, egns_groups, sizeof(egns_groups));
	struct obs_encoder_group *group = &egns_groups[1];
	obs_encoder_group_num_started_lock(group);
	bool ok = egns_locked && obs_encoder_group_num_started_read(group) == 0x9abcdef0u &&
		  obs_encoder_group_num_started_read(&egns_groups[0]) == 0x12345678u;
	obs_encoder_group_num_started_unlock(group);
	ok = ok && !egns_locked && strcmp(egns_trace, "LU") == 0;
	ok = ok && memcmp(before, egns_groups, sizeof(egns_groups)) == 0;
	egns_selected = NULL;
	return ok;
}
