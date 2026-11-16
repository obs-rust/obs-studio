/* Original setter plus production adapters over real private layouts. Shared
 * stand-ins isolate context naming and signal delivery, not setter decisions. */
#include "obs-internal.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

struct canvas_rename_fixture {
	struct obs_canvas canvas;
	struct obs_core core;
	char *taken;
	char *local_prev;
	char *local_new;
	char *global_prev;
	char *global_new;
	unsigned local_count;
	unsigned global_count;
	unsigned updates;
	bool flip_private;
};
static struct obs_core *canvas_rename_obs;
static void canvas_rename_update(struct obs_context_data *context, const char *name);
static void canvas_rename_update_ht(struct obs_context_data *context, const char *name, void *head);
static void canvas_rename_signal(signal_handler_t *handler, const char *signal, calldata_t *data);

void oracle_obs_canvas_set_name(obs_canvas_t *canvas, const char *name);
#define obs_canvas_rules_name_empty oracle_obs_canvas_rules_name_empty
#define obs_canvas_rules_renames oracle_obs_canvas_rules_renames
#define obs_canvas_rules_public oracle_obs_canvas_rules_public
#include "obs-canvas-rules.h"
#define obs canvas_rename_obs
#define obs_context_data_setname canvas_rename_update
#define obs_context_data_setname_ht canvas_rename_update_ht
#define signal_handler_signal canvas_rename_signal
#define obs_canvas_set_name oracle_obs_canvas_set_name
#include "canvas-set-name.inc"
#undef obs_canvas_set_name
#include "canvas_set_name_fields.inc"
#undef signal_handler_signal
#undef obs_context_data_setname_ht
#undef obs_context_data_setname
#undef obs

static void canvas_rename_update(struct obs_context_data *context, const char *name)
{
	struct canvas_rename_fixture *f = (void *)context;
	char *copy = bstrdup(name);
	bfree(context->name);
	context->name = copy;
	f->updates++;
}

static void canvas_rename_update_ht(struct obs_context_data *context, const char *name, void *head)
{
	struct canvas_rename_fixture *f = (void *)context;
	(void)head;
	/* The real setter excludes itself before searching. One occupied name
	 * exercises resolution to a suffix, including resolution to the old name. */
	struct dstr candidate = {0};
	if (strcmp(name, f->taken) == 0) {
		dstr_copy(&candidate, name);
		dstr_cat(&candidate, " 2");
		canvas_rename_update(context, candidate.array);
		dstr_free(&candidate);
	} else {
		canvas_rename_update(context, name);
	}
}

static void canvas_rename_signal(signal_handler_t *handler, const char *signal, calldata_t *data)
{
	struct canvas_rename_fixture *f = (void *)handler;
	bool local = strcmp(signal, "rename") == 0;
	char **prev = local ? &f->local_prev : &f->global_prev;
	char **name = local ? &f->local_new : &f->global_new;
	bfree(*prev);
	bfree(*name);
	*prev = bstrdup(calldata_string(data, "prev_name"));
	*name = bstrdup(calldata_string(data, "new_name"));
	if (calldata_ptr(data, "canvas") != &f->canvas)
		abort();
	if (local) {
		f->local_count++;
		if (f->flip_private)
			f->canvas.context.private = !f->canvas.context.private;
		/* A local callback's payload mutation is visible to the global one. */
		calldata_set_string(data, "prev_name", "callback");
	} else {
		f->global_count++;
	}
}

void *oracle_canvas_rename_new(const char *name, const char *taken, uint32_t flags, bool private, bool flip);
void *oracle_canvas_rename_new(const char *name, const char *taken, uint32_t flags, bool private, bool flip)
{
	struct canvas_rename_fixture *f = bzalloc(sizeof(*f));
	f->canvas.context.name = bstrdup(name);
	f->canvas.context.private = private;
	f->canvas.context.signals = (void *)f;
	f->canvas.flags = flags;
	f->core.signals = (void *)f;
	f->taken = bstrdup(taken);
	f->flip_private = flip;
	return f;
}

void oracle_canvas_rename_select(void *handle);
void oracle_canvas_rename_select(void *handle)
{
	struct canvas_rename_fixture *f = handle;
	canvas_rename_obs = &f->core;
}

const char *oracle_canvas_rename_string(void *handle, unsigned which);
const char *oracle_canvas_rename_string(void *handle, unsigned which)
{
	struct canvas_rename_fixture *f = handle;
	switch (which) {
	case 0:
		return f->canvas.context.name;
	case 1:
		return f->local_prev;
	case 2:
		return f->local_new;
	case 3:
		return f->global_prev;
	case 4:
		return f->global_new;
	default:
		return NULL;
	}
}

unsigned oracle_canvas_rename_count(void *handle, unsigned which);
unsigned oracle_canvas_rename_count(void *handle, unsigned which)
{
	struct canvas_rename_fixture *f = handle;
	switch (which) {
	case 0:
		return f->local_count;
	case 1:
		return f->global_count;
	case 2:
		return f->updates;
	default:
		return f->canvas.context.private;
	}
}

void oracle_canvas_rename_free(void *handle);
void oracle_canvas_rename_free(void *handle)
{
	struct canvas_rename_fixture *f = handle;
	bfree(f->canvas.context.name);
	bfree(f->taken);
	bfree(f->local_prev);
	bfree(f->local_new);
	bfree(f->global_prev);
	bfree(f->global_new);
	bfree(f);
	canvas_rename_obs = NULL;
}
