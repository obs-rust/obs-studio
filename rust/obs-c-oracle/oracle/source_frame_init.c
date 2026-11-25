/* Exact extracted C oracle, real public layout, and byte-preservation probes. */
#include "obs-internal.h"
#include "media-io/video-frame.h"
#include <stdio.h>
#include <string.h>

void oracle_video_frame_init(struct video_frame *frame, enum video_format format, uint32_t width, uint32_t height);
void oracle_obs_source_frame_init(struct obs_source_frame *frame, enum video_format format, uint32_t width,
				  uint32_t height);
#define video_frame_init oracle_video_frame_init
#define obs_source_frame_init oracle_obs_source_frame_init
#include "source-frame-init.inc"
#undef obs_source_frame_init
#undef video_frame_init

typedef void (*source_frame_init_proc)(struct obs_source_frame *, enum video_format, uint32_t, uint32_t);

unsigned oracle_source_frame_init_check(source_frame_init_proc proc, int format, uint32_t width, uint32_t height,
					uint8_t seed);
unsigned oracle_source_frame_init_check(source_frame_init_proc proc, int format, uint32_t width, uint32_t height,
					uint8_t seed)
{
	struct obs_source_frame rust, original;
	memset(&rust, seed, sizeof(rust));
	memset(&original, seed, sizeof(original));
	proc(&rust, (enum video_format)format, width, height);
	oracle_obs_source_frame_init(&original, (enum video_format)format, width, height);
	uint8_t *original_allocation = original.data[0];
	unsigned mismatch = 0;
	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		if (rust.linesize[i] != original.linesize[i])
			mismatch |= 1;
		if ((rust.data[i] == NULL) != (original.data[i] == NULL)) {
			mismatch |= 2;
		} else if (original.data[i] && (uintptr_t)rust.data[i] - (uintptr_t)rust.data[0] !=
						       (uintptr_t)original.data[i] - (uintptr_t)original_allocation) {
			mismatch |= 2;
		}
		/* Normalize only allocation addresses; compare every other byte. */
		original.data[i] = rust.data[i];
	}
	if (memcmp(&rust, &original, sizeof(rust)) != 0)
		mismatch |= 4;
	bfree(original_allocation);
	bfree(rust.data[0]);
	return mismatch;
}

void oracle_source_frame_init_layout(size_t *out);
void oracle_source_frame_init_layout(size_t *out)
{
	const size_t layout[] = {
		sizeof(struct obs_source_frame),
		_Alignof(struct obs_source_frame),
		offsetof(struct obs_source_frame, data),
		offsetof(struct obs_source_frame, linesize),
		offsetof(struct obs_source_frame, width),
		offsetof(struct obs_source_frame, height),
		offsetof(struct obs_source_frame, timestamp),
		offsetof(struct obs_source_frame, format),
		offsetof(struct obs_source_frame, color_matrix),
		offsetof(struct obs_source_frame, full_range),
		offsetof(struct obs_source_frame, max_luminance),
		offsetof(struct obs_source_frame, color_range_min),
		offsetof(struct obs_source_frame, color_range_max),
		offsetof(struct obs_source_frame, flip),
		offsetof(struct obs_source_frame, flags),
		offsetof(struct obs_source_frame, trc),
		offsetof(struct obs_source_frame, refs),
		offsetof(struct obs_source_frame, prev_frame),
	};
	memcpy(out, layout, sizeof(layout));
}

struct source_frame_init_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_frame_init_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_frame_init_capture(int level, const char *format, va_list args, void *param)
{
	struct source_frame_init_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*source_frame_init_get_log)(log_handler_t *, void **);
typedef void (*source_frame_init_set_log)(log_handler_t, void *);
bool oracle_source_frame_init_logs(source_frame_init_proc proc, source_frame_init_get_log get,
				   source_frame_init_set_log set);
bool oracle_source_frame_init_logs(source_frame_init_proc proc, source_frame_init_get_log get,
				   source_frame_init_set_log set)
{
	log_handler_t saved;
	void *saved_param;
	get(&saved, &saved_param);
	struct source_frame_init_log rust = {0}, original = {0};
	set(source_frame_init_capture, &rust);
	proc(NULL, VIDEO_FORMAT_RGBA, 13, 7);
	set(source_frame_init_capture, &original);
	oracle_obs_source_frame_init(NULL, VIDEO_FORMAT_RGBA, 13, 7);
	set(saved, saved_param);
	return rust.count == 1 && original.count == 1 && rust.level == LOG_DEBUG && original.level == LOG_DEBUG &&
	       strcmp(rust.message, "obs_source_frame_init: Null 'frame' parameter") == 0 &&
	       strcmp(rust.message, original.message) == 0;
}
