/* Original wrapper and real adapter layout, with fixture-local dependencies.
 * Exclude uninitialized/dead core/canvas, invalid callback/parameter and ref
 * overflow (original C UB). Actual traversal is covered by unchanged Tier 2. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

typedef bool (*enum_scenes_proc)(void *, obs_source_t *);
struct enum_scenes_fixture {
	struct obs_core core;
	struct obs_canvas canvases[2];
	struct obs_source sources[4];
	struct obs_weak_source controls[4];
	struct obs_weak_object canvas_control;
	unsigned params[2], calls, visited, total, stop, seen[4], tag;
	long initial;
	enum_scenes_proc requested;
	void *param;
	bool valid, acquire;
};
#if defined(_MSC_VER)
static __declspec(thread) struct enum_scenes_fixture *enum_scenes_current;
#else
static _Thread_local struct enum_scenes_fixture *enum_scenes_current;
#endif

static bool enum_scenes_callback(void *param, obs_source_t *source, unsigned tag)
{
	struct enum_scenes_fixture *f = enum_scenes_current;
	unsigned i = f->visited;
	if (i >= 4) {
		f->valid = false;
		return false;
	}
	f->valid = f->valid && param == f->param && source == &f->sources[i] &&
		   f->controls[i].ref.refs == f->initial + 1;
	f->seen[i] = i;
	f->tag ^= tag;
	f->visited++;
	if (f->acquire)
		f->controls[i].ref.refs++;
	return !f->stop || f->visited < f->stop;
}

static bool enum_scenes_a(void *param, obs_source_t *source)
{
	return enum_scenes_callback(param, source, 17);
}
static bool enum_scenes_b(void *param, obs_source_t *source)
{
	return enum_scenes_callback(param, source, 91);
}

static void enum_scenes_observe(obs_canvas_t *canvas, enum_scenes_proc callback, void *param)
{
	struct enum_scenes_fixture *f = enum_scenes_current;
	f->calls++;
	f->valid = f->valid && canvas == f->core.data.main_canvas && callback == f->requested && param == f->param &&
		   f->canvas_control.ref.refs == f->initial;
	for (unsigned i = 0; i < f->total; i++) {
		f->controls[i].ref.refs++;
		bool more = callback(param, &f->sources[i]);
		f->controls[i].ref.refs--;
		if (!more)
			break;
	}
}

void oracle_obs_enum_scenes(enum_scenes_proc callback, void *param);
#define obs (&enum_scenes_current->core)
#define obs_canvas_enum_scenes enum_scenes_observe
#define obs_enum_scenes oracle_obs_enum_scenes
#include "libobs/obs-enum-scenes.c.h"
#undef obs_enum_scenes
#include "enum-scenes-fields.inc"
#undef obs_canvas_enum_scenes
#undef obs

void oracle_enum_scenes_layout(size_t *layout);
void oracle_enum_scenes_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_enum_scenes_fields);
	layout[1] = _Alignof(struct obs_enum_scenes_fields);
	layout[2] = offsetof(struct obs_enum_scenes_fields, canvas);
	layout[3] = offsetof(struct obs_enum_scenes_fields, enumerate);
}

typedef void (*enum_scenes_fn)(enum_scenes_proc, void *);
bool oracle_enum_scenes_check(unsigned total, unsigned stop, bool second_canvas, bool second_callback,
			      bool second_param, bool acquire, long initial, enum_scenes_fn wrapper);
bool oracle_enum_scenes_check(unsigned total, unsigned stop, bool second_canvas, bool second_callback,
			      bool second_param, bool acquire, long initial, enum_scenes_fn wrapper)
{
	struct enum_scenes_fixture *f = calloc(1, sizeof(*f));
	struct enum_scenes_fixture *before = malloc(sizeof(*f)), *expected = malloc(sizeof(*f));
	if (!f || !before || !expected) {
		free(expected);
		free(before);
		free(f);
		return false;
	}
	f->core.data.main_canvas = &f->canvases[second_canvas ? 1 : 0];
	f->core.data.main_canvas->context.control = &f->canvas_control;
	f->canvas_control.ref.refs = initial;
	for (unsigned i = 0; i < 4; i++) {
		f->sources[i].context.control = (struct obs_weak_object *)&f->controls[i];
		f->controls[i].ref.refs = initial;
		f->controls[i].source = &f->sources[i];
	}
	f->total = total;
	f->stop = stop;
	f->acquire = acquire;
	f->initial = initial;
	/* NULL callback is valid for an empty traversal; NULL opaque data is valid
	 * whenever the chosen callback accepts it. */
	f->requested = total == 0 && second_callback ? NULL : (second_callback ? enum_scenes_b : enum_scenes_a);
	f->param = second_param ? NULL : &f->params[0];
	f->valid = true;
	enum_scenes_current = f;
	memcpy(before, f, sizeof(*f));
	oracle_obs_enum_scenes(f->requested, f->param);
	memcpy(expected, f, sizeof(*f));
	memcpy(f, before, sizeof(*f));
	wrapper(f->requested, f->param);
	bool valid = f->valid && f->calls == 1 && memcmp(f, expected, sizeof(*f)) == 0;
	enum_scenes_current = NULL;
	free(expected);
	free(before);
	free(f);
	return valid;
}
