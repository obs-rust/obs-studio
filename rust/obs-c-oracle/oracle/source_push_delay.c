/* Original extracted C, unchanged, with real callback/calldata integration. */
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#define obs_source_set_push_to_mute_delay oracle_obs_source_set_push_to_mute_delay
#define obs_source_set_push_to_talk_delay oracle_obs_source_set_push_to_talk_delay
#include "obs-source-push-to-delay.c.h"
#undef obs_source_set_push_to_mute_delay
#undef obs_source_set_push_to_talk_delay

typedef void (*source_delay_setter)(obs_source_t *, uint64_t);
struct source_delay_seen {
	obs_source_t *source;
	source_delay_setter nested_set;
	uint64_t expected;
	unsigned calls;
	bool talk;
	bool ok;
	int lock_result;
};
static void *source_delay_probe(void *param)
{
	struct source_delay_seen *s = param;
	s->lock_result = pthread_mutex_trylock(&s->source->audio_mutex);
	if (!s->lock_result)
		pthread_mutex_unlock(&s->source->audio_mutex);
	return NULL;
}
static void source_delay_changed(void *param, calldata_t *data)
{
	struct source_delay_seen *s = param;
	void *source = NULL;
	long long delay = 0;
	s->ok &= calldata_get_ptr(data, "source", &source) && source == s->source;
	s->ok &= calldata_get_int(data, "delay", &delay) && (uint64_t)delay == s->expected;
	s->ok &= (s->talk ? s->source->push_to_talk_delay : s->source->push_to_mute_delay) == s->expected;
	pthread_t thread;
	if (pthread_create(&thread, NULL, source_delay_probe, s) != 0) {
		s->ok = false;
		return;
	}
	s->ok &= pthread_join(thread, NULL) == 0 && s->lock_result == EBUSY;
	if (s->nested_set) {
		struct obs_source other = {0};
		pthread_mutex_init(&other.audio_mutex, NULL);
		other.context.signals = signal_handler_create();
		signal_handler_add(other.context.signals, "void push_to_talk_delay(ptr source, int delay)");
		struct source_delay_seen nested = {.source = &other, .expected = UINT64_MAX, .talk = true, .ok = true};
		signal_handler_connect(other.context.signals, "push_to_talk_delay", source_delay_changed, &nested);
		s->nested_set(&other, UINT64_MAX);
		s->ok &= nested.ok && nested.calls == 1;
		signal_handler_destroy(other.context.signals);
		s->ok &= pthread_mutex_destroy(&other.audio_mutex) == 0;
	}
	calldata_set_int(data, "delay", 17);
	s->calls++;
}
bool oracle_source_delay_check(source_delay_setter mute, source_delay_setter talk, const uint64_t *values,
			       size_t count);
bool oracle_source_delay_check(source_delay_setter mute, source_delay_setter talk, const uint64_t *values, size_t count)
{
	struct obs_source source = {0};
	if (pthread_mutex_init(&source.audio_mutex, NULL) != 0)
		return false;
	source.context.signals = signal_handler_create();
	signal_handler_add(source.context.signals, "void push_to_mute_delay(ptr source, int delay)");
	signal_handler_add(source.context.signals, "void push_to_talk_delay(ptr source, int delay)");
	struct source_delay_seen m = {.source = &source, .nested_set = talk, .ok = true};
	struct source_delay_seen t = {.source = &source, .nested_set = talk, .talk = true, .ok = true};
	signal_handler_connect(source.context.signals, "push_to_mute_delay", source_delay_changed, &m);
	signal_handler_connect(source.context.signals, "push_to_talk_delay", source_delay_changed, &t);
	bool ok = true;
	uint64_t previous = 0;
	for (size_t i = 0; i < count; ++i) {
		m.expected = t.expected = values[i];
		mute(&source, values[i]);
		ok &= m.calls == i + 1 && t.calls == i;
		ok &= source.push_to_mute_delay == values[i] && source.push_to_talk_delay == previous;
		talk(&source, values[i]);
		ok &= m.calls == i + 1 && t.calls == i + 1;
		ok &= source.push_to_mute_delay == values[i] && source.push_to_talk_delay == values[i];
		int result = pthread_mutex_trylock(&source.audio_mutex);
		ok &= result == 0;
		if (!result)
			pthread_mutex_unlock(&source.audio_mutex);
		previous = values[i];
	}
	signal_handler_destroy(source.context.signals);
	ok &= pthread_mutex_destroy(&source.audio_mutex) == 0;
	return ok && m.ok && t.ok;
}
struct source_delay_log {
	unsigned calls;
	int level;
	char text[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_delay_capture(int level, const char *fmt, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_delay_capture(int level, const char *fmt, va_list args, void *param)
{
	struct source_delay_log *log = param;
	log->calls++;
	log->level = level;
	vsnprintf(log->text, sizeof(log->text), fmt, args);
}
bool oracle_source_delay_null(source_delay_setter mute, source_delay_setter talk);
bool oracle_source_delay_null(source_delay_setter mute, source_delay_setter talk)
{
	struct source_delay_log log = {0};
	log_handler_t handler;
	void *param;
	base_get_log_handler(&handler, &param);
	base_set_log_handler(source_delay_capture, &log);
	bool ok = true;
	for (unsigned i = 0; i < 2; ++i) {
		mute(NULL, i ? UINT64_MAX : 0);
		ok &= log.calls == i * 2 + 1 && log.level == LOG_DEBUG &&
		      strcmp(log.text, "obs_source_set_push_to_mute_delay: Null 'source' parameter") == 0;
		talk(NULL, i ? UINT64_MAX : 0);
		ok &= log.calls == i * 2 + 2 && log.level == LOG_DEBUG &&
		      strcmp(log.text, "obs_source_set_push_to_talk_delay: Null 'source' parameter") == 0;
	}
	base_set_log_handler(handler, param);
	return ok;
}
