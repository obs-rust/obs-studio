/* The original shutdown detachment (the C reference) and the production
 * bridge's calls, over a real private source table. The source-table mutex,
 * the libobs source and scene calls and the destruction queue wait are
 * renamed to stand-ins that forward to the test's model, through hooks the
 * test installs for its thread. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>

struct oracle_shutdown_detach_hooks {
	void *model;
	void (*lock)(void *model);
	void (*unlock)(void *model);
	bool (*get_ref)(void *model, size_t source);
	bool (*scene_from_source)(void *model, size_t source);
	bool (*group_from_source)(void *model, size_t source);
	void (*enum_items)(void *model, size_t scene, obs_scene_t *scene_ptr,
			   bool (*callback)(obs_scene_t *, obs_sceneitem_t *, void *), void *param);
	void (*item_remove)(void *model, size_t item);
	void (*release)(void *model, size_t source);
	bool (*wait)(void *model);
};

struct oracle_shutdown_detach {
	struct obs_core core;
	struct obs_source *sources;
	size_t count;
	const struct oracle_shutdown_detach_hooks *hooks;
};

#if defined(_MSC_VER)
static __declspec(thread) struct oracle_shutdown_detach *shutdown_detach_current;
#else
static _Thread_local struct oracle_shutdown_detach *shutdown_detach_current;
#endif

static size_t shutdown_detach_index(const obs_source_t *source)
{
	struct oracle_shutdown_detach *f = shutdown_detach_current;
	return (size_t)(source - f->sources);
}

static int shutdown_detach_mutex_lock(pthread_mutex_t *mutex)
{
	struct oracle_shutdown_detach *f = shutdown_detach_current;
	if (mutex != &f->core.data.sources_mutex)
		abort();
	f->hooks->lock(f->hooks->model);
	return 0;
}

static int shutdown_detach_mutex_unlock(pthread_mutex_t *mutex)
{
	struct oracle_shutdown_detach *f = shutdown_detach_current;
	if (mutex != &f->core.data.sources_mutex)
		abort();
	f->hooks->unlock(f->hooks->model);
	return 0;
}

static obs_source_t *shutdown_detach_get_ref(obs_source_t *source)
{
	struct oracle_shutdown_detach *f = shutdown_detach_current;
	return f->hooks->get_ref(f->hooks->model, shutdown_detach_index(source)) ? source : NULL;
}

/* A scene or group is the source's own address, as an opaque handle. */
static obs_scene_t *shutdown_detach_scene_from_source(const obs_source_t *source)
{
	struct oracle_shutdown_detach *f = shutdown_detach_current;
	size_t index = shutdown_detach_index(source);
	return f->hooks->scene_from_source(f->hooks->model, index) ? (obs_scene_t *)&f->sources[index] : NULL;
}

static obs_scene_t *shutdown_detach_group_from_source(const obs_source_t *source)
{
	struct oracle_shutdown_detach *f = shutdown_detach_current;
	size_t index = shutdown_detach_index(source);
	return f->hooks->group_from_source(f->hooks->model, index) ? (obs_scene_t *)&f->sources[index] : NULL;
}

static void shutdown_detach_enum_items(obs_scene_t *scene, bool (*callback)(obs_scene_t *, obs_sceneitem_t *, void *),
				       void *param)
{
	struct oracle_shutdown_detach *f = shutdown_detach_current;
	f->hooks->enum_items(f->hooks->model, shutdown_detach_index((obs_source_t *)scene), scene, callback, param);
}

/* An item is an opaque token the model hands out. */
static void shutdown_detach_item_remove(obs_sceneitem_t *item)
{
	struct oracle_shutdown_detach *f = shutdown_detach_current;
	f->hooks->item_remove(f->hooks->model, (size_t)(uintptr_t)item);
}

static void shutdown_detach_release(obs_source_t *source)
{
	struct oracle_shutdown_detach *f = shutdown_detach_current;
	f->hooks->release(f->hooks->model, shutdown_detach_index(source));
}

static bool shutdown_detach_queue_wait(os_task_queue_t *queue)
{
	struct oracle_shutdown_detach *f = shutdown_detach_current;
	if (queue != f->core.destruction_task_thread)
		abort();
	return f->hooks->wait(f->hooks->model);
}

#define obs (&shutdown_detach_current->core)
/* winpthreads defines these as macros: keep them for after. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock shutdown_detach_mutex_lock
#define pthread_mutex_unlock shutdown_detach_mutex_unlock
#define obs_source_get_ref shutdown_detach_get_ref
#define obs_scene_from_source shutdown_detach_scene_from_source
#define obs_group_from_source shutdown_detach_group_from_source
#define obs_scene_enum_items shutdown_detach_enum_items
#define obs_sceneitem_remove shutdown_detach_item_remove
#define obs_source_release shutdown_detach_release
#define os_task_queue_wait shutdown_detach_queue_wait
void oracle_obs_shutdown_detach_scene_items_run(struct obs_core_data *data,
						const struct obs_shutdown_detach_calls *calls);
#define obs_shutdown_detach_scene_items_run oracle_obs_shutdown_detach_scene_items_run
#include "libobs/obs-shutdown-detach-reference.c.h"
#undef obs_shutdown_detach_scene_items_run
#include "libobs/obs-shutdown-detach.c.h"
#undef os_task_queue_wait
#undef obs_source_release
#undef obs_sceneitem_remove
#undef obs_scene_enum_items
#undef obs_group_from_source
#undef obs_scene_from_source
#undef obs_source_get_ref
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")
#undef obs

void *oracle_shutdown_detach_new(size_t count, const size_t *order);
void oracle_shutdown_detach_free(void *fixture);
void *oracle_shutdown_detach_data(void *fixture);
const void *oracle_shutdown_detach_calls(void);
void oracle_shutdown_detach_install(void *fixture, const struct oracle_shutdown_detach_hooks *hooks);
void oracle_shutdown_detach_original(void *fixture);

/* A core whose UUID source table holds count sources, inserted in order
 * (each index once): the table walk visits them in that order. */
void *oracle_shutdown_detach_new(size_t count, const size_t *order)
{
	struct oracle_shutdown_detach *f = calloc(1, sizeof(*f));
	if (!f)
		return NULL;
	f->sources = calloc(count ? count : 1, sizeof(*f->sources));
	if (!f->sources) {
		free(f);
		return NULL;
	}
	f->count = count;
	/* Any non-NULL handle: the stand-in wait only checks it is the one
	 * read from the core. */
	f->core.destruction_task_thread = (os_task_queue_t *)f;
	for (size_t i = 0; i < count; i++) {
		struct obs_context_data *ctx = &f->sources[order[i]].context;
		char *uuid = malloc(UUID_STR_LENGTH + 1);
		if (!uuid)
			abort();
		snprintf(uuid, UUID_STR_LENGTH + 1, "00000000-0000-4000-8000-%012u", (unsigned)order[i]);
		ctx->uuid = uuid;
		HASH_ADD_UUID(*(struct obs_context_data **)&f->core.data.sources, uuid, ctx);
	}
	return f;
}

void oracle_shutdown_detach_free(void *fixture)
{
	struct oracle_shutdown_detach *f = fixture;
	HASH_CLEAR(hh_uuid, *(struct obs_context_data **)&f->core.data.sources);
	for (size_t i = 0; i < f->count; i++)
		free((void *)(uintptr_t)f->sources[i].context.uuid);
	free(f->sources);
	free(f);
}

void *oracle_shutdown_detach_data(void *fixture)
{
	struct oracle_shutdown_detach *f = fixture;
	return &f->core.data;
}

/* The production bridge's calls, over the stand-ins. */
const void *oracle_shutdown_detach_calls(void)
{
	return &shutdown_detach_calls;
}

/* Route this thread's stand-ins to hooks over fixture; NULL clears it. */
void oracle_shutdown_detach_install(void *fixture, const struct oracle_shutdown_detach_hooks *hooks)
{
	struct oracle_shutdown_detach *f = fixture;
	if (f)
		f->hooks = hooks;
	shutdown_detach_current = f;
}

void oracle_shutdown_detach_original(void *fixture)
{
	struct oracle_shutdown_detach *f = fixture;
	oracle_obs_shutdown_detach_scene_items_run(&f->core.data, NULL);
}

void oracle_shutdown_detach_calls_layout(size_t out[12]);
void oracle_shutdown_detach_hooks_layout(size_t out[12]);

/* struct obs_shutdown_detach_calls from the real private header: its size,
 * alignment, then each member's offset in declaration order. */
void oracle_shutdown_detach_calls_layout(size_t out[12])
{
	out[0] = sizeof(struct obs_shutdown_detach_calls);
	out[1] = _Alignof(struct obs_shutdown_detach_calls);
	out[2] = offsetof(struct obs_shutdown_detach_calls, lock);
	out[3] = offsetof(struct obs_shutdown_detach_calls, unlock);
	out[4] = offsetof(struct obs_shutdown_detach_calls, each_source);
	out[5] = offsetof(struct obs_shutdown_detach_calls, get_ref);
	out[6] = offsetof(struct obs_shutdown_detach_calls, scene_from_source);
	out[7] = offsetof(struct obs_shutdown_detach_calls, group_from_source);
	out[8] = offsetof(struct obs_shutdown_detach_calls, enum_items);
	out[9] = offsetof(struct obs_shutdown_detach_calls, item_remove);
	out[10] = offsetof(struct obs_shutdown_detach_calls, release);
	out[11] = offsetof(struct obs_shutdown_detach_calls, wait_destruction);
}

/* struct oracle_shutdown_detach_hooks, the same way. */
void oracle_shutdown_detach_hooks_layout(size_t out[12])
{
	out[0] = sizeof(struct oracle_shutdown_detach_hooks);
	out[1] = _Alignof(struct oracle_shutdown_detach_hooks);
	out[2] = offsetof(struct oracle_shutdown_detach_hooks, model);
	out[3] = offsetof(struct oracle_shutdown_detach_hooks, lock);
	out[4] = offsetof(struct oracle_shutdown_detach_hooks, unlock);
	out[5] = offsetof(struct oracle_shutdown_detach_hooks, get_ref);
	out[6] = offsetof(struct oracle_shutdown_detach_hooks, scene_from_source);
	out[7] = offsetof(struct oracle_shutdown_detach_hooks, group_from_source);
	out[8] = offsetof(struct oracle_shutdown_detach_hooks, enum_items);
	out[9] = offsetof(struct oracle_shutdown_detach_hooks, item_remove);
	out[10] = offsetof(struct oracle_shutdown_detach_hooks, release);
	out[11] = offsetof(struct oracle_shutdown_detach_hooks, wait);
}
