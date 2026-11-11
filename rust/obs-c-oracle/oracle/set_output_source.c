/* Callback-only original-C wrapper oracle: all dependencies are fixture-local.
 * NULL/uninitialized core and a dead canvas/source are original C UB, excluded.
 * The downstream setter is observed even for out-of-range channels: its debug
 * assertion/release guard belongs to the unchanged real setter (Tier 2). */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

struct set_output_source_fixture {
	struct obs_core core;
	struct obs_canvas canvases[2];
	struct obs_source source;
	struct obs_weak_source source_control;
	struct obs_weak_object canvas_control;
	uint32_t channel;
	obs_source_t *requested;
	unsigned calls;
	long initial;
	bool valid;
};
#if defined(_MSC_VER)
static __declspec(thread) struct set_output_source_fixture *set_output_source_current;
#else
static _Thread_local struct set_output_source_fixture *set_output_source_current;
#endif

static void set_output_source_observe(obs_canvas_t *canvas, uint32_t channel, obs_source_t *source)
{
	struct set_output_source_fixture *f = set_output_source_current;
	f->calls++;
	f->valid = f->valid && canvas == f->core.data.main_canvas && channel == f->channel && source == f->requested;
	f->valid = f->valid && f->source_control.ref.refs == f->initial && f->canvas_control.ref.refs == f->initial;
}

void oracle_obs_set_output_source(uint32_t channel, obs_source_t *source);
#define obs (&set_output_source_current->core)
#define obs_canvas_set_channel set_output_source_observe
#define obs_set_output_source oracle_obs_set_output_source
#include "libobs/obs-set-output-source.c.h"
#undef obs_set_output_source
#include "set-output-source-fields.inc"
#undef obs_canvas_set_channel
#undef obs

void oracle_set_output_source_layout(size_t *layout);
void oracle_set_output_source_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_set_output_source_fields);
	layout[1] = _Alignof(struct obs_set_output_source_fields);
	layout[2] = offsetof(struct obs_set_output_source_fields, canvas);
	layout[3] = offsetof(struct obs_set_output_source_fields, set_channel);
}

typedef void (*set_output_source_fn)(uint32_t, obs_source_t *);
bool oracle_set_output_source_check(uint32_t channel, bool present, bool second_canvas, long initial,
				    set_output_source_fn setter);
bool oracle_set_output_source_check(uint32_t channel, bool present, bool second_canvas, long initial,
				    set_output_source_fn setter)
{
	struct set_output_source_fixture *f = calloc(1, sizeof(*f));
	struct set_output_source_fixture *before = malloc(sizeof(*f));
	struct set_output_source_fixture *expected = malloc(sizeof(*f));
	if (!f || !before || !expected) {
		free(expected);
		free(before);
		free(f);
		return false;
	}
	f->core.data.main_canvas = &f->canvases[second_canvas ? 1 : 0];
	f->source.context.control = (struct obs_weak_object *)&f->source_control;
	f->core.data.main_canvas->context.control = &f->canvas_control;
	f->source_control.ref.refs = initial;
	f->canvas_control.ref.refs = initial;
	f->source_control.source = &f->source;
	f->channel = channel;
	f->initial = initial;
	f->requested = present ? &f->source : NULL;
	f->valid = true;
	memcpy(before, f, sizeof(*f));
	set_output_source_current = f;
	oracle_obs_set_output_source(channel, f->requested);
	memcpy(expected, f, sizeof(*f));
	memcpy(f, before, sizeof(*f));
	setter(channel, f->requested);
	bool valid = f->valid && f->calls == 1 && f->source_control.ref.refs == initial &&
		     f->canvas_control.ref.refs == initial && memcmp(f, expected, sizeof(*f)) == 0;
	set_output_source_current = NULL;
	free(expected);
	free(before);
	free(f);
	return valid;
}
