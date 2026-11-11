/* Original extracted wrapper, real-header core, and one shared disconnect
 * recorder. The unit owns no registration algorithm; that dependency is probed
 * here and exercised for real by unchanged Tier 2 tests against both libraries. */
#include "libobs/build/libobs.pch.h"

static struct obs_core raw_audio_remove_core;
static unsigned char raw_audio_remove_tokens[5];
static size_t raw_audio_remove_masks[MAX_AUDIO_MIXES];
static size_t raw_audio_remove_calls;
static size_t raw_audio_remove_args[4];

static void raw_audio_remove_a(void *param, size_t mix, struct audio_data *data)
{
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(mix);
	UNUSED_PARAMETER(data);
}

static void raw_audio_remove_b(void *param, size_t mix, struct audio_data *data)
{
	volatile unsigned marker = 1;
	(void)marker;
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(mix);
	UNUSED_PARAMETER(data);
}

static void raw_audio_remove_unknown(void *param, size_t mix, struct audio_data *data)
{
	volatile unsigned marker = 2;
	(void)marker;
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(mix);
	UNUSED_PARAMETER(data);
}

static const audio_output_callback_t raw_audio_remove_callbacks[] = {NULL, raw_audio_remove_a, raw_audio_remove_b,
								     raw_audio_remove_unknown};

static void *raw_audio_remove_param(size_t id)
{
	return id ? &raw_audio_remove_tokens[id] : NULL;
}

/* The public getter's real body, compiled verbatim against this test core. This
 * also resolves the production shim's dependency in standalone test binaries. */
#define obs (&raw_audio_remove_core)
#include "raw-audio-main-getter.inc"

void oracle_raw_audio_remove_disconnect(audio_t *audio, size_t mix, audio_output_callback_t callback, void *param);
void oracle_raw_audio_remove_disconnect(audio_t *audio, size_t mix, audio_output_callback_t callback, void *param)
{
	size_t cb = 4;
	size_t context = 4;
	for (size_t i = 0; i < 4; ++i) {
		if (callback == raw_audio_remove_callbacks[i])
			cb = i;
		if (param == raw_audio_remove_param(i))
			context = i;
	}
	raw_audio_remove_calls++;
	raw_audio_remove_args[0] = audio == NULL ? 0 : audio == raw_audio_remove_core.audio.audio ? 1 : 2;
	raw_audio_remove_args[1] = mix;
	raw_audio_remove_args[2] = cb;
	raw_audio_remove_args[3] = context;
	if (audio && audio == raw_audio_remove_core.audio.audio && mix < MAX_AUDIO_MIXES && cb < 4 && context < 4)
		raw_audio_remove_masks[mix] &= ~((size_t)1 << (cb * 4 + context));
}

void oracle_obs_remove_raw_audio_callback(size_t mix, audio_output_callback_t callback, void *param);
#define obs_remove_raw_audio_callback oracle_obs_remove_raw_audio_callback
#define audio_output_disconnect oracle_raw_audio_remove_disconnect
#include "libobs/obs-raw-audio-remove.c.h"
#undef audio_output_disconnect
#undef obs_remove_raw_audio_callback
#undef obs

void oracle_raw_audio_remove_run(void (*entry)(size_t, audio_output_callback_t, void *), const size_t *masks,
				 bool has_audio, size_t mix, size_t callback, size_t param, size_t repeats,
				 size_t *report);
void oracle_raw_audio_remove_run(void (*entry)(size_t, audio_output_callback_t, void *), const size_t *masks,
				 bool has_audio, size_t mix, size_t callback, size_t param, size_t repeats,
				 size_t *report)
{
	if (callback >= 4 || param >= 4 || repeats == 0 || repeats > 3)
		abort();
	memset(&raw_audio_remove_core, 0x5a, sizeof(raw_audio_remove_core));
	raw_audio_remove_core.audio.audio = has_audio ? (audio_t *)&raw_audio_remove_tokens[0] : NULL;
	struct obs_core before;
	memcpy(&before, &raw_audio_remove_core, sizeof(before));
	memcpy(raw_audio_remove_masks, masks, sizeof(raw_audio_remove_masks));
	raw_audio_remove_calls = 0;
	memset(raw_audio_remove_args, 0, sizeof(raw_audio_remove_args));
	for (size_t i = 0; i < repeats; ++i)
		entry(mix, raw_audio_remove_callbacks[callback], raw_audio_remove_param(param));
	memset(report, 0, 16 * sizeof(*report));
	report[0] = raw_audio_remove_calls;
	memcpy(report + 1, raw_audio_remove_args, sizeof(raw_audio_remove_args));
	memcpy(report + 5, raw_audio_remove_masks, sizeof(raw_audio_remove_masks));
	report[11] = memcmp(&before, &raw_audio_remove_core, sizeof(before)) == 0;
	memset(&raw_audio_remove_core, 0, sizeof(raw_audio_remove_core));
}

void oracle_raw_audio_remove_layout(size_t *out);
void oracle_raw_audio_remove_layout(size_t *out)
{
	out[0] = sizeof(size_t);
	out[1] = _Alignof(size_t);
	out[2] = sizeof(audio_output_callback_t);
	out[3] = _Alignof(audio_output_callback_t);
	out[4] = sizeof(void *);
	out[5] = _Alignof(void *);
	out[6] = sizeof(struct audio_data);
	out[7] = _Alignof(struct audio_data);
	out[8] = offsetof(struct audio_data, data);
	out[9] = offsetof(struct audio_data, frames);
	out[10] = offsetof(struct audio_data, timestamp);
	out[11] = MAX_AUDIO_MIXES;
}
