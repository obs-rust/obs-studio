/* Original video-statistics queries (#5364), renamed, beside the real
 * adapters of obs-video-stats-queries-fields.c.h. Both read a thread-local
 * current core instead of the library's global, so the original and the Rust
 * shim read the same real-header struct obs_video_stats through the same
 * obs_video_stats_* readers. The probe reports, instead of aborting, any
 * difference in a query's result bits, a stats mutex left locked, a changed
 * stats value or a changed current core pointer, so mutants are observable.
 * Excludes a NULL or dead core and concurrent stats writes: the original
 * requires a live core. Real cores stay covered by the unchanged
 * actual-library C tests. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#if defined(_MSC_VER)
static __declspec(thread) struct obs_core *video_stats_current;
#else
static _Thread_local struct obs_core *video_stats_current;
#endif

uint64_t oracle_obs_get_video_frame_time(void);
double oracle_obs_get_active_fps(void);
uint64_t oracle_obs_get_average_frame_time_ns(void);
uint32_t oracle_obs_get_total_frames(void);
uint32_t oracle_obs_get_lagged_frames(void);
#define obs video_stats_current
#define obs_get_video_frame_time oracle_obs_get_video_frame_time
#define obs_get_active_fps oracle_obs_get_active_fps
#define obs_get_average_frame_time_ns oracle_obs_get_average_frame_time_ns
#define obs_get_total_frames oracle_obs_get_total_frames
#define obs_get_lagged_frames oracle_obs_get_lagged_frames
#include "libobs/obs-video-stats-queries.c.h"
#undef obs_get_lagged_frames
#undef obs_get_total_frames
#undef obs_get_average_frame_time_ns
#undef obs_get_active_fps
#undef obs_get_video_frame_time
#include "video_stats_queries_fields.inc"
#undef obs

/* The adapter's address against the real field. */
bool oracle_video_stats_queries_adapter(void);
bool oracle_video_stats_queries_adapter(void)
{
	struct obs_core *core = calloc(1, sizeof(*core));
	if (!core)
		return false;
	video_stats_current = core;
	bool same = obs_video_stats_queries_stats() == &core->video.stats;
	video_stats_current = NULL;
	free(core);
	return same;
}

typedef uint64_t (*video_stats_u64_fn)(void);
typedef double (*video_stats_f64_fn)(void);
typedef uint32_t (*video_stats_u32_fn)(void);

struct video_stats_candidates {
	video_stats_u64_fn frame_time;
	video_stats_f64_fn fps;
	video_stats_u64_fn avg_frame_time_ns;
	video_stats_u32_fn total_frames;
	video_stats_u32_fn lagged_frames;
};

static bool video_stats_unlocked(struct obs_video_stats *stats)
{
	if (pthread_mutex_trylock(&stats->mutex) != 0)
		return false;
	pthread_mutex_unlock(&stats->mutex);
	return true;
}

static bool video_stats_same_values(const struct obs_video_stats *a, const struct obs_video_stats *b)
{
	return a->frame_time_ns == b->frame_time_ns && a->avg_frame_time_ns == b->avg_frame_time_ns &&
	       memcmp(&a->fps, &b->fps, sizeof(a->fps)) == 0 && a->total_frames == b->total_frames &&
	       a->lagged_frames == b->lagged_frames;
}

/* Bit i (0..4, in the order frame time, FPS, average frame time, total,
 * lagged) set: that candidate's result differs from the original's. 32: a
 * query left the stats mutex locked. 64: the stats values changed. 128: the
 * current core pointer changed. 256: setup failed. out[0..4] receive the
 * original's results as raw 64-bit patterns. */
unsigned oracle_video_stats_queries_probe(const struct video_stats_candidates *cand, uint64_t frame_time_ns,
					  uint64_t avg_frame_time_ns, uint64_t fps_bits, uint32_t total_frames,
					  uint32_t lagged_frames, uint64_t *out);
unsigned oracle_video_stats_queries_probe(const struct video_stats_candidates *cand, uint64_t frame_time_ns,
					  uint64_t avg_frame_time_ns, uint64_t fps_bits, uint32_t total_frames,
					  uint32_t lagged_frames, uint64_t *out)
{
	struct obs_core *core = calloc(1, sizeof(*core));
	if (!core || obs_video_stats_init(&core->video.stats) != 0) {
		free(core);
		return 256;
	}
	struct obs_video_stats *stats = &core->video.stats;
	stats->frame_time_ns = frame_time_ns;
	stats->avg_frame_time_ns = avg_frame_time_ns;
	memcpy(&stats->fps, &fps_bits, sizeof(fps_bits));
	stats->total_frames = total_frames;
	stats->lagged_frames = lagged_frames;
	struct obs_video_stats before = *stats;
	video_stats_current = core;

	unsigned diff = 0;
	uint64_t want[5], got[5];
	double fps;

	want[0] = oracle_obs_get_video_frame_time();
	fps = oracle_obs_get_active_fps();
	memcpy(&want[1], &fps, sizeof(fps));
	want[2] = oracle_obs_get_average_frame_time_ns();
	want[3] = oracle_obs_get_total_frames();
	want[4] = oracle_obs_get_lagged_frames();

	got[0] = cand->frame_time();
	if (!video_stats_unlocked(stats))
		diff |= 32;
	fps = cand->fps();
	memcpy(&got[1], &fps, sizeof(fps));
	if (!video_stats_unlocked(stats))
		diff |= 32;
	got[2] = cand->avg_frame_time_ns();
	if (!video_stats_unlocked(stats))
		diff |= 32;
	got[3] = cand->total_frames();
	if (!video_stats_unlocked(stats))
		diff |= 32;
	got[4] = cand->lagged_frames();
	if (!video_stats_unlocked(stats))
		diff |= 32;

	for (unsigned i = 0; i < 5; ++i) {
		out[i] = want[i];
		if (want[i] != got[i])
			diff |= 1u << i;
	}
	if (!video_stats_same_values(&before, stats))
		diff |= 64;
	if (video_stats_current != core)
		diff |= 128;

	video_stats_current = NULL;
	obs_video_stats_destroy(stats);
	free(core);
	return diff;
}
