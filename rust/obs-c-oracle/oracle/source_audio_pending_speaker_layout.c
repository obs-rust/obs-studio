/* Original obs_source_audio_pending() and obs_source_get_speaker_layout()
 * over the real private layout, and the production field adapters the Rust
 * shims read through (obs-source-audio-pending-speaker-layout-fields.c.h,
 * defined once, here). The original's obs_source_removed() call reaches the
 * obs-source Rust export, as it does in hybrid libobs. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_audio_pending oracle_obs_source_audio_pending
#define obs_source_get_speaker_layout oracle_obs_source_get_speaker_layout
#include "libobs/build/libobs.pch.h"
#include "source-audio-pending-speaker-layout.inc"
#undef obs_source_get_speaker_layout
#undef obs_source_audio_pending
#include "source_audio_pending_speaker_layout_fields.inc"

struct source_audio_pending_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_audio_pending_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_audio_pending_capture(int level, const char *format, va_list args, void *param)
{
	struct source_audio_pending_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef bool (*source_audio_pending_query)(const obs_source_t *);
typedef int (*source_speaker_layout_query)(obs_source_t *);
typedef void (*source_audio_pending_get_log)(log_handler_t *, void **);
typedef void (*source_audio_pending_set_log)(log_handler_t, void *);

struct source_audio_pending_fields {
	bool removed;
	uint32_t flags;
	bool pending;
	int speakers;
};

/* One noise-filled source per side, or NULL when present is false, with the
 * given fields; both sides run their query on it. */
static bool source_audio_pending_run(int (*run)(void *query, obs_source_t *), void *queries[2],
				     source_audio_pending_get_log get_log, source_audio_pending_set_log set_log,
				     bool present, uint8_t noise, const struct source_audio_pending_fields *fields,
				     int *answer, char *message, size_t message_size)
{
	log_handler_t previous;
	void *param;
	get_log(&previous, &param);
	int results[2];
	struct source_audio_pending_log logs[2];
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
		source->removed = fields->removed;
		source->info.output_flags = fields->flags;
		source->audio_pending = fields->pending;
		source->sample_info.speakers = (enum speaker_layout)fields->speakers;
		memcpy(before, source, sizeof(*source));
		memset(&logs[side], 0, sizeof(logs[side]));
		logs[side].level = -1;
		set_log(source_audio_pending_capture, &logs[side]);
		results[side] = run(queries[side], present ? source : NULL);
		set_log(previous, param);
		equal = equal && memcmp(before, source, sizeof(*source)) == 0;
		free(source);
		free(before);
	}
	equal = equal && results[0] == results[1] && logs[0].count == logs[1].count &&
		logs[1].count == (present ? 0u : 1u) && logs[0].level == logs[1].level &&
		strcmp(logs[0].message, logs[1].message) == 0;
	*answer = results[1];
	snprintf(message, message_size, "%s", logs[1].message);
	return equal;
}

static int source_audio_pending_call(void *query, obs_source_t *source)
{
	source_audio_pending_query q;
	memcpy(&q, &query, sizeof(q));
	return q(source) ? 1 : 0;
}

static int source_speaker_layout_call(void *query, obs_source_t *source)
{
	source_speaker_layout_query q;
	memcpy(&q, &query, sizeof(q));
	return q(source);
}

static bool oracle_audio_pending_original(const obs_source_t *source)
{
	return oracle_obs_source_audio_pending(source);
}

static int oracle_speaker_layout_original(obs_source_t *source)
{
	return (int)oracle_obs_source_get_speaker_layout(source);
}

/* The given query and the original pending query each answer on the same
 * fields: both must answer alike, log alike (the function's line once for
 * NULL, never otherwise) and leave every byte unchanged. The shared answer
 * goes to *answer and the original's log line to message. */
bool oracle_source_audio_pending_check(source_audio_pending_query query, source_audio_pending_get_log get_log,
				       source_audio_pending_set_log set_log, bool present, uint8_t noise, bool removed,
				       uint32_t flags, bool pending, bool *answer, char *message, size_t message_size);
bool oracle_source_audio_pending_check(source_audio_pending_query query, source_audio_pending_get_log get_log,
				       source_audio_pending_set_log set_log, bool present, uint8_t noise, bool removed,
				       uint32_t flags, bool pending, bool *answer, char *message, size_t message_size)
{
	source_audio_pending_query original = oracle_audio_pending_original;
	void *queries[2];
	memcpy(&queries[0], &query, sizeof(query));
	memcpy(&queries[1], &original, sizeof(original));
	const struct source_audio_pending_fields fields = {removed, flags, pending, 0};
	int result = 0;
	bool equal = source_audio_pending_run(source_audio_pending_call, queries, get_log, set_log, present, noise,
					      &fields, &result, message, message_size);
	*answer = result != 0;
	return equal;
}

/* The same for the speaker layout over sample_info.speakers. */
bool oracle_source_speaker_layout_check(source_speaker_layout_query query, source_audio_pending_get_log get_log,
					source_audio_pending_set_log set_log, bool present, uint8_t noise, bool removed,
					int speakers, int *answer, char *message, size_t message_size);
bool oracle_source_speaker_layout_check(source_speaker_layout_query query, source_audio_pending_get_log get_log,
					source_audio_pending_set_log set_log, bool present, uint8_t noise, bool removed,
					int speakers, int *answer, char *message, size_t message_size)
{
	source_speaker_layout_query original = oracle_speaker_layout_original;
	void *queries[2];
	memcpy(&queries[0], &query, sizeof(query));
	memcpy(&queries[1], &original, sizeof(original));
	const struct source_audio_pending_fields fields = {removed, 0, false, speakers};
	return source_audio_pending_run(source_speaker_layout_call, queries, get_log, set_log, present, noise, &fields,
					answer, message, message_size);
}
