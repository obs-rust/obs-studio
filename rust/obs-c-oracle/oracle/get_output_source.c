/* Original-C wrapper oracle. The downstream getter is observed, not reimplemented.
 * Uninitialized core, dead main canvas/source and overflowing refcounts are C
 * UB, excluded. Full-width channels reach the observer unchanged; real getter
 * bounds assertions/release guards and reference effects are unchanged Tier 2. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

struct get_output_source_fixture {
	struct obs_core core;
	struct obs_canvas canvases[2];
	struct obs_source sources[2];
	struct obs_weak_source controls[2];
	struct obs_weak_object canvas_control;
	uint32_t channel;
	unsigned selected;
	unsigned calls;
	long initial;
	bool present;
	bool valid;
};
#if defined(_MSC_VER)
static __declspec(thread) struct get_output_source_fixture *get_output_source_current;
#else
static _Thread_local struct get_output_source_fixture *get_output_source_current;
#endif

static obs_source_t *get_output_source_observe(obs_canvas_t *canvas, uint32_t channel)
{
	struct get_output_source_fixture *f = get_output_source_current;
	f->calls++;
	f->valid = f->valid && canvas == f->core.data.main_canvas && channel == f->channel;
	f->valid = f->valid && f->canvas_control.ref.refs == f->initial;
	for (size_t i = 0; i < 2; i++)
		f->valid = f->valid && f->controls[i].ref.refs == f->initial;
	if (!f->present)
		return NULL;
	f->controls[f->selected].ref.refs++;
	return &f->sources[f->selected];
}

obs_source_t *oracle_obs_get_output_source(uint32_t channel);
#define obs (&get_output_source_current->core)
#define obs_canvas_get_channel get_output_source_observe
#define obs_get_output_source oracle_obs_get_output_source
#include "libobs/obs-get-output-source.c.h"
#undef obs_get_output_source
#include "get-output-source-fields.inc"
#undef obs_canvas_get_channel
#undef obs

void oracle_get_output_source_layout(size_t *layout);
void oracle_get_output_source_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_get_output_source_fields);
	layout[1] = _Alignof(struct obs_get_output_source_fields);
	layout[2] = offsetof(struct obs_get_output_source_fields, canvas);
	layout[3] = offsetof(struct obs_get_output_source_fields, get_channel);
}

typedef obs_source_t *(*get_output_source_fn)(uint32_t);
bool oracle_get_output_source_check(uint32_t channel, bool present, bool second_canvas, bool second_source,
				    long initial, get_output_source_fn getter);
bool oracle_get_output_source_check(uint32_t channel, bool present, bool second_canvas, bool second_source,
				    long initial, get_output_source_fn getter)
{
	struct get_output_source_fixture *f = calloc(1, sizeof(*f));
	struct get_output_source_fixture *before = malloc(sizeof(*f));
	struct get_output_source_fixture *expected = malloc(sizeof(*f));
	if (!f || !before || !expected) {
		free(expected);
		free(before);
		free(f);
		return false;
	}
	f->core.data.main_canvas = &f->canvases[second_canvas ? 1 : 0];
	f->core.data.main_canvas->context.control = &f->canvas_control;
	f->canvas_control.ref.refs = initial;
	for (size_t i = 0; i < 2; i++) {
		f->sources[i].context.control = (struct obs_weak_object *)&f->controls[i];
		f->controls[i].source = &f->sources[i];
		f->controls[i].ref.refs = initial;
	}
	f->channel = channel;
	f->initial = initial;
	f->selected = second_source ? 1 : 0;
	f->present = present;
	f->valid = true;
	memcpy(before, f, sizeof(*f));
	get_output_source_current = f;
	obs_source_t *c_result = oracle_obs_get_output_source(channel);
	memcpy(expected, f, sizeof(*f));
	memcpy(f, before, sizeof(*f));
	obs_source_t *rust_result = getter(channel);
	bool valid = f->valid && f->calls == 1 && rust_result == c_result &&
		     rust_result == (present ? &f->sources[f->selected] : NULL) && memcmp(f, expected, sizeof(*f)) == 0;
	get_output_source_current = NULL;
	free(expected);
	free(before);
	free(f);
	return valid;
}
