/* Test-only oracle: the original libobs/obs-view-api.c.h, unmodified, with
 * its functions renamed to oracle_* so it can link next to the Rust shims
 * (#1933). The hidden view accessors (obs-view.c) and the source calls it
 * makes are test doubles here that both sides call: each step goes to
 * oracle_view_trace's hook, so a C run and a Rust run give comparable
 * traces. Views are real struct obs_view (the doubles read and write its
 * fields as obs-view.c does; the lock only traces). Sources are
 * struct oracle_view_source, passed as obs_source_t pointers: a name for
 * the trace, a removed flag, and whether obs_source_get_ref() refuses it.
 *
 * obs_source_activate() and obs_source_deactivate() are obs-source's Rust
 * exports (#5811), which both sides reach. For a source no
 * oracle/source_activate.c fixture holds, their counter adapters are the
 * stand-ins there, which report each step here instead of touching the
 * source: the first step of a call (show_refs raised, or loaded) starts an
 * "activate" or "deactivate" event for AUX_VIEW, and a step on
 * activate_refs makes it MAIN_VIEW. The event is traced (call, source
 * name, view type) before the next traced step, when the hook changes, or
 * at oracle_view_flush(), so a trace reads as it did when these were
 * tracing doubles. */
#ifndef _WIN32
#define _GNU_SOURCE
#endif
/* Release libobs builds define NDEBUG, so obs_view_get_source() and
 * obs_view_set_source() return for an out-of-range channel instead of
 * asserting; the parity tests check that path. */
#ifndef NDEBUG
#define NDEBUG
#endif
#define obs_view_init oracle_obs_view_init
#define obs_view_create oracle_obs_view_create
#define obs_view_free oracle_obs_view_free
#define obs_view_destroy oracle_obs_view_destroy
#define obs_view_get_source oracle_obs_view_get_source
#define obs_view_set_source oracle_obs_view_set_source
#define obs_view_render oracle_obs_view_render
#define obs_view_add oracle_obs_view_add
#define obs_view_add2 oracle_obs_view_add2
#define obs_view_remove oracle_obs_view_remove
#define obs_view_enum_video_info oracle_obs_view_enum_video_info

#include "obs-view-api.c.h"

#undef obs_view_init
#undef obs_view_create
#undef obs_view_free
#undef obs_view_destroy
#undef obs_view_get_source
#undef obs_view_set_source
#undef obs_view_render
#undef obs_view_add
#undef obs_view_add2
#undef obs_view_remove
#undef obs_view_enum_video_info

#include <stddef.h>
#include <stdint.h>
#include <string.h>

struct oracle_view_source {
	const char *name;
	bool removed;
	bool dying;
};

typedef void (*oracle_view_trace_fn)(const char *event, const char *name, long long value);

static oracle_view_trace_fn trace_hook;
static bool mutex_fails;
static struct obs_video_info *main_info;

#define ORACLE_VIEW_MIXES 16
static struct {
	struct obs_view *view;
	struct obs_video_info *ovi;
} mixes[ORACLE_VIEW_MIXES];
static size_t mix_num;

/* The activation event still open on this thread (see the top). */
static _Thread_local struct {
	bool open;
	bool deactivate;
	const obs_source_t *source;
	long long type;
} pending;

static void emit(const char *event, const char *name, long long value)
{
	if (trace_hook)
		trace_hook(event, name, value);
}

static const char *src_name(const obs_source_t *source);

void oracle_view_flush(void);
void oracle_view_flush(void)
{
	if (!pending.open)
		return;
	pending.open = false;
	emit(pending.deactivate ? "deactivate" : "activate", src_name(pending.source), pending.type);
}

static void trace(const char *event, const char *name, long long value)
{
	oracle_view_flush();
	emit(event, name, value);
}

/* One counter step of obs_source_activate() (deactivate false) or
 * obs_source_deactivate() (true) on a source: on activate_refs (active
 * true) or on show_refs. */
void oracle_view_activation_step(const obs_source_t *source, bool deactivate, bool active);
void oracle_view_activation_step(const obs_source_t *source, bool deactivate, bool active)
{
	if (active && pending.open && pending.deactivate == deactivate && pending.source == source) {
		pending.type = MAIN_VIEW;
		return;
	}
	oracle_view_flush();
	pending.open = true;
	pending.deactivate = deactivate;
	pending.source = source;
	pending.type = active ? MAIN_VIEW : AUX_VIEW;
}

static const char *src_name(const obs_source_t *source)
{
	return source ? ((const struct oracle_view_source *)(const void *)source)->name : NULL;
}

/* test controls */

void oracle_view_set_trace(oracle_view_trace_fn hook);
void oracle_view_reset(bool fail_mutex, struct obs_video_info *main_mix_info);
size_t oracle_view_mix_num(void);
struct obs_view *oracle_view_mix_at(size_t i);
obs_source_t *oracle_view_channel_at(struct obs_view *view, size_t channel);
int oracle_view_type_of(struct obs_view *view);

void oracle_view_set_trace(oracle_view_trace_fn hook)
{
	oracle_view_flush();
	trace_hook = hook;
}

void oracle_view_reset(bool fail_mutex, struct obs_video_info *main_mix_info)
{
	mutex_fails = fail_mutex;
	main_info = main_mix_info;
	memset(mixes, 0, sizeof(mixes));
	mix_num = 0;
}

size_t oracle_view_mix_num(void)
{
	return mix_num;
}

struct obs_view *oracle_view_mix_at(size_t i)
{
	return mixes[i].view;
}

obs_source_t *oracle_view_channel_at(struct obs_view *view, size_t channel)
{
	return view->channels[channel];
}

int oracle_view_type_of(struct obs_view *view)
{
	return (int)view->type;
}

/* obs-view.c accessors */

size_t obs_view_struct_size(void)
{
	return sizeof(struct obs_view);
}

bool obs_view_mutex_init(struct obs_view *view)
{
	trace("mutex_init", NULL, 0);
	(void)view;
	return !mutex_fails;
}

void obs_view_mutex_destroy(struct obs_view *view)
{
	(void)view;
	trace("mutex_destroy", NULL, 0);
}

void obs_view_lock(struct obs_view *view)
{
	(void)view;
	trace("lock", NULL, 0);
}

void obs_view_unlock(struct obs_view *view)
{
	(void)view;
	trace("unlock", NULL, 0);
}

enum view_type obs_view_get_type(const struct obs_view *view)
{
	return view->type;
}

void obs_view_set_type(struct obs_view *view, enum view_type type)
{
	trace("type", NULL, (long long)type);
	view->type = type;
}

obs_source_t *obs_view_channel(const struct obs_view *view, size_t channel)
{
	return view->channels[channel];
}

void obs_view_set_channel(struct obs_view *view, size_t channel, obs_source_t *source)
{
	trace("set", src_name(source), (long long)channel);
	view->channels[channel] = source;
}

void obs_view_clear_channels(struct obs_view *view)
{
	trace("clear", NULL, 0);
	memset(view->channels, 0, sizeof(view->channels));
}

bool obs_view_source_removed(const obs_source_t *source)
{
	return ((const struct oracle_view_source *)(const void *)source)->removed;
}

struct obs_video_info *obs_view_main_mix_info(void)
{
	return main_info;
}

video_t *obs_view_mix_add(struct obs_view *view, struct obs_video_info *ovi)
{
	trace("mix_add", NULL, (long long)mix_num);
	if (mix_num == ORACLE_VIEW_MIXES)
		return NULL;
	mixes[mix_num].view = view;
	mixes[mix_num].ovi = ovi;
	mix_num++;
	return (video_t *)(uintptr_t)(0x9000 + mix_num);
}

void obs_view_mixes_lock(void)
{
	trace("mixes_lock", NULL, 0);
}

void obs_view_mixes_unlock(void)
{
	trace("mixes_unlock", NULL, 0);
}

size_t obs_view_mix_count(void)
{
	return mix_num;
}

struct obs_view *obs_view_mix_view(size_t i)
{
	return mixes[i].view;
}

void obs_view_mix_clear_view(size_t i)
{
	trace("mix_clear", NULL, (long long)i);
	mixes[i].view = NULL;
}

struct obs_video_info *obs_view_mix_info(size_t i)
{
	return mixes[i].ovi;
}

/* obs-source.c calls */

obs_source_t *obs_source_get_ref(obs_source_t *source)
{
	trace("get_ref", src_name(source), 0);
	if (!source || ((struct oracle_view_source *)(void *)source)->dying)
		return NULL;
	return source;
}

void obs_source_release(obs_source_t *source)
{
	trace("release", src_name(source), 0);
}

void obs_source_video_render(obs_source_t *source)
{
	trace("render", src_name(source), 0);
}
