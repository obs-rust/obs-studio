/* Tier 3: the unchanged extracted obs_source_set_video_frame_internal()
 * (libobs/obs-source-set-video-frame-internal.c.h) against the Rust export,
 * over real private source layouts.
 *
 * The Rust export calls two sets of production adapters: the preload
 * frame's shapes, destroy, create and copy, async_mutex and the
 * last_frame_ts write of obs-source-preload-video-internal-fields.c.h,
 * compiled once in source_preload_video_internal.c with its recorders, and
 * the graphics context, the async texture size and update and the stale
 * flag store of obs-source-set-video-frame-internal-fields.c.h, compiled
 * here. The original is built here with every one of those calls routed
 * through recorders of the same shape (the create allocates a real frame of
 * the asked shape, the destroy frees it, the copy copies nothing, the mutex
 * and the flag store do the real work, the graphics and texture calls only
 * record). Each recorder here first moves what source_preload_video_internal.c
 * recorded since into this log, so the log holds every call of both sets in
 * the order they ran, and the parity test compares that sequence as well as
 * the state left. */
#define _NO_CRT_STDIO_INLINE
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

/* The first six are source_preload_video_internal.c's own kinds. */
enum {
	SVF_LOCK = 1,
	SVF_UNLOCK = 2,
	SVF_CREATE = 4,
	SVF_DESTROY = 5,
	SVF_COPY = 6,
	SVF_ENTER = 7,
	SVF_LEAVE = 8,
	SVF_SIZE = 9,
	SVF_UPDATE = 10,
	SVF_SET_BOOL = 11,
};

#define SVF_MAX_EVENTS 64

struct svf_event {
	int kind;
	const void *p1;
	const void *p2;
	const void *p3;
	uint64_t a;
	uint64_t b;
	uint64_t c;
};

static struct svf_event svf_events[SVF_MAX_EVENTS];
static size_t svf_event_count;

static void svf_push(struct svf_event event)
{
	if (svf_event_count < SVF_MAX_EVENTS)
		svf_events[svf_event_count] = event;
	svf_event_count++;
}

/* source_preload_video_internal.c's recorded calls since the last read. */
size_t oracle_pvi_events(int *kinds, const void **ptrs, uint64_t *nums, size_t max);

static void svf_take_preload_events(void)
{
	int kinds[SVF_MAX_EVENTS];
	const void *ptrs[SVF_MAX_EVENTS * 2];
	uint64_t nums[SVF_MAX_EVENTS * 3];
	size_t count = oracle_pvi_events(kinds, ptrs, nums, SVF_MAX_EVENTS);
	for (size_t i = 0; i < count && i < SVF_MAX_EVENTS; i++)
		svf_push((struct svf_event){kinds[i], ptrs[i * 2], ptrs[i * 2 + 1], NULL, nums[i * 3], nums[i * 3 + 1],
					    nums[i * 3 + 2]});
	/* Overflow there shows as overflow here. */
	for (size_t i = SVF_MAX_EVENTS; i < count; i++)
		svf_event_count++;
}

static void svf_record(int kind, const void *p1, const void *p2, const void *p3, uint64_t a, uint64_t b, uint64_t c)
{
	svf_take_preload_events();
	svf_push((struct svf_event){kind, p1, p2, p3, a, b, c});
}

static int svf_mutex_lock(pthread_mutex_t *mutex)
{
	svf_record(SVF_LOCK, mutex, NULL, NULL, 0, 0, 0);
	return pthread_mutex_lock(mutex);
}

static int svf_mutex_unlock(pthread_mutex_t *mutex)
{
	svf_record(SVF_UNLOCK, mutex, NULL, NULL, 0, 0, 0);
	return pthread_mutex_unlock(mutex);
}

/* The created frame is the second pointer; the shape asked for, as given. */
static struct obs_source_frame *svf_frame_create(enum video_format format, uint32_t width, uint32_t height)
{
	struct obs_source_frame *frame = bzalloc(sizeof(*frame));
	frame->format = format;
	frame->width = width;
	frame->height = height;
	svf_record(SVF_CREATE, NULL, frame, NULL, (uint64_t)(int64_t)format, width, height);
	return frame;
}

static void svf_frame_destroy(struct obs_source_frame *frame)
{
	svf_record(SVF_DESTROY, frame, NULL, NULL, 0, 0, 0);
	bfree(frame);
}

/* Records the copy with the frame's timestamp; copies nothing. */
static void svf_frame_copy(struct obs_source_frame *dst, const struct obs_source_frame *src)
{
	svf_record(SVF_COPY, dst, src, NULL, src->timestamp, 0, 0);
}

static void svf_enter_graphics(void)
{
	svf_record(SVF_ENTER, NULL, NULL, NULL, 0, 0, 0);
}

static void svf_leave_graphics(void)
{
	svf_record(SVF_LEAVE, NULL, NULL, NULL, 0, 0, 0);
}

static bool svf_set_async_texture_size(struct obs_source *source, const struct obs_source_frame *frame)
{
	svf_record(SVF_SIZE, source, frame, NULL, 0, 0, 0);
	return true;
}

static bool svf_update_async_textures(struct obs_source *source, const struct obs_source_frame *frame,
				      gs_texture_t *tex[MAX_AV_PLANES], gs_texrender_t *texrender)
{
	svf_record(SVF_UPDATE, source, frame, tex, (uint64_t)(uintptr_t)texrender, 0, 0);
	return true;
}

static void svf_set_bool(volatile bool *ptr, bool val)
{
	svf_record(SVF_SET_BOOL, (const void *)(uintptr_t)ptr, NULL, NULL, val, 0, 0);
	os_atomic_set_bool(ptr, val);
}

#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock svf_mutex_lock
#define pthread_mutex_unlock svf_mutex_unlock
#define obs_source_frame_create svf_frame_create
#define obs_source_frame_destroy svf_frame_destroy
#define obs_source_frame_copy svf_frame_copy
#define obs_enter_graphics svf_enter_graphics
#define obs_leave_graphics svf_leave_graphics
#define set_async_texture_size svf_set_async_texture_size
#define update_async_textures svf_update_async_textures
#define os_atomic_set_bool svf_set_bool

#define obs_source_set_video_frame_internal oracle_obs_source_set_video_frame_internal
void oracle_obs_source_set_video_frame_internal(obs_source_t *source, const struct obs_source_frame *frame);
#include "source-set-video-frame-internal.inc"
#undef obs_source_set_video_frame_internal

#include "source-set-video-frame-internal-fields.inc"

#undef os_atomic_set_bool
#undef update_async_textures
#undef set_async_texture_size
#undef obs_leave_graphics
#undef obs_enter_graphics
#undef obs_source_frame_copy
#undef obs_source_frame_destroy
#undef obs_source_frame_create
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

/* The calls recorded since the last read, here and in
 * source_preload_video_internal.c, oldest first; returns the count (more
 * than max when some were dropped) and clears the log. Each event is its
 * kind, three pointers and three numbers. */
size_t oracle_svf_events(int *kinds, const void **ptrs, uint64_t *nums, size_t max);
size_t oracle_svf_events(int *kinds, const void **ptrs, uint64_t *nums, size_t max)
{
	svf_take_preload_events();
	size_t count = svf_event_count;
	for (size_t i = 0; i < count && i < max && i < SVF_MAX_EVENTS; i++) {
		kinds[i] = svf_events[i].kind;
		ptrs[i * 3] = svf_events[i].p1;
		ptrs[i * 3 + 1] = svf_events[i].p2;
		ptrs[i * 3 + 2] = svf_events[i].p3;
		nums[i * 3] = svf_events[i].a;
		nums[i * 3 + 1] = svf_events[i].b;
		nums[i * 3 + 2] = svf_events[i].c;
	}
	svf_event_count = 0;
	return count;
}

struct svf_world {
	struct obs_source source;
	/* A stand-in the recorders only compare, never use. */
	char texrender;
};

/* A source as obs_source_init() leaves its async_mutex, with the given
 * state; with `preload`, a preload frame of the given shape; with
 * `texrender`, the world's stand-in as async_texrender. */
struct obs_source *oracle_svf_source_new(bool preload, int format, uint32_t width, uint32_t height,
					 uint64_t last_frame_ts, bool stale, bool texrender);
struct obs_source *oracle_svf_source_new(bool preload, int format, uint32_t width, uint32_t height,
					 uint64_t last_frame_ts, bool stale, bool texrender)
{
	struct svf_world *world = bzalloc(sizeof(*world));
	struct obs_source *source = &world->source;
	if (pthread_mutex_init_recursive(&source->async_mutex) != 0) {
		bfree(world);
		return NULL;
	}
	source->last_frame_ts = last_frame_ts;
	source->async_textures_stale = stale;
	source->async_texrender = texrender ? (gs_texrender_t *)&world->texrender : NULL;
	if (preload) {
		struct obs_source_frame *frame = bzalloc(sizeof(*frame));
		frame->format = (enum video_format)format;
		frame->width = width;
		frame->height = height;
		source->async_preload_frame = frame;
	}
	return source;
}

void oracle_svf_source_free(struct obs_source *source);
void oracle_svf_source_free(struct obs_source *source)
{
	pthread_mutex_destroy(&source->async_mutex);
	bfree(source->async_preload_frame);
	bfree(source);
}

/* The addresses the recorders see, for the roles: async_mutex, the preload
 * frame, async_textures, async_textures_stale and the texrender stand-in. */
void oracle_svf_addresses(const struct obs_source *source, const void **out);
void oracle_svf_addresses(const struct obs_source *source, const void **out)
{
	const struct svf_world *world = (const struct svf_world *)source;
	out[0] = &source->async_mutex;
	out[1] = source->async_preload_frame;
	out[2] = (const void *)source->async_textures;
	out[3] = (const void *)(uintptr_t)&source->async_textures_stale;
	out[4] = &world->texrender;
}

/* The state the call may change: the preload frame, its shape,
 * last_frame_ts and async_textures_stale. */
void oracle_svf_state(const struct obs_source *source, const void **preload, int *format, uint32_t *width,
		      uint32_t *height, uint64_t *last_frame_ts, bool *stale);
void oracle_svf_state(const struct obs_source *source, const void **preload, int *format, uint32_t *width,
		      uint32_t *height, uint64_t *last_frame_ts, bool *stale)
{
	const struct obs_source_frame *frame = source->async_preload_frame;
	*preload = frame;
	*format = frame ? (int)frame->format : 0;
	*width = frame ? frame->width : 0;
	*height = frame ? frame->height : 0;
	*last_frame_ts = source->last_frame_ts;
	*stale = source->async_textures_stale;
}

/* The diagnostics logged between begin and end. */
struct svf_log {
	unsigned count;
	int level;
	char message[256];
};

static struct svf_log svf_log;
static log_handler_t svf_previous;
static void *svf_previous_param;

#if defined(__GNUC__) || defined(__clang__)
static void svf_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void svf_capture(int level, const char *format, va_list args, void *param)
{
	struct svf_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*svf_get_log)(log_handler_t *, void **);
typedef void (*svf_set_log)(log_handler_t, void *);

void oracle_svf_log_begin(svf_get_log get_log, svf_set_log set_log);
void oracle_svf_log_begin(svf_get_log get_log, svf_set_log set_log)
{
	memset(&svf_log, 0, sizeof(svf_log));
	get_log(&svf_previous, &svf_previous_param);
	set_log(svf_capture, &svf_log);
}

/* Restores logging; returns the count, with the last level and message. */
unsigned oracle_svf_log_end(svf_set_log set_log, int *level, char *message, size_t size);
unsigned oracle_svf_log_end(svf_set_log set_log, int *level, char *message, size_t size)
{
	set_log(svf_previous, svf_previous_param);
	*level = svf_log.level;
	snprintf(message, size, "%s", svf_log.message);
	return svf_log.count;
}
