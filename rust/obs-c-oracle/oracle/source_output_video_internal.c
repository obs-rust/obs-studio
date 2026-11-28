/* Tier 3: the unchanged extracted obs_source_output_video_internal()
 * (libobs/obs-source-output-video-internal.c.h) against the Rust export,
 * over real private source layouts.
 *
 * The production adapters the Rust export calls
 * (libobs/obs-source-output-video-internal-fields.c.h) are compiled here,
 * once for every test binary. Both they and the original are built with
 * every call the worker makes routed through recorders that log it with
 * its arguments and a snapshot of the watched source's last_output_frame_ts,
 * last_frame_ts and queue length (so a write outside its lock shows):
 * the two mutexes do the real work; os_atomic_set_bool() stores;
 * source_profiler_async_frame_received() only records; cache_video() gives
 * the answer the test asked for (a fresh frame holding the given
 * references, or NULL for an overflow, which empties the queue);
 * free_async_cache() empties the queue; os_atomic_dec_long() drops a real
 * reference; obs_source_frame_destroy() only records (every frame is freed
 * with the source); da_push_back() pushes onto async_frames. The parity
 * test compares the call sequence as well as the state left. */
#define _NO_CRT_STDIO_INLINE
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

enum {
	OVI_LOCK = 1,
	OVI_UNLOCK = 2,
	OVI_SET_BOOL = 3,
	OVI_FREE_CACHE = 4,
	OVI_RECEIVED = 5,
	OVI_CACHE = 6,
	OVI_DEC = 7,
	OVI_DESTROY = 8,
	OVI_PUSH = 9,
};

#define OVI_MAX_EVENTS 64
#define OVI_MAX_FRAMES 64

struct ovi_event {
	int kind;
	const void *p1;
	const void *p2;
	const void *p3;
	/* The call's number, then the snapshot. */
	uint64_t num[4];
};

static struct ovi_event ovi_events[OVI_MAX_EVENTS];
static size_t ovi_event_count;
static const struct obs_source *ovi_watched;

/* cache_video()'s answers, oldest first; past the last, a frame holding 2. */
static bool ovi_answer_some[OVI_MAX_FRAMES];
static long ovi_answer_refs[OVI_MAX_FRAMES];
static size_t ovi_answer_count;
static size_t ovi_answer_next;

/* Every frame the stand-in cache made, freed with the source. */
static struct obs_source_frame *ovi_made[OVI_MAX_FRAMES];
static size_t ovi_made_count;

static void ovi_record(int kind, const void *p1, const void *p2, const void *p3, uint64_t a)
{
	struct ovi_event event = {kind, p1, p2, p3, {a, 0, 0, 0}};
	if (ovi_watched) {
		event.num[1] = ovi_watched->last_output_frame_ts;
		event.num[2] = ovi_watched->last_frame_ts;
		event.num[3] = ovi_watched->async_frames.num;
	}
	if (ovi_event_count < OVI_MAX_EVENTS)
		ovi_events[ovi_event_count] = event;
	ovi_event_count++;
}

static int ovi_mutex_lock(pthread_mutex_t *mutex)
{
	ovi_record(OVI_LOCK, mutex, NULL, NULL, 0);
	return pthread_mutex_lock(mutex);
}

static int ovi_mutex_unlock(pthread_mutex_t *mutex)
{
	ovi_record(OVI_UNLOCK, mutex, NULL, NULL, 0);
	return pthread_mutex_unlock(mutex);
}

static void ovi_set_bool(volatile bool *ptr, bool val)
{
	ovi_record(OVI_SET_BOOL, (const void *)(uintptr_t)ptr, NULL, NULL, val);
	os_atomic_set_bool(ptr, val);
}

static void ovi_free_async_cache(struct obs_source *source)
{
	ovi_record(OVI_FREE_CACHE, source, NULL, NULL, 0);
	da_resize(source->async_frames, 0);
}

static void ovi_frame_received(obs_source_t *source)
{
	ovi_record(OVI_RECEIVED, source, NULL, NULL, 0);
}

/* The source, the frame, the frame made (NULL for an overflow); the
 * frame's timestamp. */
static struct obs_source_frame *ovi_cache_video(struct obs_source *source, const struct obs_source_frame *frame)
{
	bool some = true;
	long refs = 2;
	if (ovi_answer_next < ovi_answer_count) {
		some = ovi_answer_some[ovi_answer_next];
		refs = ovi_answer_refs[ovi_answer_next];
		ovi_answer_next++;
	}
	struct obs_source_frame *made = NULL;
	if (some && ovi_made_count < OVI_MAX_FRAMES) {
		made = bzalloc(sizeof(*made));
		made->timestamp = frame->timestamp;
		made->refs = refs;
		ovi_made[ovi_made_count++] = made;
	}
	ovi_record(OVI_CACHE, source, frame, made, frame->timestamp);
	if (!made)
		da_resize(source->async_frames, 0);
	return made;
}

/* The frame whose count drops; the count left. */
static long ovi_dec_long(volatile long *val)
{
	long left = os_atomic_dec_long(val);
	const char *frame = (const char *)(uintptr_t)val - offsetof(struct obs_source_frame, refs);
	ovi_record(OVI_DEC, frame, NULL, NULL, (uint64_t)(int64_t)left);
	return left;
}

static void ovi_frame_destroy(struct obs_source_frame *frame)
{
	ovi_record(OVI_DESTROY, frame, NULL, NULL, 0);
}

/* The queue's darray and the frame pushed. */
static void ovi_push(struct darray *da, struct obs_source_frame *const *item)
{
	ovi_record(OVI_PUSH, da, *item, NULL, 0);
	darray_push_back(sizeof(*item), da, item);
}

#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#pragma push_macro("da_push_back")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#undef da_push_back
#define pthread_mutex_lock ovi_mutex_lock
#define pthread_mutex_unlock ovi_mutex_unlock
#define da_push_back(v, item) ovi_push(&(v).da, item)
#define os_atomic_set_bool ovi_set_bool
#define os_atomic_dec_long ovi_dec_long
#define free_async_cache ovi_free_async_cache
#define cache_video ovi_cache_video
#define source_profiler_async_frame_received ovi_frame_received
#define obs_source_frame_destroy ovi_frame_destroy

#define obs_source_output_video_internal oracle_obs_source_output_video_internal
void oracle_obs_source_output_video_internal(obs_source_t *source, const struct obs_source_frame *frame);
#include "source-output-video-internal.inc"
#undef obs_source_output_video_internal

#include "source-output-video-internal-fields.inc"

#undef obs_source_frame_destroy
#undef source_profiler_async_frame_received
#undef cache_video
#undef free_async_cache
#undef os_atomic_dec_long
#undef os_atomic_set_bool
#undef da_push_back
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#pragma pop_macro("da_push_back")
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

/* The calls recorded since the last read, oldest first; returns the count
 * (more than max when some were dropped) and clears the log. Each event is
 * its kind, three pointers and four numbers. */
size_t oracle_ovi_events(int *kinds, const void **ptrs, uint64_t *nums, size_t max);
size_t oracle_ovi_events(int *kinds, const void **ptrs, uint64_t *nums, size_t max)
{
	size_t count = ovi_event_count;
	for (size_t i = 0; i < count && i < max && i < OVI_MAX_EVENTS; i++) {
		kinds[i] = ovi_events[i].kind;
		ptrs[i * 3] = ovi_events[i].p1;
		ptrs[i * 3 + 1] = ovi_events[i].p2;
		ptrs[i * 3 + 2] = ovi_events[i].p3;
		for (size_t j = 0; j < 4; j++)
			nums[i * 4 + j] = ovi_events[i].num[j];
	}
	ovi_event_count = 0;
	return count;
}

/* The next cache_video() answers: for each, a frame holding refs[i]
 * references, or NULL when !some[i]. */
void oracle_ovi_answers(const bool *some, const int64_t *refs, size_t count);
void oracle_ovi_answers(const bool *some, const int64_t *refs, size_t count)
{
	ovi_answer_count = count < OVI_MAX_FRAMES ? count : OVI_MAX_FRAMES;
	ovi_answer_next = 0;
	for (size_t i = 0; i < ovi_answer_count; i++) {
		ovi_answer_some[i] = some[i];
		ovi_answer_refs[i] = (long)refs[i];
	}
}

/* A source as obs_source_init() leaves its two mutexes, with the given
 * state and `queued` frames of its own already queued; it becomes the
 * watched source. */
struct obs_source *oracle_ovi_source_new(uint64_t last_output_frame_ts, uint64_t last_frame_ts, bool active, bool stale,
					 size_t queued);
struct obs_source *oracle_ovi_source_new(uint64_t last_output_frame_ts, uint64_t last_frame_ts, bool active, bool stale,
					 size_t queued)
{
	struct obs_source *source = bzalloc(sizeof(*source));
	if (pthread_mutex_init(&source->audio_buf_mutex, NULL) != 0) {
		bfree(source);
		return NULL;
	}
	if (pthread_mutex_init_recursive(&source->async_mutex) != 0) {
		pthread_mutex_destroy(&source->audio_buf_mutex);
		bfree(source);
		return NULL;
	}
	source->last_output_frame_ts = last_output_frame_ts;
	source->last_frame_ts = last_frame_ts;
	source->async_active = active;
	source->async_textures_stale = stale;
	ovi_made_count = 0;
	ovi_answer_count = 0;
	ovi_answer_next = 0;
	for (size_t i = 0; i < queued && ovi_made_count < OVI_MAX_FRAMES; i++) {
		struct obs_source_frame *frame = bzalloc(sizeof(*frame));
		frame->refs = 1;
		ovi_made[ovi_made_count++] = frame;
		darray_push_back(sizeof(frame), &source->async_frames.da, &frame);
	}
	ovi_watched = source;
	return source;
}

void oracle_ovi_source_free(struct obs_source *source);
void oracle_ovi_source_free(struct obs_source *source)
{
	ovi_watched = NULL;
	for (size_t i = 0; i < ovi_made_count; i++)
		bfree(ovi_made[i]);
	ovi_made_count = 0;
	da_free(source->async_frames);
	pthread_mutex_destroy(&source->async_mutex);
	pthread_mutex_destroy(&source->audio_buf_mutex);
	bfree(source);
}

/* The frames the source started with queued, in order (up to max); returns
 * how many. */
size_t oracle_ovi_queued(const struct obs_source *source, const void **frames, size_t max);
size_t oracle_ovi_queued(const struct obs_source *source, const void **frames, size_t max)
{
	size_t count = source->async_frames.num;
	for (size_t i = 0; i < count && i < max; i++)
		frames[i] = source->async_frames.array[i];
	return count;
}

/* The addresses the recorders see, for the roles: audio_buf_mutex,
 * async_mutex, async_active, async_textures_stale and the queue's darray. */
void oracle_ovi_addresses(const struct obs_source *source, const void **out);
void oracle_ovi_addresses(const struct obs_source *source, const void **out)
{
	out[0] = &source->audio_buf_mutex;
	out[1] = &source->async_mutex;
	out[2] = (const void *)(uintptr_t)&source->async_active;
	out[3] = (const void *)(uintptr_t)&source->async_textures_stale;
	out[4] = &source->async_frames.da;
}

/* The state the call may change. */
void oracle_ovi_state(const struct obs_source *source, uint64_t *last_output_frame_ts, uint64_t *last_frame_ts,
		      bool *active, bool *stale);
void oracle_ovi_state(const struct obs_source *source, uint64_t *last_output_frame_ts, uint64_t *last_frame_ts,
		      bool *active, bool *stale)
{
	*last_output_frame_ts = source->last_output_frame_ts;
	*last_frame_ts = source->last_frame_ts;
	*active = source->async_active;
	*stale = source->async_textures_stale;
}

/* A caller's frame with the given timestamp. */
struct obs_source_frame *oracle_ovi_frame_new(uint64_t timestamp);
struct obs_source_frame *oracle_ovi_frame_new(uint64_t timestamp)
{
	struct obs_source_frame *frame = bzalloc(sizeof(*frame));
	frame->timestamp = timestamp;
	return frame;
}

void oracle_ovi_frame_free(struct obs_source_frame *frame);
void oracle_ovi_frame_free(struct obs_source_frame *frame)
{
	bfree(frame);
}

/* The diagnostics logged between begin and end. */
struct ovi_log {
	unsigned count;
	int level;
	char message[256];
};

static struct ovi_log ovi_log;
static log_handler_t ovi_previous;
static void *ovi_previous_param;

#if defined(__GNUC__) || defined(__clang__)
static void ovi_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void ovi_capture(int level, const char *format, va_list args, void *param)
{
	struct ovi_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*ovi_get_log)(log_handler_t *, void **);
typedef void (*ovi_set_log)(log_handler_t, void *);

void oracle_ovi_log_begin(ovi_get_log get_log, ovi_set_log set_log);
void oracle_ovi_log_begin(ovi_get_log get_log, ovi_set_log set_log)
{
	memset(&ovi_log, 0, sizeof(ovi_log));
	get_log(&ovi_previous, &ovi_previous_param);
	set_log(ovi_capture, &ovi_log);
}

/* Restores logging; returns the count, with the last level and message. */
unsigned oracle_ovi_log_end(ovi_set_log set_log, int *level, char *message, size_t size);
unsigned oracle_ovi_log_end(ovi_set_log set_log, int *level, char *message, size_t size)
{
	set_log(ovi_previous, ovi_previous_param);
	*level = ovi_log.level;
	snprintf(message, size, "%s", ovi_log.message);
	return ovi_log.count;
}
