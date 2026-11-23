/* Original extracted wrapper, real-header core, and one shared connect
 * recorder. The unit owns no registration algorithm; that dependency is probed
 * here and exercised for real by unchanged Tier 2 tests against both libraries. */
#include "libobs/build/libobs.pch.h"

static struct obs_core raw_audio_add_core;
static unsigned char raw_audio_add_tokens[5];
static struct audio_convert_info raw_audio_add_conversions[3];
static size_t raw_audio_add_masks[MAX_AUDIO_MIXES];
static size_t raw_audio_add_calls;
static size_t raw_audio_add_accepted;
static size_t raw_audio_add_args[5];

static void raw_audio_add_a(void *param, size_t mix, struct audio_data *data)
{
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(mix);
	UNUSED_PARAMETER(data);
}

static void raw_audio_add_b(void *param, size_t mix, struct audio_data *data)
{
	volatile unsigned marker = 1;
	(void)marker;
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(mix);
	UNUSED_PARAMETER(data);
}

static void raw_audio_add_unknown(void *param, size_t mix, struct audio_data *data)
{
	volatile unsigned marker = 2;
	(void)marker;
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(mix);
	UNUSED_PARAMETER(data);
}

static const audio_output_callback_t raw_audio_add_callbacks[] = {NULL, raw_audio_add_a, raw_audio_add_b,
								  raw_audio_add_unknown};

static void *raw_audio_add_param(size_t id)
{
	return id ? &raw_audio_add_tokens[id] : NULL;
}

static const struct audio_convert_info *raw_audio_add_conversion(size_t id)
{
	return id ? &raw_audio_add_conversions[id - 1] : NULL;
}

/* The public getter's original C body, renamed and compiled against this test
 * core. obs-core exports the real obs_get_audio, so the oracle must not. */
audio_t *oracle_raw_audio_add_get_audio(void);
#define obs (&raw_audio_add_core)
#define obs_get_audio oracle_raw_audio_add_get_audio
#include "libobs/obs-get-audio.c.h"
#undef obs_get_audio

/* Records the arguments and keeps audio_output_connect's registration rule:
 * a mix in range takes a callback/parameter pair once; the conversion is not
 * part of the identity. */
bool oracle_raw_audio_add_connect(audio_t *audio, size_t mix, const struct audio_convert_info *conversion,
				  audio_output_callback_t callback, void *param);
bool oracle_raw_audio_add_connect(audio_t *audio, size_t mix, const struct audio_convert_info *conversion,
				  audio_output_callback_t callback, void *param)
{
	size_t cb = 4;
	size_t context = 4;
	size_t conv = 4;
	for (size_t i = 0; i < 4; ++i) {
		if (callback == raw_audio_add_callbacks[i])
			cb = i;
		if (param == raw_audio_add_param(i))
			context = i;
		if (conversion == raw_audio_add_conversion(i))
			conv = i;
	}
	raw_audio_add_calls++;
	raw_audio_add_args[0] = audio == NULL ? 0 : audio == raw_audio_add_core.audio.audio ? 1 : 2;
	raw_audio_add_args[1] = mix;
	raw_audio_add_args[2] = conv;
	raw_audio_add_args[3] = cb;
	raw_audio_add_args[4] = context;
	if (!audio || audio != raw_audio_add_core.audio.audio || mix >= MAX_AUDIO_MIXES || cb >= 4 || context >= 4)
		return false;
	const size_t bit = (size_t)1 << (cb * 4 + context);
	if (raw_audio_add_masks[mix] & bit)
		return false;
	raw_audio_add_masks[mix] |= bit;
	raw_audio_add_accepted++;
	return true;
}

void oracle_obs_add_raw_audio_callback(size_t mix, const struct audio_convert_info *conversion,
				       audio_output_callback_t callback, void *param);
#define obs_add_raw_audio_callback oracle_obs_add_raw_audio_callback
#define audio_output_connect oracle_raw_audio_add_connect
#include "libobs/obs-raw-audio-add.c.h"
#undef audio_output_connect
#undef obs_add_raw_audio_callback
#undef obs

void oracle_raw_audio_add_run(void (*entry)(size_t, const struct audio_convert_info *, audio_output_callback_t, void *),
			      const size_t *masks, bool has_audio, size_t mix, size_t conversion, size_t callback,
			      size_t param, size_t repeats, size_t *report);
void oracle_raw_audio_add_run(void (*entry)(size_t, const struct audio_convert_info *, audio_output_callback_t, void *),
			      const size_t *masks, bool has_audio, size_t mix, size_t conversion, size_t callback,
			      size_t param, size_t repeats, size_t *report)
{
	if (conversion >= 3 || callback >= 4 || param >= 4 || repeats == 0 || repeats > 3)
		abort();
	memset(&raw_audio_add_core, 0x5a, sizeof(raw_audio_add_core));
	raw_audio_add_core.audio.audio = has_audio ? (audio_t *)&raw_audio_add_tokens[0] : NULL;
	raw_audio_add_conversions[0] = (struct audio_convert_info){44100, AUDIO_FORMAT_16BIT, SPEAKERS_MONO, false};
	raw_audio_add_conversions[1] = (struct audio_convert_info){0, AUDIO_FORMAT_UNKNOWN, SPEAKERS_UNKNOWN, true};
	struct obs_core before;
	struct audio_convert_info conversions[3];
	memcpy(&before, &raw_audio_add_core, sizeof(before));
	memcpy(conversions, raw_audio_add_conversions, sizeof(conversions));
	memcpy(raw_audio_add_masks, masks, sizeof(raw_audio_add_masks));
	raw_audio_add_calls = 0;
	raw_audio_add_accepted = 0;
	memset(raw_audio_add_args, 0, sizeof(raw_audio_add_args));
	for (size_t i = 0; i < repeats; ++i)
		entry(mix, raw_audio_add_conversion(conversion), raw_audio_add_callbacks[callback],
		      raw_audio_add_param(param));
	memset(report, 0, 16 * sizeof(*report));
	report[0] = raw_audio_add_calls;
	memcpy(report + 1, raw_audio_add_args, sizeof(raw_audio_add_args));
	memcpy(report + 6, raw_audio_add_masks, sizeof(raw_audio_add_masks));
	report[12] = memcmp(&before, &raw_audio_add_core, sizeof(before)) == 0;
	report[13] = memcmp(conversions, raw_audio_add_conversions, sizeof(conversions)) == 0;
	report[14] = raw_audio_add_accepted;
	memset(&raw_audio_add_core, 0, sizeof(raw_audio_add_core));
}

void oracle_raw_audio_add_layout(size_t *out);
void oracle_raw_audio_add_layout(size_t *out)
{
	out[0] = sizeof(size_t);
	out[1] = _Alignof(size_t);
	out[2] = sizeof(audio_output_callback_t);
	out[3] = _Alignof(audio_output_callback_t);
	out[4] = sizeof(void *);
	out[5] = _Alignof(void *);
	out[6] = sizeof(struct audio_convert_info);
	out[7] = _Alignof(struct audio_convert_info);
	out[8] = offsetof(struct audio_convert_info, samples_per_sec);
	out[9] = offsetof(struct audio_convert_info, format);
	out[10] = offsetof(struct audio_convert_info, speakers);
	out[11] = offsetof(struct audio_convert_info, allow_clipping);
	out[12] = sizeof(bool);
	out[13] = MAX_AUDIO_MIXES;
}
