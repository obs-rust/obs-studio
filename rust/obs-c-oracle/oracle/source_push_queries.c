/* Original C bodies, real source layout and automatically discovered adapter.
 * No intentional differences or excluded input values. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_push_to_mute_enabled oracle_obs_source_push_to_mute_enabled
#define obs_source_push_to_talk_enabled oracle_obs_source_push_to_talk_enabled
#define obs_source_get_push_to_mute_delay oracle_obs_source_get_push_to_mute_delay
#define obs_source_get_push_to_talk_delay oracle_obs_source_get_push_to_talk_delay
#include "obs-source-push-queries.c.h"
#undef obs_source_push_to_mute_enabled
#undef obs_source_push_to_talk_enabled
#undef obs_source_get_push_to_mute_delay
#undef obs_source_get_push_to_talk_delay
#include "source_push_query_fields.inc"

void oracle_source_push_layout(size_t *layout);
void oracle_source_push_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_push_query_fields);
	layout[1] = _Alignof(struct obs_source_push_query_fields);
	layout[2] = offsetof(struct obs_source_push_query_fields, audio_mutex);
	layout[3] = offsetof(struct obs_source_push_query_fields, mute_enabled);
	layout[4] = offsetof(struct obs_source_push_query_fields, talk_enabled);
	layout[5] = offsetof(struct obs_source_push_query_fields, mute_delay);
	layout[6] = offsetof(struct obs_source_push_query_fields, talk_delay);
	layout[7] = sizeof(bool);
	layout[8] = _Alignof(bool);
	layout[9] = sizeof(uint64_t);
	layout[10] = _Alignof(uint64_t);
}
struct source_push_fixture {
	struct obs_source source;
	unsigned char snapshot[sizeof(struct obs_source)];
};
void *oracle_source_push_new(bool mute, bool talk, uint64_t mute_delay, uint64_t talk_delay, bool pressed);
void *oracle_source_push_new(bool mute, bool talk, uint64_t mute_delay, uint64_t talk_delay, bool pressed)
{
	struct source_push_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	if (pthread_mutex_init(&fixture->source.audio_mutex, NULL) != 0) {
		free(fixture);
		return NULL;
	}
	fixture->source.push_to_mute_enabled = mute;
	fixture->source.push_to_talk_enabled = talk;
	fixture->source.push_to_mute_delay = mute_delay;
	fixture->source.push_to_talk_delay = talk_delay;
	fixture->source.push_to_mute_pressed = pressed;
	fixture->source.push_to_talk_pressed = !pressed;
	fixture->source.push_to_mute_stop_time = ~mute_delay;
	fixture->source.push_to_talk_stop_time = ~talk_delay;
	memcpy(fixture->snapshot, &fixture->source, sizeof(fixture->source));
	return fixture;
}
bool oracle_source_push_unchanged(const void *source);
bool oracle_source_push_unchanged(const void *source)
{
	const struct source_push_fixture *fixture = source;
	unsigned char current[sizeof(struct obs_source)];
	memcpy(current, &fixture->source, sizeof(current));
	/* Mutex internals may retain platform-specific bookkeeping after unlock. */
	memcpy(current + offsetof(struct obs_source, audio_mutex),
	       fixture->snapshot + offsetof(struct obs_source, audio_mutex), sizeof(pthread_mutex_t));
	return memcmp(current, fixture->snapshot, sizeof(current)) == 0;
}
void oracle_source_push_free(void *source);
void oracle_source_push_free(void *source)
{
	struct source_push_fixture *fixture = source;
	pthread_mutex_destroy(&fixture->source.audio_mutex);
	free(fixture);
}
struct source_push_log {
	unsigned count;
	int level;
	char message[128];
};

/* Like the other oracle printf forwarders, retain format checking at the call site. */
#if defined(__GNUC__) || defined(__clang__)
static void source_push_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif

static void source_push_capture(int level, const char *format, va_list args, void *param)
{
	struct source_push_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef bool (*push_bool_getter)(obs_source_t *source);
typedef uint64_t (*push_u64_getter)(obs_source_t *source);
bool oracle_source_push_null_log(push_bool_getter mute, push_bool_getter talk, push_u64_getter mute_delay,
				 push_u64_getter talk_delay);
bool oracle_source_push_null_log(push_bool_getter mute, push_bool_getter talk, push_u64_getter mute_delay,
				 push_u64_getter talk_delay)
{
	struct source_push_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(source_push_capture, &log);
	bool ok1 = mute(NULL) == 0 && log.count == 1 && log.level == LOG_DEBUG &&
		   strcmp(log.message, "obs_source_push_to_mute_enabled: Null 'source' parameter") == 0;
	bool ok2 = talk(NULL) == 0 && log.count == 2 && log.level == LOG_DEBUG &&
		   strcmp(log.message, "obs_source_push_to_talk_enabled: Null 'source' parameter") == 0;
	bool ok3 = mute_delay(NULL) == 0 && log.count == 3 && log.level == LOG_DEBUG &&
		   strcmp(log.message, "obs_source_get_push_to_mute_delay: Null 'source' parameter") == 0;
	bool ok4 = talk_delay(NULL) == 0 && log.count == 4 && log.level == LOG_DEBUG &&
		   strcmp(log.message, "obs_source_get_push_to_talk_delay: Null 'source' parameter") == 0;
	base_set_log_handler(old_handler, old_param);
	return ok1 && ok2 && ok3 && ok4;
}
