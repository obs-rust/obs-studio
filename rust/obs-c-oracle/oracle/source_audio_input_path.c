/* Test-only oracle: the original libobs/obs-source-audio-input-path.c.h,
 * unmodified, with obs_source_output_audio() renamed to oracle_* so it can link
 * next to the Rust shim
 * (rust/obs-source-seam/src/ffi/source_audio_input_path.rs).
 *
 * The Rust shim reaches the fields through the seam,
 * libobs/obs-source-audio-input-path-seam.c.h, compiled here under its own names,
 * as it stays C in libobs.core-source. The decisions are the C oracle's
 * (source_audio_input.c), so this side is C throughout.
 *
 * A test source is a zeroed struct obs_source with its mutexes, and a small
 * context of its own. The audio output, the resampler, the logger, the balance
 * getter, the filters and the output path are stand-ins that append to the
 * context's log, so a test compares the order of the calls and what each one
 * saw. The context is per thread: parity tests run side by side. */

/* Under their own names, so no other test binary that links this library pulls
 * this object in for libobs's global or these calls. */
#define obs oracle_audio_input_path_obs
#define audio_output_get_info oracle_audio_input_path_get_info
#define audio_output_get_planes oracle_audio_input_path_get_planes
#define audio_output_get_block_size oracle_audio_input_path_get_block_size
#define audio_output_get_channels oracle_audio_input_path_get_channels
#define audio_resampler_create oracle_audio_input_path_resampler_create
#define audio_resampler_destroy oracle_audio_input_path_resampler_destroy
#define audio_resampler_resample oracle_audio_input_path_resampler_resample
#define blog oracle_audio_input_path_blog
#define obs_source_get_balance_value oracle_audio_input_path_balance_value
#define filter_async_audio oracle_audio_input_path_filter
#define source_output_audio_data oracle_audio_input_path_output

#define obs_source_audio_input_discontinuous oracle_obs_source_audio_input_discontinuous
#define obs_source_audio_input_reset_needed oracle_obs_source_audio_input_reset_needed
#define obs_source_audio_input_resampler_needed oracle_obs_source_audio_input_resampler_needed
#define obs_source_audio_input_balance_needed oracle_obs_source_audio_input_balance_needed
#define obs_source_audio_input_mono_needed oracle_obs_source_audio_input_mono_needed
#define obs_source_audio_input_balance oracle_obs_source_audio_input_balance
#define obs_source_audio_input_downmix oracle_obs_source_audio_input_downmix

#define obs_source_output_audio oracle_obs_source_output_audio

/* the file's precompiled header brings obs-internal.h, so the oracle_* prototypes
 * come from there through the renames */
#include "obs-source-audio-input-path.c.h"

#undef obs_source_output_audio

#include "obs-source-audio-input-path-seam.h"
#include "obs-source-audio-input-path-seam.c.h"

#include <string.h>

#if defined(_MSC_VER)
#define ORACLE_THREAD_LOCAL __declspec(thread)
#else
#define ORACLE_THREAD_LOCAL __thread
#endif

/* libobs has this one in obs.c, under its own name. The tests never free it. */
struct obs_core *obs;

#define ORACLE_MAX_FRAMES 8192

struct oracle_audio_input_path_init {
	float balance;
	uint32_t flags;
	/* 0: the packet goes on, 1: dropped, 2: replaced */
	int32_t filter_mode;
	bool compensation;
	bool create_fails;
	bool destroying;
};

struct input_context {
	char log[65536];
	size_t len;
	struct oracle_audio_input_path_init init;
	/* what the stand-in resampler and a replacing filter hand out */
	float resampled[MAX_AV_PLANES][ORACLE_MAX_FRAMES];
	float replacement[MAX_AV_PLANES][8];
	struct obs_audio_data replaced;
};

static ORACLE_THREAD_LOCAL obs_source_t *current;
static ORACLE_THREAD_LOCAL struct audio_output_info output_info;

obs_source_t *oracle_audio_input_path_new(const struct oracle_audio_input_path_init *init);
void oracle_audio_input_path_free(obs_source_t *source);
void oracle_audio_input_path_begin(obs_source_t *source, int format, uint32_t rate, int speakers);
struct obs_source_audio *oracle_audio_input_path_packet_new(int format, int speakers, uint32_t rate, uint32_t frames,
							    uint64_t timestamp, float value);
void oracle_audio_input_path_packet_free(struct obs_source_audio *audio);
void oracle_audio_input_path_set_compensation(obs_source_t *source, bool value);
void oracle_audio_input_path_set_balance(obs_source_t *source, float value);
size_t oracle_audio_input_path_state(const obs_source_t *source, char *out, size_t cap);
size_t oracle_audio_input_path_layout(size_t which);

/* No <stdio.h>: on MSVC it makes this object define snprintf(), and another test
 * binary that links this library then pulls the object in for it. Values are
 * written as the hex of their bits, so they compare exactly. */
static void put_str(char *out, size_t cap, size_t *len, const char *text)
{
	size_t n = strlen(text);

	if (*len + n < cap) {
		memcpy(out + *len, text, n);
		*len += n;
		out[*len] = 0;
	}
}

static void put_hex(char *out, size_t cap, size_t *len, uint64_t value)
{
	char text[17];

	for (int i = 0; i < 16; i++)
		text[i] = "0123456789abcdef"[(value >> ((15 - i) * 4)) & 15];
	text[16] = 0;
	put_str(out, cap, len, text);
}

static uint64_t fnv_add(uint64_t hash, const uint8_t *bytes, size_t size)
{
	for (size_t i = 0; i < size; i++) {
		hash ^= bytes[i];
		hash *= 0x100000001b3ULL;
	}
	return hash;
}

static bool mutex_free(pthread_mutex_t *mutex)
{
	bool is_free = pthread_mutex_trylock(mutex) == 0;

	if (is_free)
		pthread_mutex_unlock(mutex);
	return is_free;
}

static uint64_t locks_free(void)
{
	return (mutex_free(&current->audio_buf_mutex) ? 1u : 0u) | (mutex_free(&current->filter_mutex) ? 2u : 0u) |
	       (mutex_free(&current->audio_mutex) ? 4u : 0u);
}

/* The name, the value and which mutexes are free. */
static void add_log(const char *name, uint64_t value)
{
	struct input_context *c = current->context.data;
	size_t cap = sizeof(c->log);

	put_str(c->log, cap, &c->len, name);
	put_str(c->log, cap, &c->len, " ");
	put_hex(c->log, cap, &c->len, value);
	put_str(c->log, cap, &c->len, " locks ");
	put_hex(c->log, cap, &c->len, locks_free());
	put_str(c->log, cap, &c->len, ";");
}

static uint64_t planes_hash(uint8_t *const data[], size_t planes, size_t size)
{
	uint64_t hash = 0xcbf29ce484222325ULL;

	for (size_t i = 0; i < planes; i++) {
		if (data[i])
			hash = fnv_add(hash, data[i], size);
	}
	return hash;
}

const struct audio_output_info *audio_output_get_info(const audio_t *audio)
{
	UNUSED_PARAMETER(audio);
	add_log("info", output_info.samples_per_sec);
	return &output_info;
}

size_t audio_output_get_planes(const audio_t *audio)
{
	size_t planes = get_audio_planes(output_info.format, output_info.speakers);

	UNUSED_PARAMETER(audio);
	add_log("planes", planes);
	return planes;
}

size_t audio_output_get_block_size(const audio_t *audio)
{
	size_t size = get_audio_size(output_info.format, output_info.speakers, 1);

	UNUSED_PARAMETER(audio);
	add_log("block_size", size);
	return size;
}

size_t audio_output_get_channels(const audio_t *audio)
{
	size_t channels = get_audio_channels(output_info.speakers);

	UNUSED_PARAMETER(audio);
	add_log("channels", channels);
	return channels;
}

audio_resampler_t *audio_resampler_create(const struct resample_info *dst, const struct resample_info *src)
{
	struct input_context *c = current->context.data;

	add_log("create dst format", (uint64_t)dst->format);
	add_log("create dst rate", dst->samples_per_sec);
	add_log("create dst speakers", (uint64_t)dst->speakers);
	add_log("create src format", (uint64_t)src->format);
	add_log("create src rate", src->samples_per_sec);
	add_log("create src speakers", (uint64_t)src->speakers);
	/* never dereferenced: the resampler calls are the stand-ins here */
	return c->init.create_fails ? NULL : (audio_resampler_t *)current;
}

void audio_resampler_destroy(audio_resampler_t *resampler)
{
	add_log("destroy", resampler != NULL);
}

/* One output frame for each input frame, from each set plane. */
bool audio_resampler_resample(audio_resampler_t *resampler, uint8_t *output[], uint32_t *out_frames,
			      uint64_t *ts_offset, const uint8_t *const input[], uint32_t in_frames)
{
	struct input_context *c = current->context.data;
	uint32_t frames = in_frames < ORACLE_MAX_FRAMES ? in_frames : ORACLE_MAX_FRAMES;
	uint64_t set = 0;
	uint64_t null_output = 0;

	UNUSED_PARAMETER(resampler);

	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		if (input[i])
			set |= 1ULL << i;
		if (!output[i])
			null_output |= 1ULL << i;
	}
	add_log("resample planes", set);
	add_log("resample null output", null_output);
	add_log("resample frames", in_frames);
	add_log("resample offset", *ts_offset);

	for (size_t i = 0; i < get_audio_planes(output_info.format, output_info.speakers); i++) {
		for (uint32_t f = 0; f < frames; f++)
			c->resampled[i][f] = (float)(f + i);
		if (input[i])
			memcpy(c->resampled[i], input[i], (size_t)frames * sizeof(float));
		output[i] = (uint8_t *)c->resampled[i];
	}

	*out_frames = frames;
	*ts_offset += 7;
	return true;
}

/* The format with each value in hex: %s and the PRIu64 forms are all it holds. */
void blog(int log_level, const char *format, ...)
{
	struct input_context *c = current->context.data;
	size_t cap = sizeof(c->log);
	va_list args;

	va_start(args, format);
	put_hex(c->log, cap, &c->len, (uint64_t)log_level);
	for (const char *p = format; *p; p++) {
		if (*p != '%') {
			char one[2] = {*p, 0};
			put_str(c->log, cap, &c->len, one);
			continue;
		}
		while (p[1] && p[1] != 's' && p[1] != 'u')
			p++;
		p++;
		if (*p == 's')
			put_str(c->log, cap, &c->len, va_arg(args, const char *));
		else
			put_hex(c->log, cap, &c->len, va_arg(args, uint64_t));
	}
	va_end(args);
	put_str(c->log, cap, &c->len, ";");
}

float obs_source_get_balance_value(const obs_source_t *source)
{
	const struct input_context *c = source->context.data;

	add_log("balance_value", 0);
	return c->init.balance;
}

struct obs_audio_data *filter_async_audio(obs_source_t *source, struct obs_audio_data *in)
{
	struct input_context *c = source->context.data;
	size_t planes = get_audio_planes(output_info.format, output_info.speakers);
	size_t size = (size_t)in->frames * sizeof(float);

	add_log("filter same", in == &source->audio_data);
	add_log("filter frames", in->frames);
	add_log("filter ts", in->timestamp);
	if (source->audio_storage_size >= size)
		add_log("filter hash", planes_hash(in->data, planes, size));

	if (c->init.filter_mode == 1)
		return NULL;
	if (c->init.filter_mode == 2) {
		for (size_t i = 0; i < MAX_AV_PLANES; i++)
			c->replaced.data[i] = (uint8_t *)c->replacement[i];
		c->replaced.frames = 8;
		c->replaced.timestamp = in->timestamp + 1;
		return &c->replaced;
	}
	return in;
}

void source_output_audio_data(obs_source_t *source, const struct audio_data *data)
{
	size_t planes = get_audio_planes(output_info.format, output_info.speakers);
	size_t size = (size_t)data->frames * sizeof(float);

	add_log("output frames", data->frames);
	add_log("output ts", data->timestamp);
	if (source->audio_storage_size >= size || data->frames == 8)
		add_log("output hash", planes_hash(data->data, planes, size));
}

static pthread_once_t core_once = PTHREAD_ONCE_INIT;

static void core_init(void)
{
	obs = bzalloc(sizeof(*obs));
}

obs_source_t *oracle_audio_input_path_new(const struct oracle_audio_input_path_init *init)
{
	obs_source_t *source = bzalloc(sizeof(*source));
	struct input_context *c = bzalloc(sizeof(*c));

	pthread_once(&core_once, core_init);

	c->init = *init;
	source->context.data = c;
	source->context.name = (char *)"in";
	source->flags = init->flags;
	source->destroying = init->destroying;
	source->async_compensation = init->compensation;
	pthread_mutex_init(&source->audio_buf_mutex, NULL);
	pthread_mutex_init(&source->filter_mutex, NULL);
	pthread_mutex_init(&source->audio_mutex, NULL);
	return source;
}

void oracle_audio_input_path_free(obs_source_t *source)
{
	for (size_t i = 0; i < MAX_AV_PLANES; i++)
		bfree(source->audio_data.data[i]);
	pthread_mutex_destroy(&source->audio_buf_mutex);
	pthread_mutex_destroy(&source->filter_mutex);
	pthread_mutex_destroy(&source->audio_mutex);
	bfree(source->context.data);
	bfree(source);
}

void oracle_audio_input_path_begin(obs_source_t *source, int format, uint32_t rate, int speakers)
{
	current = source;
	output_info.format = (enum audio_format)format;
	output_info.samples_per_sec = rate;
	output_info.speakers = (enum speaker_layout)speakers;
}

void oracle_audio_input_path_set_compensation(obs_source_t *source, bool value)
{
	source->async_compensation = value;
}

void oracle_audio_input_path_set_balance(obs_source_t *source, float value)
{
	struct input_context *c = source->context.data;

	c->init.balance = value;
}

/* All MAX_AV_PLANES set, each with the frames of a two channel interleaved packet,
 * so a read past what the format says would show. */
struct obs_source_audio *oracle_audio_input_path_packet_new(int format, int speakers, uint32_t rate, uint32_t frames,
							    uint64_t timestamp, float value)
{
	struct obs_source_audio *audio = bzalloc(sizeof(*audio));

	audio->format = (enum audio_format)format;
	audio->speakers = (enum speaker_layout)speakers;
	audio->samples_per_sec = rate;
	audio->frames = frames;
	audio->timestamp = timestamp;
	for (size_t plane = 0; plane < MAX_AV_PLANES; plane++) {
		float *samples = bmalloc(((size_t)frames + 1) * 2 * sizeof(float));
		for (uint32_t i = 0; i < (frames + 1) * 2; i++)
			samples[i] = value + (float)plane + (float)i;
		audio->data[plane] = (const uint8_t *)samples;
	}
	return audio;
}

void oracle_audio_input_path_packet_free(struct obs_source_audio *audio)
{
	for (size_t plane = 0; plane < MAX_AV_PLANES; plane++)
		bfree((void *)(uintptr_t)audio->data[plane]);
	bfree(audio);
}

/* The log, every field, the storage's bytes, and whether the mutexes are free. */
size_t oracle_audio_input_path_state(const obs_source_t *source, char *out, size_t cap)
{
	const struct input_context *c = source->context.data;
	obs_source_t *s = (obs_source_t *)(uintptr_t)source;
	size_t size = (size_t)source->audio_data.frames * sizeof(float);
	size_t planes = get_audio_planes(output_info.format, output_info.speakers);
	size_t len = 0;

	out[0] = 0;
	put_str(out, cap, &len, c->log);
	put_str(out, cap, &len, "|next ");
	put_hex(out, cap, &len, source->next_resample_in_ts);
	put_str(out, cap, &len, " format ");
	put_hex(out, cap, &len, (uint64_t)source->sample_info.format);
	put_str(out, cap, &len, " rate ");
	put_hex(out, cap, &len, source->sample_info.samples_per_sec);
	put_str(out, cap, &len, " speakers ");
	put_hex(out, cap, &len, (uint64_t)source->sample_info.speakers);
	put_str(out, cap, &len, source->last_async_compensation ? " comp" : " nocomp");
	put_str(out, cap, &len, source->audio_failed ? " failed" : " fine");
	put_str(out, cap, &len, source->resampler ? " resampler" : " noresampler");
	put_str(out, cap, &len, " offset ");
	put_hex(out, cap, &len, source->resample_offset);
	put_str(out, cap, &len, " storage ");
	put_hex(out, cap, &len, (uint64_t)source->audio_storage_size);
	put_str(out, cap, &len, " frames ");
	put_hex(out, cap, &len, source->audio_data.frames);
	put_str(out, cap, &len, " ts ");
	put_hex(out, cap, &len, source->audio_data.timestamp);
	if (source->audio_storage_size >= size) {
		put_str(out, cap, &len, " hash ");
		put_hex(out, cap, &len, planes_hash(source->audio_data.data, planes, size));
	}
	put_str(out, cap, &len,
		mutex_free(&s->audio_buf_mutex) && mutex_free(&s->filter_mutex) && mutex_free(&s->audio_mutex)
			? " free"
			: " held");
	return len;
}

size_t oracle_audio_input_path_layout(size_t which)
{
	const size_t values[8] = {
		sizeof(struct obs_source_audio),
		_Alignof(struct obs_source_audio),
		offsetof(struct obs_source_audio, data),
		offsetof(struct obs_source_audio, frames),
		offsetof(struct obs_source_audio, speakers),
		offsetof(struct obs_source_audio, format),
		offsetof(struct obs_source_audio, samples_per_sec),
		offsetof(struct obs_source_audio, timestamp),
	};
	return which < 8 ? values[which] : 0;
}
