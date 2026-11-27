/* Tier 3: the unchanged extracted obs_source_preload_video_internal()
 * (libobs/obs-source-preload-video-internal.c.h) against the Rust export,
 * over real private source layouts.
 *
 * The production adapters the Rust export calls
 * (libobs/obs-source-preload-video-internal-fields.c.h) are compiled here,
 * once for every test binary. Both they and the original are built with
 * destroying(), the preload frame's create, destroy and copy and the
 * async_mutex lock and unlock routed through recorders that log each call
 * with its arguments (destroying() reads the real field, the create
 * allocates a real frame of the asked shape, the destroy frees it, and the
 * mutex calls do the real work), so the parity test compares the call
 * sequence as well as the state left. */
#define _NO_CRT_STDIO_INLINE
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

enum {
	PVI_LOCK = 1,
	PVI_UNLOCK = 2,
	PVI_DESTROYING = 3,
	PVI_CREATE = 4,
	PVI_DESTROY = 5,
	PVI_COPY = 6,
};

#define PVI_MAX_EVENTS 64

struct pvi_event {
	int kind;
	const void *p1;
	const void *p2;
	uint64_t a;
	uint64_t b;
	uint64_t c;
};

static struct pvi_event pvi_events[PVI_MAX_EVENTS];
static size_t pvi_event_count;

static void pvi_record(int kind, const void *p1, const void *p2, uint64_t a, uint64_t b, uint64_t c)
{
	if (pvi_event_count < PVI_MAX_EVENTS)
		pvi_events[pvi_event_count] = (struct pvi_event){kind, p1, p2, a, b, c};
	pvi_event_count++;
}

static int pvi_mutex_lock(pthread_mutex_t *mutex)
{
	pvi_record(PVI_LOCK, mutex, NULL, 0, 0, 0);
	return pthread_mutex_lock(mutex);
}

static int pvi_mutex_unlock(pthread_mutex_t *mutex)
{
	pvi_record(PVI_UNLOCK, mutex, NULL, 0, 0, 0);
	return pthread_mutex_unlock(mutex);
}

static bool pvi_destroying(const struct obs_source *source)
{
	pvi_record(PVI_DESTROYING, source, NULL, 0, 0, 0);
	return os_atomic_load_long(&source->destroying);
}

/* The created frame is the second pointer; the shape asked for, as given. */
static struct obs_source_frame *pvi_frame_create(enum video_format format, uint32_t width, uint32_t height)
{
	struct obs_source_frame *frame = bzalloc(sizeof(*frame));
	frame->format = format;
	frame->width = width;
	frame->height = height;
	pvi_record(PVI_CREATE, NULL, frame, (uint64_t)(int64_t)format, width, height);
	return frame;
}

static void pvi_frame_destroy(struct obs_source_frame *frame)
{
	pvi_record(PVI_DESTROY, frame, NULL, 0, 0, 0);
	bfree(frame);
}

/* Records the copy with the frame's timestamp; copies nothing. */
static void pvi_frame_copy(struct obs_source_frame *dst, const struct obs_source_frame *src)
{
	pvi_record(PVI_COPY, dst, src, src->timestamp, 0, 0);
}

#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock pvi_mutex_lock
#define pthread_mutex_unlock pvi_mutex_unlock
#define destroying pvi_destroying
#define obs_source_frame_create pvi_frame_create
#define obs_source_frame_destroy pvi_frame_destroy
#define obs_source_frame_copy pvi_frame_copy

#define obs_source_preload_video_internal oracle_obs_source_preload_video_internal
void oracle_obs_source_preload_video_internal(obs_source_t *source, const struct obs_source_frame *frame);
#include "source-preload-video-internal.inc"
#undef obs_source_preload_video_internal

#include "source-preload-video-internal-fields.inc"

#undef obs_source_frame_copy
#undef obs_source_frame_destroy
#undef obs_source_frame_create
#undef destroying
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

/* The calls recorded since the last read, oldest first; returns the count
 * (more than max when some were dropped) and clears the log. Each event is
 * its kind, two pointers and three numbers. */
size_t oracle_pvi_events(int *kinds, const void **ptrs, uint64_t *nums, size_t max);
size_t oracle_pvi_events(int *kinds, const void **ptrs, uint64_t *nums, size_t max)
{
	size_t count = pvi_event_count;
	for (size_t i = 0; i < count && i < max && i < PVI_MAX_EVENTS; i++) {
		kinds[i] = pvi_events[i].kind;
		ptrs[i * 2] = pvi_events[i].p1;
		ptrs[i * 2 + 1] = pvi_events[i].p2;
		nums[i * 3] = pvi_events[i].a;
		nums[i * 3 + 1] = pvi_events[i].b;
		nums[i * 3 + 2] = pvi_events[i].c;
	}
	pvi_event_count = 0;
	return count;
}

/* A source as obs_source_init() leaves its async_mutex, with the given
 * state; with `preload`, a preload frame of the given shape. */
struct obs_source *oracle_pvi_source_new(bool destroying, bool preload, int format, uint32_t width, uint32_t height,
					 uint64_t last_frame_ts);
struct obs_source *oracle_pvi_source_new(bool destroying, bool preload, int format, uint32_t width, uint32_t height,
					 uint64_t last_frame_ts)
{
	struct obs_source *source = bzalloc(sizeof(*source));
	if (pthread_mutex_init_recursive(&source->async_mutex) != 0) {
		bfree(source);
		return NULL;
	}
	source->destroying = destroying;
	source->last_frame_ts = last_frame_ts;
	if (preload) {
		struct obs_source_frame *frame = bzalloc(sizeof(*frame));
		frame->format = (enum video_format)format;
		frame->width = width;
		frame->height = height;
		source->async_preload_frame = frame;
	}
	return source;
}

void oracle_pvi_source_free(struct obs_source *source);
void oracle_pvi_source_free(struct obs_source *source)
{
	pthread_mutex_destroy(&source->async_mutex);
	bfree(source->async_preload_frame);
	bfree(source);
}

/* The source's async_mutex and preload frame, for the roles. */
void oracle_pvi_addresses(const struct obs_source *source, const void **out);
void oracle_pvi_addresses(const struct obs_source *source, const void **out)
{
	out[0] = &source->async_mutex;
	out[1] = source->async_preload_frame;
}

/* The state the call may change: the preload frame, its shape and
 * last_frame_ts. */
void oracle_pvi_state(const struct obs_source *source, const void **preload, int *format, uint32_t *width,
		      uint32_t *height, uint64_t *last_frame_ts);
void oracle_pvi_state(const struct obs_source *source, const void **preload, int *format, uint32_t *width,
		      uint32_t *height, uint64_t *last_frame_ts)
{
	const struct obs_source_frame *frame = source->async_preload_frame;
	*preload = frame;
	*format = frame ? (int)frame->format : 0;
	*width = frame ? frame->width : 0;
	*height = frame ? frame->height : 0;
	*last_frame_ts = source->last_frame_ts;
}

/* A frame of the given shape and timestamp, for the caller to free. */
struct obs_source_frame *oracle_pvi_frame_new(int format, uint32_t width, uint32_t height, uint64_t timestamp);
struct obs_source_frame *oracle_pvi_frame_new(int format, uint32_t width, uint32_t height, uint64_t timestamp)
{
	struct obs_source_frame *frame = bzalloc(sizeof(*frame));
	frame->format = (enum video_format)format;
	frame->width = width;
	frame->height = height;
	frame->timestamp = timestamp;
	return frame;
}

void oracle_pvi_frame_free(struct obs_source_frame *frame);
void oracle_pvi_frame_free(struct obs_source_frame *frame)
{
	bfree(frame);
}

/* The diagnostics logged between begin and end. */
struct pvi_log {
	unsigned count;
	int level;
	char message[256];
};

static struct pvi_log pvi_log;
static log_handler_t pvi_previous;
static void *pvi_previous_param;

#if defined(__GNUC__) || defined(__clang__)
static void pvi_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void pvi_capture(int level, const char *format, va_list args, void *param)
{
	struct pvi_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*pvi_get_log)(log_handler_t *, void **);
typedef void (*pvi_set_log)(log_handler_t, void *);

void oracle_pvi_log_begin(pvi_get_log get_log, pvi_set_log set_log);
void oracle_pvi_log_begin(pvi_get_log get_log, pvi_set_log set_log)
{
	memset(&pvi_log, 0, sizeof(pvi_log));
	get_log(&pvi_previous, &pvi_previous_param);
	set_log(pvi_capture, &pvi_log);
}

/* Restores logging; returns the count, with the last level and message. */
unsigned oracle_pvi_log_end(pvi_set_log set_log, int *level, char *message, size_t size);
unsigned oracle_pvi_log_end(pvi_set_log set_log, int *level, char *message, size_t size)
{
	set_log(pvi_previous, pvi_previous_param);
	*level = pvi_log.level;
	snprintf(message, size, "%s", pvi_log.message);
	return pvi_log.count;
}
