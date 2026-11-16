/* Original-C wrapper and real adapter over thread-local, real-layout fixtures.
 * The downstream observer pins delegation and ownership effects; unchanged
 * Tier 2 exercises the real reference operation in both actual libraries.
 * Uninitialized core, dead canvases and overflowing counts are excluded.
 * There are no intentional behavior differences. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

struct get_main_canvas_fixture {
	struct obs_core core;
	struct obs_canvas canvases[2];
	struct obs_weak_object controls[2];
	obs_canvas_t *returned;
	long initial;
	unsigned calls;
	bool valid;
};
#if defined(_MSC_VER)
static __declspec(thread) struct get_main_canvas_fixture *get_main_canvas_current;
#else
static _Thread_local struct get_main_canvas_fixture *get_main_canvas_current;
#endif

static obs_canvas_t *get_main_canvas_observe(obs_canvas_t *canvas)
{
	struct get_main_canvas_fixture *f = get_main_canvas_current;
	f->calls++;
	f->valid = f->valid && canvas == f->core.data.main_canvas;
	if (!canvas)
		return NULL;
	/* Detect any wrapper-side acquisition before the downstream operation. */
	f->valid = f->valid && canvas->context.control->ref.refs == f->initial;
	canvas->context.control->ref.refs++;
	return f->returned;
}

obs_canvas_t *oracle_obs_get_main_canvas(void);
#define obs (&get_main_canvas_current->core)
#define obs_canvas_get_ref get_main_canvas_observe
#define obs_get_main_canvas oracle_obs_get_main_canvas
#include "libobs/obs-get-main-canvas.c.h"
#undef obs_get_main_canvas
#include "get-main-canvas-fields.inc"
#undef obs_canvas_get_ref
#undef obs

void oracle_get_main_canvas_layout(size_t *layout);
void oracle_get_main_canvas_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_get_main_canvas_fields);
	layout[1] = _Alignof(struct obs_get_main_canvas_fields);
	layout[2] = offsetof(struct obs_get_main_canvas_fields, canvas);
	layout[3] = offsetof(struct obs_get_main_canvas_fields, get_ref);
}

typedef obs_canvas_t *(*get_main_canvas_fn)(void);
bool oracle_get_main_canvas_check(bool present, bool second, bool alternate, bool null_result, long initial,
				  get_main_canvas_fn getter);
bool oracle_get_main_canvas_check(bool present, bool second, bool alternate, bool null_result, long initial,
				  get_main_canvas_fn getter)
{
	struct get_main_canvas_fixture *f = calloc(1, sizeof(*f));
	struct get_main_canvas_fixture *before = malloc(sizeof(*f));
	struct get_main_canvas_fixture *expected = malloc(sizeof(*f));
	if (!f || !before || !expected) {
		free(expected);
		free(before);
		free(f);
		return false;
	}
	for (size_t i = 0; i < 2; ++i) {
		f->canvases[i].context.control = &f->controls[i];
		f->controls[i].object = &f->canvases[i].context;
		f->controls[i].ref.refs = initial;
		f->controls[i].ref.weak_refs = 7;
	}
	f->core.data.main_canvas = present ? &f->canvases[second ? 1 : 0] : NULL;
	/* An observer may return NULL or a different live identity. The wrapper
	 * must preserve the downstream return without adding a reference. */
	f->returned = null_result ? NULL : &f->canvases[(second != alternate) ? 1 : 0];
	f->initial = initial;
	f->valid = true;
	memcpy(before, f, sizeof(*f));
	get_main_canvas_current = f;
	obs_canvas_t *original = oracle_obs_get_main_canvas();
	memcpy(expected, f, sizeof(*f));
	memcpy(f, before, sizeof(*f));
	obs_canvas_t *actual = getter();
	bool valid = actual == original && f->valid && f->calls == 1 && memcmp(f, expected, sizeof(*f)) == 0;
	get_main_canvas_current = NULL;
	free(expected);
	free(before);
	free(f);
	return valid;
}
