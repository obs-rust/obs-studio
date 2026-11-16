/* Original object-ref bodies and weak helpers, all over the real C layouts.
 * Excludes overflow and release without ownership/control (undefined in C). */
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include "obs-internal.h"

static struct obs_core *object_ref_core;
void oracle_core_initialized_set(void *core);
void oracle_object_ref_weak_release(obs_weak_object_t *weak);
obs_object_t *oracle_object_ref_weak_get(obs_weak_object_t *weak);
obs_object_t *oracle_obs_object_get_ref(obs_object_t *object);
void oracle_obs_object_release(obs_object_t *object);

#define obs object_ref_core
#define obs_weak_object_release oracle_object_ref_weak_release
#define obs_weak_object_get_object oracle_object_ref_weak_get
#define obs_object_get_ref oracle_obs_object_get_ref
#define obs_object_release oracle_obs_object_release
#include "object-ref-weak.inc"
#include "obs-object-ref.c.h"
#undef obs_object_release
#undef obs_object_get_ref
#undef obs_weak_object_get_object
#undef obs_weak_object_release
#undef obs

#include "object-ref-fields.inc"

struct object_ref_observation {
	unsigned destroys;
	bool identity;
	long refs_at_destroy;
	long weak_at_destroy;
	unsigned warnings;
	bool exact_warning;
};
struct object_ref_fixture {
	struct obs_context_data context;
	struct object_ref_observation *observation;
};
static void object_ref_destroy(void *object)
{
	struct object_ref_fixture *fixture = object;
	struct object_ref_observation *o = fixture->observation;
	o->destroys++;
	o->identity = object == &fixture->context;
	o->refs_at_destroy = os_atomic_load_long(&fixture->context.control->ref.refs);
	o->weak_at_destroy = os_atomic_load_long(&fixture->context.control->ref.weak_refs);
	/* Destroy may invalidate the context. The caller must retain the original
	 * control across the callback and never access context afterwards. */
	fixture->context.control = NULL;
}

#if defined(__GNUC__) || defined(__clang__)
static void object_ref_warning(int level, const char *message, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void object_ref_warning(int level, const char *message, va_list args, void *param)
{
	(void)args;
	struct object_ref_observation *o = param;
	o->warnings++;
	o->exact_warning = o->exact_warning && level == LOG_WARNING &&
			   strcmp(message, "Tried to release an object when the OBS core is shut down!") == 0;
}

void oracle_object_ref_layout(size_t *layout);
void oracle_object_ref_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_weak_ref);
	layout[1] = _Alignof(struct obs_weak_ref);
	layout[2] = offsetof(struct obs_weak_ref, refs);
	layout[3] = offsetof(struct obs_weak_ref, weak_refs);
	layout[4] = sizeof(struct obs_weak_object);
	layout[5] = _Alignof(struct obs_weak_object);
	layout[6] = offsetof(struct obs_weak_object, ref);
	layout[7] = offsetof(struct obs_weak_object, object);
}

typedef obs_object_t *(*object_ref_get)(obs_object_t *);
typedef void (*object_ref_release)(obs_object_t *);

/* flags: NULL object, NULL control, NULL stored identity, absent core, alias.
 * operation: acquire, release, two acquisitions followed by two releases. */
bool oracle_object_ref_probe(unsigned operation, unsigned flags, unsigned type, long refs, long weak,
			     object_ref_get get, object_ref_release release);
bool oracle_object_ref_probe(unsigned operation, unsigned flags, unsigned type, long refs, long weak,
			     object_ref_get get, object_ref_release release)
{
	struct object_ref_fixture fixture;
	struct obs_weak_object control;
	struct obs_context_data alias;
	struct object_ref_observation observations[2];
	long counts[2][2];
	bool results[2][2];
	bool unchanged[2];
	for (unsigned ported = 0; ported < 2; ++ported) {
		memset(&fixture, 0, sizeof(fixture));
		memset(&control, 0, sizeof(control));
		memset(&alias, 0, sizeof(alias));
		memset(&observations[ported], 0, sizeof(observations[ported]));
		observations[ported].exact_warning = true;
		fixture.context.type = (enum obs_obj_type)type;
		fixture.context.control = (flags & 2) ? NULL : &control;
		fixture.context.destroy = object_ref_destroy;
		fixture.observation = &observations[ported];
		control.ref.refs = refs;
		control.ref.weak_refs = weak;
		control.object = (flags & 4) ? NULL : &fixture.context;
		alias.control = fixture.context.control;
		struct obs_context_data before;
		memcpy(&before, &fixture.context, sizeof(before));
		obs_object_t *argument = (flags & 1) ? NULL : ((flags & 16) ? &alias : &fixture.context);
		object_ref_core = (flags & 8) ? NULL : (struct obs_core *)(uintptr_t)1;
		oracle_core_initialized_set(object_ref_core);
		object_ref_get acquire = ported ? get : oracle_obs_object_get_ref;
		object_ref_release drop = ported ? release : oracle_obs_object_release;
		log_handler_t previous;
		void *previous_param;
		base_get_log_handler(&previous, &previous_param);
		base_set_log_handler(object_ref_warning, &observations[ported]);
		results[ported][0] = results[ported][1] = true;
		if (operation != 1) {
			obs_object_t *expected = !(flags & 7) && refs >= 0 ? &fixture.context : NULL;
			results[ported][0] = acquire(argument) == expected;
			if (operation == 2)
				results[ported][1] = acquire(argument) == expected;
		}
		if (operation == 1) {
			drop(argument);
		} else if (operation == 2) {
			drop(&fixture.context);
			drop(&fixture.context);
		}
		base_set_log_handler(previous, previous_param);
		counts[ported][0] = control.ref.refs;
		counts[ported][1] = control.ref.weak_refs;
		/* Callback is allowed to clear control. Restore only that field before
		 * comparing every byte of the remaining private context. */
		fixture.context.control = before.control;
		unchanged[ported] = memcmp(&before, &fixture.context, sizeof(before)) == 0;
	}
	oracle_core_initialized_set(NULL);
	return results[0][0] && results[0][1] && results[1][0] && results[1][1] && unchanged[0] && unchanged[1] &&
	       memcmp(counts[0], counts[1], sizeof(counts[0])) == 0 &&
	       memcmp(&observations[0], &observations[1], sizeof(observations[0])) == 0;
}

bool oracle_object_ref_final_free(unsigned type, object_ref_release release);
bool oracle_object_ref_final_free(unsigned type, object_ref_release release)
{
	struct object_ref_fixture fixture = {0};
	struct object_ref_observation o = {0};
	fixture.context.type = (enum obs_obj_type)type;
	fixture.context.destroy = object_ref_destroy;
	fixture.observation = &o;
	long allocations = bnum_allocs();
	fixture.context.control = bzalloc(sizeof(*fixture.context.control));
	fixture.context.control->object = &fixture.context;
	oracle_core_initialized_set((void *)(uintptr_t)1);
	release(&fixture.context);
	oracle_core_initialized_set(NULL);
	return o.destroys == 1 && o.identity && o.refs_at_destroy == -1 && o.weak_at_destroy == 0 &&
	       bnum_allocs() == allocations;
}
