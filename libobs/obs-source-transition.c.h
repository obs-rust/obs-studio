/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "libobs/build/libobs.pch.h"

#define lock_transition(transition) pthread_mutex_lock(&transition->transition_mutex);
#define unlock_transition(transition) pthread_mutex_unlock(&transition->transition_mutex);

_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_MODE_MANUAL == OBS_TRANSITION_MODE_MANUAL, "OBS_TRANSITION_MODE_MANUAL");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_SCALE_MAX_ONLY == OBS_TRANSITION_SCALE_MAX_ONLY,
	       "OBS_TRANSITION_SCALE_MAX_ONLY");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_SCALE_ASPECT == OBS_TRANSITION_SCALE_ASPECT, "OBS_TRANSITION_SCALE_ASPECT");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_SCALE_STRETCH == OBS_TRANSITION_SCALE_STRETCH,
	       "OBS_TRANSITION_SCALE_STRETCH");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB == GS_CS_SRGB, "GS_CS_SRGB");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB_16F == GS_CS_SRGB_16F, "GS_CS_SRGB_16F");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_CS_709_EXTENDED == GS_CS_709_EXTENDED, "GS_CS_709_EXTENDED");
_Static_assert(OBS_SOURCE_TRANSITION_CLOCK_CS_709_SCRGB == GS_CS_709_SCRGB, "GS_CS_709_SCRGB");
_Static_assert(sizeof(enum obs_transition_mode) == sizeof(int), "enum obs_transition_mode is passed as int");
_Static_assert(sizeof(enum obs_transition_scale_type) == sizeof(int),
	       "enum obs_transition_scale_type is passed as int");
_Static_assert(sizeof(enum gs_color_space) == sizeof(int), "enum gs_color_space is passed as int");
_Static_assert(OBS_SOURCE_TRANSITION_RENDER_TARGET_B == OBS_TRANSITION_SOURCE_B, "OBS_TRANSITION_SOURCE_B");
_Static_assert(OBS_SOURCE_TRANSITION_RENDER_AUDIO_FRAMES == AUDIO_OUTPUT_FRAMES, "AUDIO_OUTPUT_FRAMES");
_Static_assert(sizeof(enum obs_transition_target) == sizeof(int), "enum obs_transition_target is passed as int");
_Static_assert(OBS_SOURCE_TRANSITION_START_MODE_MANUAL == OBS_TRANSITION_MODE_MANUAL, "OBS_TRANSITION_MODE_MANUAL");
_Static_assert(OBS_SOURCE_TRANSITION_LOAD_TYPE_TRANSITION == OBS_SOURCE_TYPE_TRANSITION, "OBS_SOURCE_TYPE_TRANSITION");
_Static_assert(sizeof(enum obs_source_type) == sizeof(int), "enum obs_source_type is passed as int");
_Static_assert(OBS_SOURCE_TRANSITION_LOAD_SCALE_MAX_ONLY == OBS_TRANSITION_SCALE_MAX_ONLY,
	       "OBS_TRANSITION_SCALE_MAX_ONLY");
_Static_assert(OBS_SOURCE_TRANSITION_LOAD_SCALE_ASPECT == OBS_TRANSITION_SCALE_ASPECT, "OBS_TRANSITION_SCALE_ASPECT");
_Static_assert(OBS_SOURCE_TRANSITION_LOAD_SCALE_STRETCH == OBS_TRANSITION_SCALE_STRETCH,
	       "OBS_TRANSITION_SCALE_STRETCH");

bool obs_transition_init(obs_source_t *transition)
{
	pthread_mutex_init_value(&transition->transition_mutex);
	pthread_mutex_init_value(&transition->transition_tex_mutex);
	if (pthread_mutex_init(&transition->transition_mutex, NULL) != 0)
		return false;
	if (pthread_mutex_init(&transition->transition_tex_mutex, NULL) != 0)
		return false;

	transition->transition_alignment = OBS_ALIGN_LEFT | OBS_ALIGN_TOP;
	transition->transition_texrender[0] = gs_texrender_create(GS_RGBA, GS_ZS_NONE);
	transition->transition_texrender[1] = gs_texrender_create(GS_RGBA, GS_ZS_NONE);
	transition->transition_source_active[0] = true;

	return transition->transition_texrender[0] != NULL && transition->transition_texrender[1] != NULL;
}

void obs_transition_free(obs_source_t *transition)
{
	pthread_mutex_destroy(&transition->transition_mutex);
	pthread_mutex_destroy(&transition->transition_tex_mutex);

	gs_enter_context(obs->video.graphics);
	gs_texrender_destroy(transition->transition_texrender[0]);
	gs_texrender_destroy(transition->transition_texrender[1]);
	gs_leave_context();
}

static inline void mix_child(obs_source_t *transition, const struct transition_state *state, float *out, float *in,
			     size_t count, size_t sample_rate, uint64_t ts, obs_transition_audio_mix_callback_t mix)
{
	void *context_data = transition->context.data;

	for (size_t i = 0; i < count; i++) {
		float t = obs_source_transition_sample_time((int)state->mode, state->manual_val, state->start_time,
							    state->duration, ts, sample_rate, i);
		out[i] += in[i] * mix(context_data, t);
	}
}

static void transition_process_audio(obs_source_t *transition, const struct transition_state *state,
				     obs_source_t *child, struct obs_source_audio_mix *audio, uint64_t min_ts,
				     uint32_t mixers, size_t channels, size_t sample_rate,
				     obs_transition_audio_mix_callback_t mix)
{
	bool valid = obs_source_transition_render_audio_valid(child != NULL, child && child->audio_pending,
							      child && child->audio_is_duplicated);
	enum obs_source_transition_render_audio_place place;
	struct obs_source_audio_mix child_audio;
	uint64_t ts;
	size_t pos = 0;

	if (!valid)
		return;

	ts = obs_source_get_audio_timestamp(child);
	place = obs_source_transition_render_audio_place(ts, min_ts, sample_rate, &pos);
	if (place == OBS_SOURCE_TRANSITION_RENDER_PLACE_NONE)
		return;
	obs_source_get_audio_mix(child, &child_audio);

	if (place == OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE)
		return;

	for (size_t mix_idx = 0; mix_idx < MAX_AUDIO_MIXES; mix_idx++) {
		struct audio_output_data *output = &audio->output[mix_idx];
		struct audio_output_data *input = &child_audio.output[mix_idx];

		if ((mixers & (1 << mix_idx)) == 0)
			continue;

		for (size_t ch = 0; ch < channels; ch++) {
			float *out = output->data[ch];
			float *in = input->data[ch];

			mix_child(transition, state, out + pos, in, AUDIO_OUTPUT_FRAMES - pos, sample_rate, ts, mix);
		}
	}
}

/* The transition's children, leaving out any that was removed. */
static inline void transition_live_sources(obs_source_t *transition, obs_source_t *sources[2])
{
	for (size_t i = 0; i < 2; i++) {
		sources[i] = transition->transition_sources[i];
		if (sources[i] && obs_source_removed(sources[i]))
			sources[i] = NULL;
	}
}

static inline uint64_t transition_calc_min_ts(obs_source_t *sources[2])
{
	uint64_t timestamps[2] = {0, 0};

	for (size_t i = 0; i < 2; i++) {
		if (sources[i] && !sources[i]->audio_pending)
			timestamps[i] = obs_source_get_audio_timestamp(sources[i]);
	}

	return obs_source_transition_clock_min_ts(timestamps[0], timestamps[1]);
}

bool obs_transition_audio_render(obs_source_t *transition, uint64_t *ts_out, struct obs_source_audio_mix *audio,
				 uint32_t mixers, size_t channels, size_t sample_rate,
				 obs_transition_audio_mix_callback_t mix_a, obs_transition_audio_mix_callback_t mix_b)
{
	obs_source_t *sources[2];
	struct transition_state state = {0};
	enum obs_source_transition_render_audio_step step;
	bool stopped = false;
	uint64_t min_ts;
	uint32_t mix;
	float t = 0.0f;

	if (!transition_valid(transition, "obs_transition_audio_render"))
		return false;

	lock_transition(transition);

	transition_live_sources(transition, sources);
	min_ts = transition_calc_min_ts(sources);

	if (min_ts)
		t = transition_calc_time(transition, min_ts);

	step = obs_source_transition_render_audio_stop(min_ts != 0, t,
						       os_atomic_load_bool(&transition->transitioning_video),
						       os_atomic_load_bool(&transition->transitioning_audio));
	if (step != OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP) {
		os_atomic_store_bool(&transition->transitioning_audio, false);
		if (step == OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP) {
			transition_stop(transition);
			stopped = true;
		}
	}

	if (min_ts) {
		/* The audio stop can make a removed child source A (#2185). */
		transition_live_sources(transition, sources);
		min_ts = transition_calc_min_ts(sources);
		if (min_ts) {
			state.s[0] = obs_source_get_ref(sources[0]);
			state.s[1] = obs_source_get_ref(sources[1]);
			state.transitioning_video = os_atomic_load_bool(&transition->transitioning_video);
			state.transitioning_audio = os_atomic_load_bool(&transition->transitioning_audio);
			state.mode = transition->transition_mode;
			state.manual_val = transition->transition_manual_val;
			state.start_time = transition->transition_start_time;
			state.duration = transition->transition_duration;
		}
	}

	unlock_transition(transition);

	if (min_ts) {
		mix = obs_source_transition_render_audio_mix(state.transitioning_audio, state.s[0] != NULL,
							     state.s[1] != NULL);
		if (mix & OBS_SOURCE_TRANSITION_RENDER_MIX_A)
			transition_process_audio(transition, &state, state.s[0], audio, min_ts, mixers, channels,
						 sample_rate, mix_a);
		if (mix & OBS_SOURCE_TRANSITION_RENDER_MIX_B)
			transition_process_audio(transition, &state, state.s[1], audio, min_ts, mixers, channels,
						 sample_rate, mix_b);
		if (mix & OBS_SOURCE_TRANSITION_RENDER_COPY_A)
			memcpy(audio->output[0].data[0], state.s[0]->audio_output_buf[0][0], TOTAL_AUDIO_SIZE);

		obs_source_release(state.s[0]);
		obs_source_release(state.s[1]);
	}

	if (stopped)
		transition_handle_stop(transition);

	*ts_out = min_ts;
	return !!min_ts;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef lock_transition
#undef unlock_transition
