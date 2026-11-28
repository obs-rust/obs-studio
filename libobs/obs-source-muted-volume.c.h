#include "libobs/build/libobs.pch.h"

/* source_muted() (obs-internal.h, no EXPORT; the audio-output seam's mute
 * check) and get_source_volume() (apply_audio_volume() and
 * apply_audio_actions() on the audio mix thread), moved verbatim out of
 * obs-source.c.h (#5853). get_source_volume() is no longer static:
 * obs-internal.h declares it (hidden, no EXPORT), because this file comes
 * before obs-source.c.h in the unity group. */

/* The gate's decisions are in obs-source-ptm-gate.c, on values. Each field
 * is read and written here as before, in the same order: the pressed flags
 * are written by the hotkey thread without atomics, and get_source_volume()
 * on the audio mix thread writes the stop times too. */
bool source_muted(obs_source_t *source, uint64_t os_time)
{
	if (source->push_to_mute_enabled && source->user_push_to_mute_pressed)
		source->push_to_mute_stop_time = obs_source_ptm_gate_stop_time(os_time, source->push_to_mute_delay);

	if (source->push_to_talk_enabled && source->user_push_to_talk_pressed)
		source->push_to_talk_stop_time = obs_source_ptm_gate_stop_time(os_time, source->push_to_talk_delay);

	bool push_to_mute_active = source->user_push_to_mute_pressed ||
				   obs_source_ptm_gate_held(os_time, source->push_to_mute_stop_time);
	bool push_to_talk_active = source->user_push_to_talk_pressed ||
				   obs_source_ptm_gate_held(os_time, source->push_to_talk_stop_time);

	return !source->enabled || os_atomic_load_bool(&source->user_muted) ||
	       obs_source_ptm_gate_mutes(source->push_to_mute_enabled, push_to_mute_active) ||
	       obs_source_ptm_gate_talk_mutes(source->push_to_talk_enabled, push_to_talk_active);
}

/* The gate goes through obs-source-ptm-gate.c and the snap through
 * obs-source-volume.c, on values. Each field is read and written here as
 * before, in the same order: the stop times only when their key is not
 * pressed, enabled, muted and the gates' enabled flags only where the
 * short-circuit reaches them. */
float get_source_volume(obs_source_t *source, uint64_t os_time)
{
	if (source->push_to_mute_enabled && source->push_to_mute_pressed)
		source->push_to_mute_stop_time = obs_source_ptm_gate_stop_time(os_time, source->push_to_mute_delay);

	if (source->push_to_talk_enabled && source->push_to_talk_pressed)
		source->push_to_talk_stop_time = obs_source_ptm_gate_stop_time(os_time, source->push_to_talk_delay);

	bool push_to_mute_active = source->push_to_mute_pressed ||
				   obs_source_ptm_gate_held(os_time, source->push_to_mute_stop_time);
	bool push_to_talk_active = source->push_to_talk_pressed ||
				   obs_source_ptm_gate_held(os_time, source->push_to_talk_stop_time);

	bool muted = !source->enabled || source->muted ||
		     obs_source_ptm_gate_mutes(source->push_to_mute_enabled, push_to_mute_active) ||
		     obs_source_ptm_gate_talk_mutes(source->push_to_talk_enabled, push_to_talk_active);

	/* source->volume is written only by apply_audio_action() on this
	 * thread, so reading it once, muted or not, reads the same value. */
	return obs_source_volume_snap(muted, source->volume);
}
