/* Original extracted obs_canvas_reset_video() entry. The real flags field,
 * the Rust canvas rules and the Rust obs_video_active() over the recording
 * fixture core of oracle/video_active.c serve both implementations; the
 * internal reset is a recording double under its real name (the real one
 * needs graphics; it stays unchanged Tier 2, test_canvas_reset_video_entry).
 * NULL canvas with non-NULL info is outside the C contract. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <string.h>

bool oracle_obs_canvas_reset_video(obs_canvas_t *canvas, struct obs_video_info *ovi);
#define obs_canvas_reset_video oracle_obs_canvas_reset_video
#include "canvas-reset-video-entry.inc"
#undef obs_canvas_reset_video

void oracle_video_active_arm_one(long raw_count);
size_t oracle_video_active_events(void);

static _Thread_local struct {
	size_t calls;
	obs_canvas_t *canvas;
	struct obs_video_info *ovi;
	bool result;
} reset_double;

bool obs_canvas_reset_video_internal(obs_canvas_t *canvas, struct obs_video_info *ovi)
{
	reset_double.calls++;
	reset_double.canvas = canvas;
	reset_double.ovi = ovi;
	return reset_double.result;
}

typedef bool (*reset_fn)(obs_canvas_t *, struct obs_video_info *);

/* trace: [0] answer, [1] internal calls, [2] internal got this canvas,
 * [3] internal got this info, [4] activity-query events, [5] canvas and
 * info bytes unchanged. `null_info` passes NULL info (and a NULL canvas). */
void oracle_canvas_reset_video_run(reset_fn candidate, uint32_t flags, long raw_active, bool null_info, bool result,
				   size_t *trace);
void oracle_canvas_reset_video_run(reset_fn candidate, uint32_t flags, long raw_active, bool null_info, bool result,
				   size_t *trace)
{
	static _Thread_local struct obs_canvas canvas, canvas_before;
	static _Thread_local struct obs_video_info ovi, ovi_before;
	memset(trace, 0, 6 * sizeof(*trace));
	memset(&canvas, 0x5a, sizeof(canvas));
	canvas.flags = flags;
	memset(&ovi, 0x33, sizeof(ovi));
	memcpy(&canvas_before, &canvas, sizeof(canvas));
	memcpy(&ovi_before, &ovi, sizeof(ovi));
	memset(&reset_double, 0, sizeof(reset_double));
	reset_double.result = result;
	oracle_video_active_arm_one(raw_active);
	bool answer = null_info ? candidate(NULL, NULL) : candidate(&canvas, &ovi);
	trace[0] = answer;
	trace[1] = reset_double.calls;
	trace[2] = reset_double.canvas == &canvas;
	trace[3] = reset_double.ovi == &ovi;
	trace[4] = oracle_video_active_events();
	trace[5] = memcmp(&canvas, &canvas_before, sizeof(canvas)) == 0 && memcmp(&ovi, &ovi_before, sizeof(ovi)) == 0;
}
