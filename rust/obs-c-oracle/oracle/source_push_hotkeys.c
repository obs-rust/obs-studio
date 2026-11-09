/* Original callbacks, deterministic clock/lock/queue seam. No excluded inputs
 * or intentional behavior changes; user flags are written by the C itself. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <assert.h>

typedef size_t obs_hotkey_id;
typedef void obs_hotkey_t;
enum { AUDIO_ACTION_VOL, AUDIO_ACTION_MUTE, AUDIO_ACTION_PTT, AUDIO_ACTION_PTM };
struct audio_action {
	uint64_t timestamp;
	int type;
	bool set;
};
struct obs_source {
	bool audio_actions_mutex;
	struct audio_action audio_actions;
	bool user_push_to_mute_pressed;
	bool user_push_to_talk_pressed;
};
static _Thread_local uint64_t *push_output;
static uint64_t push_clock(void)
{
	push_output[5] = 1;
	return push_output[0];
}
static int push_lock(bool *locked)
{
	assert(!*locked);
	*locked = true;
	push_output[6] = 2;
	return 0;
}
static int push_unlock(bool *locked)
{
	assert(*locked);
	*locked = false;
	push_output[10] = 4;
	return 0;
}
static void push_append(struct obs_source *source, const struct audio_action *action)
{
	assert(source->audio_actions_mutex);
	source->audio_actions = *action;
	push_output[7] = 3;
	push_output[8] = source->user_push_to_mute_pressed;
	push_output[9] = source->user_push_to_talk_pressed;
}
#define UNUSED_PARAMETER(x) (void)(x)
#define os_gettime_ns push_clock
#define pthread_mutex_lock push_lock
#define pthread_mutex_unlock push_unlock
#define da_push_back(queue, action) push_append(source, action)
#define obs_source_push_hotkey_dispatch oracle_push_hotkey_dispatch
void oracle_push_hotkey_dispatch(void *source, bool talk, bool pressed);
#include "obs-source-push-hotkeys.inc"
#undef obs_source_push_hotkey_dispatch
#undef da_push_back
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#undef os_gettime_ns
#undef UNUSED_PARAMETER

void oracle_push_hotkey_event(uint64_t timestamp, bool mute, bool talk, bool kind, bool pressed, uint64_t *out);
void oracle_push_hotkey_event(uint64_t timestamp, bool mute, bool talk, bool kind, bool pressed, uint64_t *out)
{
	struct obs_source source = {0};
	source.user_push_to_mute_pressed = mute;
	source.user_push_to_talk_pressed = talk;
	out[0] = timestamp;
	push_output = out;
	oracle_push_hotkey_dispatch(&source, kind, pressed);
	out[0] = source.audio_actions.timestamp;
	out[1] = (uint64_t)source.audio_actions.type;
	out[2] = source.audio_actions.set;
	out[3] = source.user_push_to_mute_pressed;
	out[4] = source.user_push_to_talk_pressed;
	push_output = NULL;
}
