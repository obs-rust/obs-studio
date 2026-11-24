/* Original obs_source_get_audio_timestamp() and obs_source_get_audio_mix()
 * over the real private layout, and the production field adapters the Rust
 * shims read through (obs-source-audio-buffer-getters-fields.c.h, defined
 * once, here). */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_audio_timestamp oracle_obs_source_get_audio_timestamp
#define obs_source_get_audio_mix oracle_obs_source_get_audio_mix
#include "libobs/build/libobs.pch.h"
#include "source-audio-buffer-getters.inc"
#undef obs_source_get_audio_mix
#undef obs_source_get_audio_timestamp
#include "source_audio_buffer_getters_fields.inc"

struct source_audio_buffer_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_audio_buffer_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_audio_buffer_capture(int level, const char *format, va_list args, void *param)
{
	struct source_audio_buffer_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef uint64_t (*source_audio_timestamp_query)(const obs_source_t *);
typedef void (*source_audio_mix_query)(const obs_source_t *, struct obs_source_audio_mix *);
typedef void (*source_audio_buffer_get_log)(log_handler_t *, void **);
typedef void (*source_audio_buffer_set_log)(log_handler_t, void *);

static bool source_audio_buffer_logs_equal(const struct source_audio_buffer_log logs[2])
{
	return logs[0].count == logs[1].count && logs[0].level == logs[1].level &&
	       strcmp(logs[0].message, logs[1].message) == 0;
}

/* Whether the bytes outside audio_buf_mutex are unchanged: locking may touch
 * the mutex's own bookkeeping, nothing else may change. */
static bool source_audio_buffer_unchanged(const struct obs_source *before, const struct obs_source *after)
{
	const size_t start = offsetof(struct obs_source, audio_buf_mutex);
	const size_t end = start + sizeof(before->audio_buf_mutex);
	return memcmp(before, after, start) == 0 &&
	       memcmp((const unsigned char *)before + end, (const unsigned char *)after + end, sizeof(*before) - end) ==
		       0;
}

/* The sizes the Rust shim assumes: struct obs_source_audio_mix,
 * MAX_AUDIO_MIXES, MAX_AUDIO_CHANNELS and the source's audio_output_buf. */
void oracle_source_audio_buffer_layout(size_t *layout);
void oracle_source_audio_buffer_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_audio_mix);
	layout[1] = MAX_AUDIO_MIXES;
	layout[2] = MAX_AUDIO_CHANNELS;
	layout[3] = sizeof(((struct obs_source *)NULL)->audio_output_buf);
}

/* One source, or NULL when present is false: every byte is noise, then the
 * mutex is initialized and audio_ts takes ts. The given query and the
 * original each answer twice; both must answer alike, log alike (the NULL
 * line once for NULL, never otherwise), leave every byte outside the mutex
 * unchanged and the mutex unlocked. The shared answer goes to *answer and
 * the original's log line to message. */
bool oracle_source_audio_timestamp_check(source_audio_timestamp_query query, source_audio_buffer_get_log get_log,
					 source_audio_buffer_set_log set_log, bool present, uint8_t noise, uint64_t ts,
					 uint64_t *answer, char *message, size_t message_size);
bool oracle_source_audio_timestamp_check(source_audio_timestamp_query query, source_audio_buffer_get_log get_log,
					 source_audio_buffer_set_log set_log, bool present, uint8_t noise, uint64_t ts,
					 uint64_t *answer, char *message, size_t message_size)
{
	log_handler_t previous;
	void *param;
	get_log(&previous, &param);
	source_audio_timestamp_query queries[2] = {query, oracle_obs_source_get_audio_timestamp};
	uint64_t results[2][2];
	struct source_audio_buffer_log logs[2];
	bool equal = true;
	for (size_t side = 0; side < 2; side++) {
		struct obs_source *source = malloc(sizeof(*source));
		struct obs_source *before = malloc(sizeof(*before));
		if (!source || !before) {
			free(source);
			free(before);
			return false;
		}
		memset(source, noise, sizeof(*source));
		if (pthread_mutex_init(&source->audio_buf_mutex, NULL) != 0) {
			free(source);
			free(before);
			return false;
		}
		source->audio_ts = ts;
		memcpy(before, source, sizeof(*source));
		memset(&logs[side], 0, sizeof(logs[side]));
		logs[side].level = -1;
		set_log(source_audio_buffer_capture, &logs[side]);
		for (size_t repeat = 0; repeat < 2; repeat++)
			results[side][repeat] = queries[side](present ? source : NULL);
		set_log(previous, param);
		equal = equal && source_audio_buffer_unchanged(before, source);
		/* The mutex is left unlocked. */
		equal = equal && pthread_mutex_trylock(&source->audio_buf_mutex) == 0;
		pthread_mutex_unlock(&source->audio_buf_mutex);
		pthread_mutex_destroy(&source->audio_buf_mutex);
		free(source);
		free(before);
	}
	equal = equal && results[0][0] == results[1][0] && results[0][1] == results[1][1] &&
		results[1][0] == results[1][1] && logs[1].count == (present ? 0u : 2u) &&
		source_audio_buffer_logs_equal(logs);
	*answer = results[1][0];
	snprintf(message, message_size, "%s", logs[1].message);
	return equal;
}

/* One source (or NULL) whose bytes are noise and whose output buffers point
 * into a static block (NULL in every third slot, shifted by the noise), and
 * one mix (or NULL) filled with ~noise. The given query and the original
 * each fill a copy; both must log alike (once when either is NULL, never
 * otherwise), write the same mix and leave the source unchanged. *copied is
 * whether the original's mix is the source's buffers, *untouched whether it
 * is still the fill; the original's line goes to message. */
bool oracle_source_audio_mix_check(source_audio_mix_query query, source_audio_buffer_get_log get_log,
				   source_audio_buffer_set_log set_log, bool present, bool audio_present, uint8_t noise,
				   bool *copied, bool *untouched, char *message, size_t message_size);
bool oracle_source_audio_mix_check(source_audio_mix_query query, source_audio_buffer_get_log get_log,
				   source_audio_buffer_set_log set_log, bool present, bool audio_present, uint8_t noise,
				   bool *copied, bool *untouched, char *message, size_t message_size)
{
	static float samples[MAX_AUDIO_MIXES][MAX_AUDIO_CHANNELS];
	log_handler_t previous;
	void *param;
	get_log(&previous, &param);
	source_audio_mix_query queries[2] = {query, oracle_obs_source_get_audio_mix};
	struct obs_source_audio_mix mixes[2];
	struct source_audio_buffer_log logs[2];
	struct obs_source_audio_mix fill;
	memset(&fill, (uint8_t)~noise, sizeof(fill));
	bool equal = true;
	bool same_as_source = false;
	for (size_t side = 0; side < 2; side++) {
		struct obs_source *source = malloc(sizeof(*source));
		struct obs_source *before = malloc(sizeof(*before));
		if (!source || !before) {
			free(source);
			free(before);
			return false;
		}
		memset(source, noise, sizeof(*source));
		for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
			for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
				source->audio_output_buf[mix][ch] = (mix + ch + noise) % 3 == 0 ? NULL
												: &samples[mix][ch];
		memcpy(before, source, sizeof(*source));
		memcpy(&mixes[side], &fill, sizeof(fill));
		memset(&logs[side], 0, sizeof(logs[side]));
		logs[side].level = -1;
		set_log(source_audio_buffer_capture, &logs[side]);
		queries[side](present ? source : NULL, audio_present ? &mixes[side] : NULL);
		set_log(previous, param);
		equal = equal && memcmp(before, source, sizeof(*source)) == 0;
		if (side == 1) {
			same_as_source = true;
			for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
				for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
					same_as_source = same_as_source && mixes[1].output[mix].data[ch] ==
										   source->audio_output_buf[mix][ch];
		}
		free(source);
		free(before);
	}
	equal = equal && memcmp(&mixes[0], &mixes[1], sizeof(mixes[0])) == 0 &&
		logs[1].count == (present && audio_present ? 0u : 1u) && source_audio_buffer_logs_equal(logs);
	*copied = same_as_source;
	*untouched = memcmp(&mixes[1], &fill, sizeof(fill)) == 0;
	snprintf(message, message_size, "%s", logs[1].message);
	return equal;
}

/* A zeroed source with an initialized audio_buf_mutex and audio_ts, for a
 * reader on another thread while this one holds the mutex. */
struct obs_source *oracle_source_audio_buffer_new(uint64_t ts);
struct obs_source *oracle_source_audio_buffer_new(uint64_t ts)
{
	struct obs_source *source = calloc(1, sizeof(*source));
	if (!source)
		return NULL;
	if (pthread_mutex_init(&source->audio_buf_mutex, NULL) != 0) {
		free(source);
		return NULL;
	}
	source->audio_ts = ts;
	return source;
}

void oracle_source_audio_buffer_free(struct obs_source *source);
void oracle_source_audio_buffer_free(struct obs_source *source)
{
	pthread_mutex_destroy(&source->audio_buf_mutex);
	free(source);
}

void oracle_source_audio_buffer_lock(struct obs_source *source);
void oracle_source_audio_buffer_lock(struct obs_source *source)
{
	pthread_mutex_lock(&source->audio_buf_mutex);
}

/* Write audio_ts (under the mutex the caller holds) and release it. */
void oracle_source_audio_buffer_unlock_with(struct obs_source *source, uint64_t ts);
void oracle_source_audio_buffer_unlock_with(struct obs_source *source, uint64_t ts)
{
	source->audio_ts = ts;
	pthread_mutex_unlock(&source->audio_buf_mutex);
}
