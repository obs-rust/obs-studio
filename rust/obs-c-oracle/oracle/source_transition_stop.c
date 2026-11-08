/* Test-only oracle: the original libobs/obs-source-transition-stop.c.h,
 * unmodified, with its six functions renamed to oracle_* so they can link
 * next to the Rust shims
 * (rust/obs-source-seam/src/ffi/source_transition_stop.rs).
 *
 * transition_stop is also the name of a member of struct obs_source_info,
 * which transition_handle_stop() calls through: the renames come before the
 * first include, so in this file the member is renamed with the function
 * (its type and place in the struct are unchanged), and
 * oracle_stop_plugin() below sets it under the same name. The Rust side
 * reaches it through the transition seam, compiled under libobs's names by
 * oracle/source_transition_settings.c.
 *
 * The time rule (obs_source_transition_clock_time) is the Rust one, as in
 * the hybrid libobs; the C reads the fields and takes transition_mutex
 * directly, the Rust shims through the seam. The C's other calls go to
 * recording stand-ins, which the parity test hands the Rust shims too
 * (their Calls), so both sides run on the slots oracle's graph
 * (oracle/source_transition_slots.c): its obs_source_release() and
 * obs_source_remove_active_child(), the switch oracle's
 * obs_source_dosignal() (oracle/source_transition_switch.c), and here the
 * video frame time (obs_video_stats_frame_time(&obs->video.stats), whose
 * argument the stand-in drops, so obs is never read) and a recording
 * transition_stop callback. Each records its call in the slots oracle's
 * trace, in order with the others; the frame time and the callback also
 * record whether the watched transition's transition_mutex was held. */

#define obs_transition_get_time oracle_obs_transition_get_time
#define obs_transition_force_stop oracle_obs_transition_force_stop
#define transition_calc_time oracle_transition_calc_time
#define transition_get_video_time oracle_transition_get_video_time
#define transition_stop oracle_transition_stop
#define transition_handle_stop oracle_transition_handle_stop

/* obs-internal.h comes first, so its static inline obs_source_dosignal()
 * and obs-video-stats.h's obs_video_stats_frame_time() keep their names and
 * only the moved file's calls are renamed. */
#include "libobs/obs-internal.h"
#include "libobs/obs-source-transition-stop.h"

void oracle_slots_release(obs_source_t *source);
void oracle_slots_remove_active_child(obs_source_t *parent, obs_source_t *child);
void oracle_slots_trace(int kind, obs_source_t *a, obs_source_t *b, int result);
void oracle_switch_dosignal(obs_source_t *source, const char *signal_obs, const char *signal_source);
uint64_t oracle_stop_video_time(void);

#define obs_source_release oracle_slots_release
#define obs_source_remove_active_child oracle_slots_remove_active_child
#define obs_source_dosignal oracle_switch_dosignal
#define obs_video_stats_frame_time(stats) oracle_stop_video_time()

#include "obs-source-transition-stop.c.h"

#undef obs_video_stats_frame_time
#undef obs_source_dosignal
#undef obs_source_remove_active_child
#undef obs_source_release

/* The trace kinds of the stand-ins below, after the slots oracle's 1-4 and
 * the switch oracle's 5-11. */
#define ORACLE_STOP_VIDEO_TIME 12
#define ORACLE_STOP_PLUGIN 13

static struct {
	obs_source_t *watched;
	uint64_t video_time;
} st;

void oracle_stop_reset(obs_source_t *watched, uint64_t video_time);
void oracle_stop_plugin(obs_source_t *transition);
int oracle_stop_lock(obs_source_t *transition);
int oracle_stop_unlock(obs_source_t *transition);
int oracle_stop_held(obs_source_t *transition);

/* Watches transition (whose lock the stand-ins record) and sets the video
 * frame time. */
void oracle_stop_reset(obs_source_t *watched, uint64_t video_time)
{
	st.watched = watched;
	st.video_time = video_time;
}

/* 1 when transition_mutex is held, 0 when it is free. */
int oracle_stop_held(obs_source_t *transition)
{
	if (pthread_mutex_trylock(&transition->transition_mutex) != 0)
		return 1;
	pthread_mutex_unlock(&transition->transition_mutex);
	return 0;
}

static int watched_held(void)
{
	return st.watched ? oracle_stop_held(st.watched) : -1;
}

uint64_t oracle_stop_video_time(void)
{
	oracle_slots_trace(ORACLE_STOP_VIDEO_TIME, st.watched, NULL, watched_held());
	return st.video_time;
}

static void plugin_stop(void *data)
{
	oracle_slots_trace(ORACLE_STOP_PLUGIN, data, NULL, watched_held());
}

/* Gives a test transition the recording transition_stop callback, with
 * itself as its context data. */
void oracle_stop_plugin(obs_source_t *transition)
{
	transition->info.transition_stop = plugin_stop;
	transition->context.data = transition;
}

/* The caller's lock around the helpers the renders call with it held. */
int oracle_stop_lock(obs_source_t *transition)
{
	return pthread_mutex_lock(&transition->transition_mutex);
}

int oracle_stop_unlock(obs_source_t *transition)
{
	return pthread_mutex_unlock(&transition->transition_mutex);
}
