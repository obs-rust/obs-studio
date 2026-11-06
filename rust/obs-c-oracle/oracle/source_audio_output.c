/* Test-only oracle: the original libobs/obs-source-audio-output.c.h, unmodified,
 * with its functions renamed to oracle_* so they can link next to the Rust shims
 * (rust/obs-source-seam/src/ffi/source_audio_output.rs).
 *
 * The Rust shims reach the fields through the seam,
 * libobs/obs-source-audio-output-seam.c.h, compiled here under its own names, as
 * it stays C in libobs.core-source. The decisions are the C oracles'
 * (source_audio_timing.c, source_audio_place.c), so this side is C throughout.
 *
 * A test source is a zeroed struct obs_source with its mutexes, and a small
 * context of its own. The clock, the audio output, the logger, the resampler's
 * compensation, source_muted() and source_signal_audio_data() are stand-ins that
 * append to the context's log, so a test compares the order of the calls and the
 * fields each one saw. The clock and the context are per thread: parity tests
 * run side by side. */

/* Under their own names, so no other test binary that links this library pulls
 * this object in for libobs's global or these calls. */
#define obs oracle_audio_output_obs
#define os_gettime_ns oracle_audio_output_now
#define audio_output_get_sample_rate oracle_audio_output_sample_rate
#define audio_output_get_channels oracle_audio_output_channels
#define audio_resampler_set_compensation_error oracle_audio_output_compensation_error
#define blog oracle_audio_output_blog
#define source_muted oracle_audio_output_source_muted
#define source_signal_audio_data oracle_audio_output_signal

#define obs_source_audio_timing_pre_lock oracle_obs_source_audio_timing_pre_lock
#define obs_source_audio_timing_under_lock oracle_obs_source_audio_timing_under_lock
#define obs_source_audio_timing_buffer_start oracle_obs_source_audio_timing_buffer_start
#define obs_source_audio_timing_follows_video oracle_obs_source_audio_timing_follows_video
#define obs_source_audio_place_started oracle_obs_source_audio_place_started
#define obs_source_audio_place_older oracle_obs_source_audio_place_older
#define obs_source_audio_place_size oracle_obs_source_audio_place_size
#define obs_source_audio_place_offset oracle_obs_source_audio_place_offset
#define obs_source_audio_place_fits oracle_obs_source_audio_place_fits
#define obs_source_audio_place_pop_back oracle_obs_source_audio_place_pop_back
#define obs_source_audio_place_outputs oracle_obs_source_audio_place_outputs

#define reset_audio_timing oracle_reset_audio_timing
#define reset_audio_data oracle_reset_audio_data
#define reset_audio_data_at_sys_time oracle_reset_audio_data_at_sys_time
#define source_output_audio_data oracle_source_output_audio_data

/* The C adds the int64_t sync offset to a uint64_t timestamp in its decisions,
 * and casts as libobs does. The oracle builds with -Wsign-conversion and libobs
 * does not. */
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif

/* the file's precompiled header brings obs-internal.h, so the oracle_* prototypes
 * come from there through the renames */
#include "obs-source-audio-output.c.h"

#undef source_output_audio_data
#undef reset_audio_data_at_sys_time
#undef reset_audio_data
#undef reset_audio_timing

#include "obs-source-audio-output-seam.h"
#include "obs-source-audio-output-seam.c.h"

#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic pop
#endif

#include <string.h>

#if defined(_MSC_VER)
#define ORACLE_THREAD_LOCAL __declspec(thread)
#else
#define ORACLE_THREAD_LOCAL __thread
#endif

/* libobs has this one in obs.c, under its own name. The tests never free it. */
struct obs_core *obs;

struct output_context {
	char log[32768];
	size_t len;
	bool muted;
};

struct oracle_audio_output_init {
	uint64_t timing_adjust;
	uint64_t next_audio_ts_min;
	uint64_t next_audio_sys_ts_min;
	uint64_t audio_ts;
	uint64_t resample_offset;
	int64_t sync_offset;
	int64_t last_sync_offset;
	int32_t monitoring_type;
	bool timing_set;
	bool async_unbuffered;
	bool async_decoupled;
	bool compensating;
	bool muted;
};

static ORACLE_THREAD_LOCAL obs_source_t *current;
static ORACLE_THREAD_LOCAL uint64_t clock_now;
static ORACLE_THREAD_LOCAL uint32_t output_rate;
static ORACLE_THREAD_LOCAL size_t output_channels;

obs_source_t *oracle_audio_output_new(void);
void oracle_audio_output_free(obs_source_t *source);
void oracle_audio_output_set_fields(obs_source_t *source, const struct oracle_audio_output_init *init);
void oracle_audio_output_begin(obs_source_t *source, uint64_t now, uint32_t rate, size_t channels);
struct audio_data *oracle_audio_output_packet_new(uint64_t timestamp, uint32_t frames, float value);
void oracle_audio_output_packet_free(struct audio_data *data);
size_t oracle_audio_output_state(const obs_source_t *source, char *out, size_t cap);
void oracle_audio_output_lock(obs_source_t *source);
void oracle_audio_output_unlock(obs_source_t *source);

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

static void put_fields(char *out, size_t cap, size_t *len, const obs_source_t *source)
{
	put_str(out, cap, len, source->timing_set ? "set adj " : "unset adj ");
	put_hex(out, cap, len, source->timing_adjust);
	put_str(out, cap, len, " min ");
	put_hex(out, cap, len, source->next_audio_ts_min);
	put_str(out, cap, len, " sys ");
	put_hex(out, cap, len, source->next_audio_sys_ts_min);
	put_str(out, cap, len, " ts ");
	put_hex(out, cap, len, source->audio_ts);
	put_str(out, cap, len, " last ");
	put_hex(out, cap, len, (uint64_t)source->last_audio_input_buf_size);
	put_str(out, cap, len, " lastsync ");
	put_hex(out, cap, len, (uint64_t)source->last_sync_offset);
	for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++) {
		put_str(out, cap, len, " buf ");
		put_hex(out, cap, len, (uint64_t)source->audio_input_buf[i].size);
	}
}

static void add_log(const char *name, uint64_t value)
{
	struct output_context *c = current->context.data;
	size_t cap = sizeof(c->log);

	put_str(c->log, cap, &c->len, name);
	put_str(c->log, cap, &c->len, " ");
	put_hex(c->log, cap, &c->len, value);
	put_str(c->log, cap, &c->len, " [");
	put_fields(c->log, cap, &c->len, current);
	put_str(c->log, cap, &c->len, "];");
}

uint64_t os_gettime_ns(void)
{
	add_log("now", clock_now);
	return clock_now;
}

uint32_t audio_output_get_sample_rate(const audio_t *audio)
{
	UNUSED_PARAMETER(audio);
	add_log("rate", output_rate);
	return output_rate;
}

size_t audio_output_get_channels(const audio_t *audio)
{
	UNUSED_PARAMETER(audio);
	add_log("channels", output_channels);
	return output_channels;
}

void audio_resampler_set_compensation_error(audio_resampler_t *resampler, int64_t error)
{
	UNUSED_PARAMETER(resampler);
	add_log("compensation", (uint64_t)error);
}

/* The format with each value in hex: %s and the PRIu64 forms are all it holds. */
void blog(int log_level, const char *format, ...)
{
	struct output_context *c = current->context.data;
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

bool source_muted(obs_source_t *source, uint64_t os_time)
{
	struct output_context *c = source->context.data;

	add_log("muted", os_time);
	return c->muted;
}

void source_signal_audio_data(obs_source_t *source, const struct audio_data *in, bool muted)
{
	bool buf_free = pthread_mutex_trylock(&source->audio_buf_mutex) == 0;

	if (buf_free)
		pthread_mutex_unlock(&source->audio_buf_mutex);
	add_log("signal ts", in->timestamp);
	add_log("signal frames", in->frames);
	add_log(muted ? "signal muted" : "signal unmuted", buf_free ? 1 : 0);
}

static pthread_once_t core_once = PTHREAD_ONCE_INIT;

static void core_init(void)
{
	obs = bzalloc(sizeof(*obs));
}

obs_source_t *oracle_audio_output_new(void)
{
	obs_source_t *source = bzalloc(sizeof(*source));

	pthread_once(&core_once, core_init);

	source->context.data = bzalloc(sizeof(struct output_context));
	source->context.name = (char *)"out";
	pthread_mutex_init(&source->audio_buf_mutex, NULL);
	return source;
}

void oracle_audio_output_free(obs_source_t *source)
{
	for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++)
		deque_free(&source->audio_input_buf[i]);
	pthread_mutex_destroy(&source->audio_buf_mutex);
	bfree(source->context.data);
	bfree(source);
}

void oracle_audio_output_set_fields(obs_source_t *source, const struct oracle_audio_output_init *init)
{
	struct output_context *c = source->context.data;

	source->timing_set = init->timing_set;
	source->timing_adjust = init->timing_adjust;
	source->next_audio_ts_min = init->next_audio_ts_min;
	source->next_audio_sys_ts_min = init->next_audio_sys_ts_min;
	source->audio_ts = init->audio_ts;
	source->resample_offset = init->resample_offset;
	source->sync_offset = init->sync_offset;
	source->last_sync_offset = init->last_sync_offset;
	source->monitoring_type = (enum obs_monitoring_type)init->monitoring_type;
	source->async_unbuffered = init->async_unbuffered;
	source->async_decoupled = init->async_decoupled;
	/* never dereferenced: the compensation call is the stand-in above */
	source->resampler = init->compensating ? (audio_resampler_t *)source : NULL;
	source->last_async_compensation = init->compensating;
	source->last_audio_input_buf_size = 7;
	c->muted = init->muted;
}

void oracle_audio_output_begin(obs_source_t *source, uint64_t now, uint32_t rate, size_t channels)
{
	current = source;
	clock_now = now;
	output_rate = rate;
	output_channels = channels;
}

struct audio_data *oracle_audio_output_packet_new(uint64_t timestamp, uint32_t frames, float value)
{
	struct audio_data *data = bzalloc(sizeof(*data));

	data->timestamp = timestamp;
	data->frames = frames;
	for (size_t ch = 0; ch < 2; ch++) {
		float *samples = bmalloc((size_t)frames * sizeof(float) + sizeof(float));
		for (uint32_t i = 0; i < frames; i++)
			samples[i] = (ch ? -value : value) + (float)i;
		data->data[ch] = (uint8_t *)samples;
	}
	return data;
}

void oracle_audio_output_packet_free(struct audio_data *data)
{
	bfree(data->data[0]);
	bfree(data->data[1]);
	bfree(data);
}

void oracle_audio_output_lock(obs_source_t *source)
{
	pthread_mutex_lock(&source->audio_buf_mutex);
}

void oracle_audio_output_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->audio_buf_mutex);
}

/* The log, every field, the contents of each buffer, and whether the mutex is
 * free. */
size_t oracle_audio_output_state(const obs_source_t *source, char *out, size_t cap)
{
	const struct output_context *c = source->context.data;
	obs_source_t *s = (obs_source_t *)(uintptr_t)source;
	size_t len = 0;

	out[0] = 0;
	put_str(out, cap, &len, c->log);
	put_str(out, cap, &len, "|");
	put_fields(out, cap, &len, source);

	for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++) {
		size_t size = source->audio_input_buf[i].size;
		uint64_t hash = 0xcbf29ce484222325ULL;

		if (size) {
			uint8_t *bytes = bmalloc(size);
			struct deque copy = source->audio_input_buf[i];
			deque_peek_front(&copy, bytes, size);
			hash = fnv_add(hash, bytes, size);
			bfree(bytes);
		}
		put_str(out, cap, &len, " hash ");
		put_hex(out, cap, &len, hash);
	}

	bool buf_free = pthread_mutex_trylock(&s->audio_buf_mutex) == 0;
	if (buf_free)
		pthread_mutex_unlock(&s->audio_buf_mutex);
	put_str(out, cap, &len, buf_free ? " free" : " held");
	return len;
}
