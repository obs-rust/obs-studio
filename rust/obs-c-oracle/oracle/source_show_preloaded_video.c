/* Tier 3: the unchanged extracted obs_source_show_preloaded_video()
 * (libobs/obs-source-show-preloaded-video.c.h) against the Rust export,
 * over real private source layouts.
 *
 * The production adapters the Rust export calls
 * (libobs/obs-source-show-preloaded-video-fields.c.h) are compiled here,
 * once for every test binary. Both they and the original are built with the
 * two mutexes, the graphics context, the async texture upload, the two flag
 * stores, the clock and the two audio resets routed through recorders that
 * log each call with its arguments (the mutexes and flag stores also do the
 * real work; the clock answers a value the test sets), so the parity test
 * compares the call sequence as well as the state left. */
#define _NO_CRT_STDIO_INLINE
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

enum {
	SPV_LOCK = 1,
	SPV_UNLOCK = 2,
	SPV_ENTER = 3,
	SPV_LEAVE = 4,
	SPV_SIZE = 5,
	SPV_UPDATE = 6,
	SPV_SET_BOOL = 7,
	SPV_NOW = 8,
	SPV_TIMING = 9,
	SPV_DATA = 10,
};

#define SPV_MAX_EVENTS 64

struct spv_event {
	int kind;
	const void *p1;
	const void *p2;
	const void *p3;
	uint64_t a;
	uint64_t b;
};

static struct spv_event spv_events[SPV_MAX_EVENTS];
static size_t spv_event_count;
static uint64_t spv_clock;

static void spv_record(int kind, const void *p1, const void *p2, const void *p3, uint64_t a, uint64_t b)
{
	if (spv_event_count < SPV_MAX_EVENTS)
		spv_events[spv_event_count] = (struct spv_event){kind, p1, p2, p3, a, b};
	spv_event_count++;
}

static int spv_mutex_lock(pthread_mutex_t *mutex)
{
	spv_record(SPV_LOCK, mutex, NULL, NULL, 0, 0);
	return pthread_mutex_lock(mutex);
}

static int spv_mutex_unlock(pthread_mutex_t *mutex)
{
	spv_record(SPV_UNLOCK, mutex, NULL, NULL, 0, 0);
	return pthread_mutex_unlock(mutex);
}

static void spv_enter_graphics(void)
{
	spv_record(SPV_ENTER, NULL, NULL, NULL, 0, 0);
}

static void spv_leave_graphics(void)
{
	spv_record(SPV_LEAVE, NULL, NULL, NULL, 0, 0);
}

static bool spv_set_async_texture_size(struct obs_source *source, const struct obs_source_frame *frame)
{
	spv_record(SPV_SIZE, source, frame, NULL, 0, 0);
	return true;
}

static bool spv_update_async_textures(struct obs_source *source, const struct obs_source_frame *frame,
				      gs_texture_t *tex[MAX_AV_PLANES], gs_texrender_t *texrender)
{
	spv_record(SPV_UPDATE, source, frame, tex, (uint64_t)(uintptr_t)texrender, 0);
	return true;
}

static void spv_set_bool(volatile bool *ptr, bool val)
{
	spv_record(SPV_SET_BOOL, (const void *)(uintptr_t)ptr, NULL, NULL, val, 0);
	os_atomic_set_bool(ptr, val);
}

static uint64_t spv_gettime_ns(void)
{
	spv_record(SPV_NOW, NULL, NULL, NULL, spv_clock, 0);
	return spv_clock;
}

static void spv_reset_audio_timing(obs_source_t *source, uint64_t timestamp, uint64_t os_time)
{
	spv_record(SPV_TIMING, source, NULL, NULL, timestamp, os_time);
}

static void spv_reset_audio_data_at_sys_time(obs_source_t *source, uint64_t os_time)
{
	spv_record(SPV_DATA, source, NULL, NULL, os_time, 0);
}

#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock spv_mutex_lock
#define pthread_mutex_unlock spv_mutex_unlock
#define obs_enter_graphics spv_enter_graphics
#define obs_leave_graphics spv_leave_graphics
#define set_async_texture_size spv_set_async_texture_size
#define update_async_textures spv_update_async_textures
#define os_atomic_set_bool spv_set_bool
#define os_gettime_ns spv_gettime_ns
#define reset_audio_timing spv_reset_audio_timing
#define reset_audio_data_at_sys_time spv_reset_audio_data_at_sys_time

#define obs_source_show_preloaded_video oracle_obs_source_show_preloaded_video
void oracle_obs_source_show_preloaded_video(obs_source_t *source);
#include "source-show-preloaded-video.inc"
#undef obs_source_show_preloaded_video

#include "source-show-preloaded-video-fields.inc"

#undef reset_audio_data_at_sys_time
#undef reset_audio_timing
#undef os_gettime_ns
#undef os_atomic_set_bool
#undef update_async_textures
#undef set_async_texture_size
#undef obs_leave_graphics
#undef obs_enter_graphics
#undef pthread_mutex_lock
#undef pthread_mutex_unlock

/* The calls recorded since the last read, oldest first; returns the count
 * (more than max when some were dropped) and clears the log. Each event is
 * its kind, three pointers and two numbers. */
size_t oracle_spv_events(int *kinds, const void **ptrs, uint64_t *nums, size_t max);
size_t oracle_spv_events(int *kinds, const void **ptrs, uint64_t *nums, size_t max)
{
	size_t count = spv_event_count;
	for (size_t i = 0; i < count && i < max && i < SPV_MAX_EVENTS; i++) {
		kinds[i] = spv_events[i].kind;
		ptrs[i * 3] = spv_events[i].p1;
		ptrs[i * 3 + 1] = spv_events[i].p2;
		ptrs[i * 3 + 2] = spv_events[i].p3;
		nums[i * 2] = spv_events[i].a;
		nums[i * 2 + 1] = spv_events[i].b;
	}
	spv_event_count = 0;
	return count;
}

/* The value the next clock reads answer. */
void oracle_spv_set_clock(uint64_t now);
void oracle_spv_set_clock(uint64_t now)
{
	spv_clock = now;
}

struct spv_world {
	struct obs_source source;
	struct obs_source_frame frame;
	/* Stand-ins the recorders only compare, never use. */
	char textures[MAX_AV_PLANES];
	char texrender;
};

/* A source as obs_source_init() leaves its two mutexes, with the given
 * state; the preload frame and texture handles are the world's own. */
struct obs_source *oracle_spv_source_new(bool destroying, bool preload, int monitoring_type, uint64_t last_frame_ts,
					 bool stale, bool active);
struct obs_source *oracle_spv_source_new(bool destroying, bool preload, int monitoring_type, uint64_t last_frame_ts,
					 bool stale, bool active)
{
	struct spv_world *world = bzalloc(sizeof(*world));
	struct obs_source *source = &world->source;
	if (pthread_mutex_init_recursive(&source->async_mutex) != 0) {
		bfree(world);
		return NULL;
	}
	if (pthread_mutex_init_recursive(&source->audio_buf_mutex) != 0) {
		pthread_mutex_destroy(&source->async_mutex);
		bfree(world);
		return NULL;
	}
	source->destroying = destroying;
	source->async_preload_frame = preload ? &world->frame : NULL;
	source->monitoring_type = (enum obs_monitoring_type)monitoring_type;
	source->last_frame_ts = last_frame_ts;
	source->async_textures_stale = stale;
	source->async_active = active;
	for (size_t i = 0; i < MAX_AV_PLANES; i++)
		source->async_textures[i] = (gs_texture_t *)&world->textures[i];
	source->async_texrender = (gs_texrender_t *)&world->texrender;
	return source;
}

void oracle_spv_source_free(struct obs_source *source);
void oracle_spv_source_free(struct obs_source *source)
{
	pthread_mutex_destroy(&source->async_mutex);
	pthread_mutex_destroy(&source->audio_buf_mutex);
	bfree(source);
}

/* The addresses the recorders may report, by role: the two mutexes, the
 * world's frame, the texture array, the texrender and the two flags. */
void oracle_spv_addresses(const struct obs_source *source, const void **out);
void oracle_spv_addresses(const struct obs_source *source, const void **out)
{
	const struct spv_world *world = (const struct spv_world *)source;
	out[0] = &source->async_mutex;
	out[1] = &source->audio_buf_mutex;
	out[2] = &world->frame;
	out[3] = source->async_textures;
	out[4] = &world->texrender;
	out[5] = (const void *)(uintptr_t)&source->async_textures_stale;
	out[6] = (const void *)(uintptr_t)&source->async_active;
}

/* The state the call may change: the two flags and the preload frame. */
void oracle_spv_state(const struct obs_source *source, bool *stale, bool *active, const void **preload);
void oracle_spv_state(const struct obs_source *source, bool *stale, bool *active, const void **preload)
{
	*stale = source->async_textures_stale;
	*active = source->async_active;
	*preload = source->async_preload_frame;
}

/* The diagnostics logged between begin and end. */
struct spv_log {
	unsigned count;
	int level;
	char message[256];
};

static struct spv_log spv_log;
static log_handler_t spv_previous;
static void *spv_previous_param;

#if defined(__GNUC__) || defined(__clang__)
static void spv_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void spv_capture(int level, const char *format, va_list args, void *param)
{
	struct spv_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*spv_get_log)(log_handler_t *, void **);
typedef void (*spv_set_log)(log_handler_t, void *);

void oracle_spv_log_begin(spv_get_log get_log, spv_set_log set_log);
void oracle_spv_log_begin(spv_get_log get_log, spv_set_log set_log)
{
	memset(&spv_log, 0, sizeof(spv_log));
	get_log(&spv_previous, &spv_previous_param);
	set_log(spv_capture, &spv_log);
}

/* Restores logging; returns the count, with the last level and message. */
unsigned oracle_spv_log_end(spv_set_log set_log, int *level, char *message, size_t size);
unsigned oracle_spv_log_end(spv_set_log set_log, int *level, char *message, size_t size)
{
	set_log(spv_previous, spv_previous_param);
	*level = spv_log.level;
	snprintf(message, size, "%s", spv_log.message);
	return spv_log.count;
}
