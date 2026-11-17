/* Original canvas strong release over the real private layout, with a traced
 * obs_canvas_destroy seam. Excludes signed overflow and release without strong
 * ownership (undefined in C). After shutdown any non-NULL handle is only
 * compared for the warning: neither side may read it. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "obs-internal.h"

static struct obs_core *canvas_release_core;
void oracle_core_initialized_set(void *core);
void oracle_obs_weak_canvas_release(obs_weak_canvas_t *weak);
void oracle_obs_canvas_release(obs_canvas_t *canvas);

#define obs canvas_release_core
#define obs_weak_canvas_release oracle_obs_weak_canvas_release
#define obs_canvas_release oracle_obs_canvas_release
#include "obs-canvas-release.c.h"
#undef obs_canvas_release
#undef obs_weak_canvas_release
#undef obs

struct canvas_release_observation {
	unsigned destroys;
	bool identity;
	long refs_at_destroy;
	long weak_at_destroy;
	unsigned warnings;
	bool exact_warning;
};

struct canvas_release_fixture {
	obs_canvas_t *canvas;
	obs_weak_canvas_t *control;
	struct canvas_release_observation *observation;
};

#if defined(_MSC_VER)
static __declspec(thread) struct canvas_release_fixture *canvas_release_current;
#else
static _Thread_local struct canvas_release_fixture *canvas_release_current;
#endif

/* The real destroy frees the canvas context. Record the counts it sees, then
 * clear the context's control so any later read of the context is caught. */
void obs_canvas_destroy(obs_canvas_t *canvas)
{
	struct canvas_release_fixture *f = canvas_release_current;
	if (!f)
		abort();
	struct canvas_release_observation *o = f->observation;
	o->destroys++;
	o->identity = canvas == f->canvas;
	o->refs_at_destroy = os_atomic_load_long(&f->control->ref.refs);
	o->weak_at_destroy = os_atomic_load_long(&f->control->ref.weak_refs);
	canvas->context.control = NULL;
}

#if defined(__GNUC__) || defined(__clang__)
static void canvas_release_warning(int level, const char *message, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void canvas_release_warning(int level, const char *message, va_list args, void *param)
{
	(void)args;
	struct canvas_release_observation *o = param;
	o->warnings++;
	o->exact_warning = o->exact_warning && level == LOG_WARNING &&
			   strcmp(message, "Tried to release a canvas when the OBS core is shut down!") == 0;
}

typedef void (*canvas_release_fn)(obs_canvas_t *);

/* flags: 1 NULL canvas, 2 absent core, 4 invalid non-NULL handle (with 2).
 * releases: how many times the same owner set is released (weak >= 1 keeps
 * the stack control alive across a final release). */
bool oracle_canvas_release_probe(unsigned flags, unsigned releases, long refs, long weak, canvas_release_fn ported);
bool oracle_canvas_release_probe(unsigned flags, unsigned releases, long refs, long weak, canvas_release_fn ported)
{
	obs_canvas_t *canvas = calloc(1, sizeof(*canvas));
	obs_canvas_t *before = malloc(sizeof(*before));
	struct canvas_release_observation observations[2];
	long counts[2][2];
	bool unchanged[2] = {false, false};
	bool valid = canvas && before && !((flags & 4) && !(flags & 2));
	for (unsigned side = 0; valid && side < 2; ++side) {
		struct obs_weak_canvas control;
		memset(canvas, 0, sizeof(*canvas));
		memset(&control, 0, sizeof(control));
		memset(&observations[side], 0, sizeof(observations[side]));
		observations[side].exact_warning = true;
		control.ref.refs = refs;
		control.ref.weak_refs = weak;
		control.canvas = canvas;
		canvas->context.control = (obs_weak_object_t *)&control;
		memcpy(before, canvas, sizeof(*canvas));
		struct canvas_release_fixture fixture = {canvas, &control, &observations[side]};
		canvas_release_current = &fixture;
		canvas_release_core = (flags & 2) ? NULL : (struct obs_core *)(uintptr_t)1;
		oracle_core_initialized_set(canvas_release_core);
		obs_canvas_t *argument = (flags & 1) ? NULL : ((flags & 4) ? (obs_canvas_t *)(uintptr_t)1 : canvas);
		canvas_release_fn release = side ? ported : oracle_obs_canvas_release;
		log_handler_t previous;
		void *previous_param;
		base_get_log_handler(&previous, &previous_param);
		base_set_log_handler(canvas_release_warning, &observations[side]);
		for (unsigned i = 0; i < releases; ++i)
			release(argument);
		base_set_log_handler(previous, previous_param);
		canvas_release_current = NULL;
		counts[side][0] = control.ref.refs;
		counts[side][1] = control.ref.weak_refs;
		/* Destroy clears the control; restore it, then every other byte of the
		 * private canvas must be untouched. */
		canvas->context.control = before->context.control;
		unchanged[side] = memcmp(before, canvas, sizeof(*canvas)) == 0;
	}
	canvas_release_core = NULL;
	oracle_core_initialized_set(NULL);
	free(before);
	free(canvas);
	return valid && unchanged[0] && unchanged[1] && memcmp(counts[0], counts[1], sizeof(counts[0])) == 0 &&
	       memcmp(&observations[0], &observations[1], sizeof(observations[0])) == 0;
}

/* The final release of a canvas whose weak control has no other owner frees
 * that control after destroy has returned, and nothing else. */
bool oracle_canvas_release_final_free(canvas_release_fn release);
bool oracle_canvas_release_final_free(canvas_release_fn release)
{
	obs_canvas_t *canvas = calloc(1, sizeof(*canvas));
	if (!canvas)
		return false;
	struct canvas_release_observation o = {0};
	long allocations = bnum_allocs();
	obs_weak_canvas_t *control = bzalloc(sizeof(*control));
	control->canvas = canvas;
	canvas->context.control = (obs_weak_object_t *)control;
	struct canvas_release_fixture fixture = {canvas, control, &o};
	canvas_release_current = &fixture;
	oracle_core_initialized_set((void *)(uintptr_t)1);
	canvas_release_core = (struct obs_core *)(uintptr_t)1;
	release(canvas);
	canvas_release_core = NULL;
	oracle_core_initialized_set(NULL);
	canvas_release_current = NULL;
	free(canvas);
	return o.destroys == 1 && o.identity && o.refs_at_destroy == -1 && o.weak_at_destroy == 0 &&
	       bnum_allocs() == allocations;
}
