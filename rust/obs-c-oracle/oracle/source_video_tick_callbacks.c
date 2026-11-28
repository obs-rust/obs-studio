/* Tier 3: the unchanged extracted per-source callback steps of
 * obs_source_video_tick() (libobs/obs-source-video-tick-callbacks.c.h:
 * obs_source_deferred_update(), activate_source(), deactivate_source(),
 * show_source(), hide_source()) and the production field adapters of
 * obs-source-video-tick-callbacks-fields.c.h the Rust exports call, over
 * the real source layout with recording type callbacks and real source and
 * core signal handlers. A private core keeps this fixture from sharing
 * another oracle's global; Rust runs each parity test in an isolated
 * child. */
#define obs source_video_tick_callbacks_oracle_core
#include "obs-internal.h"
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

struct obs_core *obs;

void oracle_obs_source_deferred_update(obs_source_t *source);
void oracle_activate_source(obs_source_t *source);
void oracle_deactivate_source(obs_source_t *source);
void oracle_show_source(obs_source_t *source);
void oracle_hide_source(obs_source_t *source);
#define obs_source_deferred_update oracle_obs_source_deferred_update
#define activate_source oracle_activate_source
#define deactivate_source oracle_deactivate_source
#define show_source oracle_show_source
#define hide_source oracle_hide_source
#include "source-video-tick-callbacks.inc"
#undef hide_source
#undef show_source
#undef deactivate_source
#undef activate_source
#undef obs_source_deferred_update
#include "source-video-tick-callbacks-fields.inc"

struct vtc_fixture {
	struct obs_core core;
	struct obs_source source;
	int plugin_data;
	/* Added to defer_update_count from inside each update callback. */
	long redefer;
	/* Every argument a callback or signal saw was the fixture's own. */
	bool identity;
	char events[1024];
	size_t len;
};

static struct vtc_fixture *vtc_of_data(void *data)
{
	return (struct vtc_fixture *)((char *)data - offsetof(struct vtc_fixture, plugin_data));
}

static void vtc_event(struct vtc_fixture *f, const char *prefix, const char *name)
{
	int n = snprintf(f->events + f->len, sizeof(f->events) - f->len, "%s%s:%s", f->len ? " " : "", prefix, name);
	if (n > 0 && (size_t)n < sizeof(f->events) - f->len)
		f->len += (size_t)n;
}

static void vtc_update(void *data, obs_data_t *settings)
{
	struct vtc_fixture *f = vtc_of_data(data);
	f->identity = f->identity && settings == f->source.context.settings;
	vtc_event(f, "cb", "update");
	/* Wraps like the Rust model; only this thread touches the count. */
	unsigned long count = (unsigned long)os_atomic_load_long(&f->source.defer_update_count);
	os_atomic_set_long(&f->source.defer_update_count, (long)(count + (unsigned long)f->redefer));
}

#define VTC_CALLBACK(step)                            \
	static void vtc_##step(void *data)            \
	{                                             \
		vtc_event(vtc_of_data(data), "cb", #step); \
	}
VTC_CALLBACK(activate)
VTC_CALLBACK(deactivate)
VTC_CALLBACK(show)
VTC_CALLBACK(hide)

static void vtc_local(void *param, calldata_t *data)
{
	struct vtc_fixture *f = param;
	f->identity = f->identity && calldata_ptr(data, "source") == &f->source;
}

#define VTC_LOCAL(step)                                                           \
	static void vtc_local_##step(void *param, calldata_t *data)               \
	{                                                                         \
		vtc_local(param, data);                                           \
		vtc_event(param, "l", #step);                                     \
	}
VTC_LOCAL(update)
VTC_LOCAL(activate)
VTC_LOCAL(deactivate)
VTC_LOCAL(show)
VTC_LOCAL(hide)

static void vtc_global(void *param, const char *name, calldata_t *data)
{
	struct vtc_fixture *f = param;
	f->identity = f->identity && calldata_ptr(data, "source") == &f->source;
	vtc_event(f, "g", name);
}

/* bits: 0 data, 1 update, 2 activate, 3 deactivate, 4 show, 5 hide,
 * 6 private. */
void *oracle_vtc_new(uint32_t bits, long defer, long redefer);
void *oracle_vtc_new(uint32_t bits, long defer, long redefer)
{
	struct vtc_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return NULL;
	f->identity = true;
	f->redefer = redefer;
	f->source.context.data = (bits & 1u) ? &f->plugin_data : NULL;
	f->source.context.settings = obs_data_create();
	f->source.context.private = (bits & 64u) != 0;
	f->source.info.update = (bits & 2u) ? vtc_update : NULL;
	f->source.info.activate = (bits & 4u) ? vtc_activate : NULL;
	f->source.info.deactivate = (bits & 8u) ? vtc_deactivate : NULL;
	f->source.info.show = (bits & 16u) ? vtc_show : NULL;
	f->source.info.hide = (bits & 32u) ? vtc_hide : NULL;
	f->source.defer_update_count = defer;
	f->source.context.signals = signal_handler_create();
	f->core.signals = signal_handler_create();
	static const char *const decls[] = {"void update(ptr source)", "void activate(ptr source)",
					    "void deactivate(ptr source)", "void show(ptr source)",
					    "void hide(ptr source)"};
	static const char *const core_decls[] = {"void source_update(ptr source)", "void source_activate(ptr source)",
						 "void source_deactivate(ptr source)", "void source_show(ptr source)",
						 "void source_hide(ptr source)"};
	for (size_t i = 0; i < 5; i++) {
		signal_handler_add(f->source.context.signals, decls[i]);
		signal_handler_add(f->core.signals, core_decls[i]);
	}
	signal_handler_connect(f->source.context.signals, "update", vtc_local_update, f);
	signal_handler_connect(f->source.context.signals, "activate", vtc_local_activate, f);
	signal_handler_connect(f->source.context.signals, "deactivate", vtc_local_deactivate, f);
	signal_handler_connect(f->source.context.signals, "show", vtc_local_show, f);
	signal_handler_connect(f->source.context.signals, "hide", vtc_local_hide, f);
	signal_handler_connect_global(f->core.signals, vtc_global, f);
	return f;
}

typedef void (*vtc_step_fn)(obs_source_t *);

/* Run one step on the fixture's source under its private core. */
void oracle_vtc_run(void *handle, vtc_step_fn step);
void oracle_vtc_run(void *handle, vtc_step_fn step)
{
	struct vtc_fixture *f = handle;
	struct obs_core *old = obs;
	obs = &f->core;
	step(&f->source);
	obs = old;
}

/* The record: the events in order (space separated "cb:<callback>",
 * "g:<global signal>", "l:<source signal>"), whether every argument was the
 * fixture's own, the defer count, and whether the source still holds its
 * data and callbacks as built. */
const char *oracle_vtc_record(const void *handle, bool *identity, long *defer);
const char *oracle_vtc_record(const void *handle, bool *identity, long *defer)
{
	const struct vtc_fixture *f = handle;
	*identity = f->identity;
	*defer = f->source.defer_update_count;
	return f->events;
}

void oracle_vtc_free(void *handle);
void oracle_vtc_free(void *handle)
{
	struct vtc_fixture *f = handle;
	if (!f)
		return;
	signal_handler_destroy(f->source.context.signals);
	signal_handler_destroy(f->core.signals);
	obs_data_release(f->source.context.settings);
	free(f);
}
