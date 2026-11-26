/* Tier 3: the unchanged extracted obs_source_get_frame() and
 * obs_source_release_frame() (libobs/obs-source-get-release-frame.c.h)
 * against the Rust exports, over real private source and frame layouts.
 *
 * The production adapters the Rust exports call
 * (libobs/obs-source-get-release-frame-fields.c.h, remove_async_frame()
 * included) are compiled here, once for every test binary, and both sides
 * share them: the original calls remove_async_frame() directly. Both are
 * built with the async_mutex lock and unlock and obs_source_frame_destroy()
 * routed through recorders that do the real work and log each call, so the
 * parity test compares the call sequence as well as the state left. */
#define _NO_CRT_STDIO_INLINE
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

enum {
	GRF_LOCK = 1,
	GRF_UNLOCK = 2,
	GRF_DESTROY = 3,
};

#define GRF_MAX_EVENTS 256

struct grf_event {
	int kind;
	const void *arg;
};

static struct grf_event grf_events[GRF_MAX_EVENTS];
static size_t grf_event_count;

static void grf_record(int kind, const void *arg)
{
	if (grf_event_count < GRF_MAX_EVENTS)
		grf_events[grf_event_count] = (struct grf_event){kind, arg};
	grf_event_count++;
}

static int grf_mutex_lock(pthread_mutex_t *mutex)
{
	grf_record(GRF_LOCK, mutex);
	return pthread_mutex_lock(mutex);
}

static int grf_mutex_unlock(pthread_mutex_t *mutex)
{
	grf_record(GRF_UNLOCK, mutex);
	return pthread_mutex_unlock(mutex);
}

static void grf_frame_destroy(struct obs_source_frame *frame)
{
	grf_record(GRF_DESTROY, frame);
	obs_source_frame_destroy(frame);
}

#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock grf_mutex_lock
#define pthread_mutex_unlock grf_mutex_unlock
#define obs_source_frame_destroy grf_frame_destroy

#define obs_source_get_frame oracle_obs_source_get_frame
#define obs_source_release_frame oracle_obs_source_release_frame
struct obs_source_frame *oracle_obs_source_get_frame(obs_source_t *source);
void oracle_obs_source_release_frame(obs_source_t *source, struct obs_source_frame *frame);
#include "source-get-release-frame.inc"
#undef obs_source_get_frame
#undef obs_source_release_frame

#include "source-get-release-frame-fields.inc"

#undef obs_source_frame_destroy
#undef pthread_mutex_lock
#undef pthread_mutex_unlock

/* The calls recorded since the last read, oldest first; returns the count
 * (more than max when some were dropped) and clears the log. */
size_t oracle_grf_events(int *kinds, const void **args, size_t max);
size_t oracle_grf_events(int *kinds, const void **args, size_t max)
{
	size_t count = grf_event_count;
	for (size_t i = 0; i < count && i < max && i < GRF_MAX_EVENTS; i++) {
		kinds[i] = grf_events[i].kind;
		args[i] = grf_events[i].arg;
	}
	grf_event_count = 0;
	return count;
}

void *oracle_grf_source_new(void);
void *oracle_grf_source_new(void)
{
	struct obs_source *source = bzalloc(sizeof(*source));
	/* As obs_source_init(). */
	if (pthread_mutex_init_recursive(&source->async_mutex) != 0) {
		bfree(source);
		return NULL;
	}
	return source;
}

void oracle_grf_source_free(struct obs_source *source);
void oracle_grf_source_free(struct obs_source *source)
{
	da_free(source->async_cache);
	pthread_mutex_destroy(&source->async_mutex);
	bfree(source);
}

const void *oracle_grf_mutex(const struct obs_source *source);
const void *oracle_grf_mutex(const struct obs_source *source)
{
	return &source->async_mutex;
}

void oracle_grf_set_current(struct obs_source *source, struct obs_source_frame *frame);
void oracle_grf_set_current(struct obs_source *source, struct obs_source_frame *frame)
{
	source->cur_async_frame = frame;
}

struct obs_source_frame *oracle_grf_current(const struct obs_source *source);
struct obs_source_frame *oracle_grf_current(const struct obs_source *source)
{
	return source->cur_async_frame;
}

void oracle_grf_cache_push(struct obs_source *source, struct obs_source_frame *frame, bool used, long unused_count);
void oracle_grf_cache_push(struct obs_source *source, struct obs_source_frame *frame, bool used, long unused_count)
{
	struct async_frame entry = {frame, unused_count, used};
	da_push_back(source->async_cache, &entry);
}

/* Slot index's frame, used flag and unused count; false past the end. */
bool oracle_grf_cache_slot(const struct obs_source *source, size_t index, const void **frame, bool *used,
			   long *unused_count);
bool oracle_grf_cache_slot(const struct obs_source *source, size_t index, const void **frame, bool *used,
			   long *unused_count)
{
	if (index >= source->async_cache.num)
		return false;
	*frame = source->async_cache.array[index].frame;
	*used = source->async_cache.array[index].used;
	*unused_count = source->async_cache.array[index].unused_count;
	return true;
}

/* Two allocations, as obs_source_frame_create(): the frame and its data. */
struct obs_source_frame *oracle_grf_frame_new(long refs, bool prev_frame);
struct obs_source_frame *oracle_grf_frame_new(long refs, bool prev_frame)
{
	struct obs_source_frame *frame = bzalloc(sizeof(*frame));
	frame->data[0] = bmalloc(16);
	frame->refs = refs;
	frame->prev_frame = prev_frame;
	return frame;
}

void oracle_grf_frame_free(struct obs_source_frame *frame);
void oracle_grf_frame_free(struct obs_source_frame *frame)
{
	bfree(frame->data[0]);
	bfree(frame);
}

long oracle_grf_frame_refs(const struct obs_source_frame *frame);
long oracle_grf_frame_refs(const struct obs_source_frame *frame)
{
	return frame->refs;
}

bool oracle_grf_frame_prev(const struct obs_source_frame *frame);
bool oracle_grf_frame_prev(const struct obs_source_frame *frame)
{
	return frame->prev_frame;
}

/* The diagnostics logged between begin and end. */
struct grf_log {
	unsigned count;
	int level;
	char message[256];
};

static struct grf_log grf_log;
static log_handler_t grf_previous;
static void *grf_previous_param;

#if defined(__GNUC__) || defined(__clang__)
static void grf_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void grf_capture(int level, const char *format, va_list args, void *param)
{
	struct grf_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*grf_get_log)(log_handler_t *, void **);
typedef void (*grf_set_log)(log_handler_t, void *);

void oracle_grf_log_begin(grf_get_log get_log, grf_set_log set_log);
void oracle_grf_log_begin(grf_get_log get_log, grf_set_log set_log)
{
	memset(&grf_log, 0, sizeof(grf_log));
	get_log(&grf_previous, &grf_previous_param);
	set_log(grf_capture, &grf_log);
}

/* Restores logging; returns the count, with the last level and message. */
unsigned oracle_grf_log_end(grf_set_log set_log, int *level, char *message, size_t size);
unsigned oracle_grf_log_end(grf_set_log set_log, int *level, char *message, size_t size)
{
	set_log(grf_previous, grf_previous_param);
	*level = grf_log.level;
	snprintf(message, size, "%s", grf_log.message);
	return grf_log.count;
}

/* For a mutant: drops one reference behind the export. */
void oracle_grf_frame_drop_ref(struct obs_source_frame *frame);
void oracle_grf_frame_drop_ref(struct obs_source_frame *frame)
{
	frame->refs--;
}
